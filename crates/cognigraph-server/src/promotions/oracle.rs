//! Oracle.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleAttestationStatus {
    Verified,
    Failed,
    Unverifiable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleStageRule {
    pub stage: String,
    pub allow_oracle_reads: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OraclePolicy {
    pub required_status: OracleAttestationStatus,
    pub verifier_name: String,
    pub verifier_version: String,
    pub verifier_artifact_digest: String,
    pub required_stages: Vec<OracleStageRule>,
}
impl OraclePolicy {
    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        if self.required_status != OracleAttestationStatus::Verified
            || self.verifier_name != M18_ORACLE_VERIFIER_NAME
            || self.verifier_version != M18_ORACLE_VERIFIER_VERSION
            || self.verifier_artifact_digest
                != digest_bytes(M18_ORACLE_VERIFIER_ARTIFACT_IDENTITY.as_bytes())
        {
            return Err(validation(
                "M18 oracle policy does not use the pinned verifier identity",
            ));
        }
        let required = [
            ("candidate_build", false),
            ("candidate_tuning", false),
            ("construction", false),
            ("evaluation", true),
        ];
        if self.required_stages.len() != required.len()
            || self
                .required_stages
                .iter()
                .zip(required)
                .any(|(actual, (stage, allow))| {
                    actual.stage != stage || actual.allow_oracle_reads != allow
                })
        {
            return Err(validation(
                "M18 oracle policy must cover candidate_build, candidate_tuning, construction, and evaluation",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleStageProjection {
    pub stage: String,
    pub read_set_digest: String,
    pub oracle_reads: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleSeparationAttestation {
    pub status: OracleAttestationStatus,
    pub promotion_oracle_digest: String,
    pub stage_projections: Vec<OracleStageProjection>,
    pub verifier_name: String,
    pub verifier_version: String,
    pub verifier_artifact_digest: String,
    pub verifier_run_id: String,
    pub attested_by: String,
    pub verified_at_ms: u64,
    pub attestation_uri: String,
    pub attestation_digest: String,
    pub overlapping_document_ids: u64,
    pub overlapping_case_ids: u64,
    pub overlapping_content_digests: u64,
    pub overlapping_artifact_digests: u64,
}
impl OracleSeparationAttestation {
    pub(super) fn validate_structure(&self) -> Result<(), CogniGraphError> {
        if self.verified_at_ms == 0 {
            return Err(validation(
                "oracle_separation.verified_at_ms must be positive",
            ));
        }
        for (label, digest) in [
            ("promotion_oracle_digest", &self.promotion_oracle_digest),
            ("verifier_artifact_digest", &self.verifier_artifact_digest),
            ("attestation_digest", &self.attestation_digest),
        ] {
            validate_digest(&format!("oracle_separation.{label}"), digest)?;
        }
        for (label, value) in [
            ("verifier_name", &self.verifier_name),
            ("verifier_version", &self.verifier_version),
            ("verifier_run_id", &self.verifier_run_id),
            ("attested_by", &self.attested_by),
        ] {
            validate_text(
                &format!("oracle_separation.{label}"),
                value,
                MAX_IDENTIFIER_BYTES,
            )?;
        }
        validate_text(
            "oracle_separation.attestation_uri",
            &self.attestation_uri,
            MAX_URI_BYTES,
        )?;
        let mut previous_stage: Option<&str> = None;
        for projection in &self.stage_projections {
            validate_text(
                "oracle_separation.stage",
                &projection.stage,
                MAX_IDENTIFIER_BYTES,
            )?;
            validate_digest(
                "oracle_separation.stage.read_set_digest",
                &projection.read_set_digest,
            )?;
            if previous_stage.is_some_and(|previous| previous >= projection.stage.as_str()) {
                return Err(validation(
                    "oracle stage projections must be sorted and unique",
                ));
            }
            previous_stage = Some(&projection.stage);
        }
        if record_digest(self, "attestation_digest")? != self.attestation_digest {
            return Err(validation("oracle attestation content digest mismatch"));
        }
        Ok(())
    }

    pub(super) fn validate_for_policy(&self, policy: &OraclePolicy) -> Result<(), CogniGraphError> {
        self.validate_structure()?;
        if self.status != policy.required_status {
            return Err(validation("oracle separation is not verified"));
        }
        if self.verifier_name != policy.verifier_name
            || self.verifier_version != policy.verifier_version
            || self.verifier_artifact_digest != policy.verifier_artifact_digest
            || self.attested_by != policy.verifier_name
        {
            return Err(validation(
                "oracle attestation does not match the pinned verifier",
            ));
        }
        if self.stage_projections.len() != policy.required_stages.len() {
            return Err(validation(
                "oracle attestation stage coverage is incomplete",
            ));
        }
        for (projection, rule) in self.stage_projections.iter().zip(&policy.required_stages) {
            if projection.stage != rule.stage
                || (!rule.allow_oracle_reads && projection.oracle_reads != 0)
            {
                return Err(validation(
                    "oracle attestation reports a missing stage or forbidden oracle read",
                ));
            }
        }
        if self.overlapping_document_ids != 0
            || self.overlapping_case_ids != 0
            || self.overlapping_content_digests != 0
            || self.overlapping_artifact_digests != 0
        {
            return Err(validation(
                "oracle separation reports construction exposure or overlap",
            ));
        }
        Ok(())
    }
}
