//! Project-owner-accepted development-showcase calibration for the DailyMed
//! clinical matcher.
//!
//! This is project-owner accepted calibration on third-party label text. It is
//! deliberately not represented as external clinical validation and cannot be
//! used as medical advice or a substitute for the two-reviewer held-out gold.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::model::*;

pub const CALIBRATION_SCHEMA_VERSION: &str = "dailymed-clinical-calibration-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationArtifact {
    pub schema_version: String,
    pub ruleset_version: String,
    pub purpose: String,
    pub accepted_by: String,
    pub accepted_at: String,
    pub acceptance_basis: String,
    pub limitation: String,
    pub cases_digest: String,
    pub cases: Vec<CalibrationCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationCase {
    pub id: String,
    pub set_id: String,
    pub relation: String,
    pub concept: String,
    pub verdict: ClinicalVerdict,
    pub section_code: String,
    pub evidence: String,
    pub rule: String,
    #[serde(default)]
    pub notes: String,
}

pub fn calibration_digest(cases: &[CalibrationCase]) -> Result<String> {
    let encoded = serde_json::to_vec(cases)?;
    let digest = Sha256::digest(encoded);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub fn load_calibration(path: &Path, packets: &[ReviewPacket]) -> Result<CalibrationArtifact> {
    let body = fs::read_to_string(path)
        .with_context(|| format!("read calibration artifact {}", path.display()))?;
    let artifact: CalibrationArtifact = serde_json::from_str(&body)
        .with_context(|| format!("parse calibration artifact {}", path.display()))?;
    if artifact.schema_version != CALIBRATION_SCHEMA_VERSION {
        bail!("unsupported calibration schema {}", artifact.schema_version);
    }
    if artifact.accepted_by.trim().is_empty() || artifact.accepted_at.trim().is_empty() {
        bail!("calibration artifact has no recorded acceptance");
    }
    let expected = calibration_digest(&artifact.cases)?;
    if artifact.cases_digest != expected {
        bail!(
            "calibration artifact failed its integrity check (expected {expected}, found {})",
            artifact.cases_digest
        );
    }

    let packet_map: BTreeMap<_, _> = packets
        .iter()
        .filter(|packet| packet.split == ReviewSplit::Calibration)
        .map(|packet| (packet.set_id.as_str(), packet))
        .collect();
    let mut ids = BTreeSet::new();
    let mut facts = BTreeSet::new();
    for case in &artifact.cases {
        if !ids.insert(case.id.as_str()) {
            bail!("duplicate calibration case id {}", case.id);
        }
        let packet = packet_map
            .iter()
            .find_map(|(set_id, packet)| (*set_id == case.set_id).then_some(*packet))
            .with_context(|| format!("case {} is not a calibration document", case.id))?;
        if case.relation != TREATS && case.relation != CONTRAINDICATED_IN {
            bail!(
                "case {} has unsupported relation {}",
                case.id,
                case.relation
            );
        }
        if case.verdict == ClinicalVerdict::Unreviewed {
            bail!("case {} is unreviewed", case.id);
        }
        if case.concept.trim().is_empty() || case.evidence.trim().is_empty() {
            bail!("case {} has an empty concept or evidence", case.id);
        }
        let section = packet
            .sections
            .iter()
            .find(|section| section.code == case.section_code)
            .with_context(|| format!("case {} references an absent section", case.id))?;
        if !section.text.contains(&case.evidence) {
            bail!(
                "case {} evidence is not an exact section substring",
                case.id
            );
        }
        let fact = (
            case.set_id.as_str(),
            case.relation.as_str(),
            case.concept.to_lowercase(),
        );
        if !facts.insert(fact) {
            bail!("duplicate calibration fact in case {}", case.id);
        }
    }
    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_changes_with_a_verdict() {
        let mut case = CalibrationCase {
            id: "x".into(),
            set_id: "s".into(),
            relation: TREATS.into(),
            concept: "spasticity".into(),
            verdict: ClinicalVerdict::True,
            section_code: "34067-9".into(),
            evidence: "evidence".into(),
            rule: "R1".into(),
            notes: String::new(),
        };
        let first = calibration_digest(std::slice::from_ref(&case)).unwrap();
        case.verdict = ClinicalVerdict::False;
        let second = calibration_digest(&[case]).unwrap();
        assert_ne!(first, second);
    }
}
