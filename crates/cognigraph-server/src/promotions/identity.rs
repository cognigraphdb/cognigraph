//! Identity.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionTarget {
    pub space_type: String,
    pub channel: String,
}
impl PromotionTarget {
    pub fn validate(&self) -> Result<(), CogniGraphError> {
        validate_path_segment("space_type", &self.space_type)?;
        validate_path_segment("channel", &self.channel)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactIdentity {
    pub uri: String,
    pub media_type: String,
    pub bytes: u64,
    pub digest: String,
}
impl ArtifactIdentity {
    pub(super) fn validate(&self, label: &str) -> Result<(), CogniGraphError> {
        validate_text(&format!("{label}.uri"), &self.uri, MAX_URI_BYTES)?;
        validate_text(
            &format!("{label}.media_type"),
            &self.media_type,
            MAX_IDENTIFIER_BYTES,
        )?;
        if self.bytes == 0 {
            return Err(validation(format!("{label}.bytes must be positive")));
        }
        validate_digest(&format!("{label}.digest"), &self.digest)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateIdentity {
    pub kind: String,
    pub id: String,
    pub revision: String,
    pub artifact: ArtifactIdentity,
    pub candidate_digest: String,
}
impl CandidateIdentity {
    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        validate_text("candidate.kind", &self.kind, MAX_IDENTIFIER_BYTES)?;
        validate_text("candidate.id", &self.id, MAX_IDENTIFIER_BYTES)?;
        validate_text("candidate.revision", &self.revision, MAX_IDENTIFIER_BYTES)?;
        self.artifact.validate("candidate.artifact")?;
        validate_digest("candidate.candidate_digest", &self.candidate_digest)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveConfiguration {
    pub construction_config_digest: String,
    pub evaluation_config_digest: String,
    pub eval_spec_digest: String,
    pub preprocessing_digest: String,
    pub scorer_id: String,
    pub scorer_version: String,
    pub scorer_artifact_digest: String,
}
impl EffectiveConfiguration {
    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        for (label, digest) in [
            (
                "effective_configuration.construction_config_digest",
                &self.construction_config_digest,
            ),
            (
                "effective_configuration.evaluation_config_digest",
                &self.evaluation_config_digest,
            ),
            (
                "effective_configuration.eval_spec_digest",
                &self.eval_spec_digest,
            ),
            (
                "effective_configuration.preprocessing_digest",
                &self.preprocessing_digest,
            ),
            (
                "effective_configuration.scorer_artifact_digest",
                &self.scorer_artifact_digest,
            ),
        ] {
            validate_digest(label, digest)?;
        }
        validate_text(
            "effective_configuration.scorer_id",
            &self.scorer_id,
            MAX_IDENTIFIER_BYTES,
        )?;
        validate_text(
            "effective_configuration.scorer_version",
            &self.scorer_version,
            MAX_IDENTIFIER_BYTES,
        )?;
        if self.scorer_id != M18_SCORER_ID
            || self.scorer_version != M18_SCORER_VERSION
            || self.scorer_artifact_digest != digest_bytes(M18_SCORER_ARTIFACT_IDENTITY.as_bytes())
        {
            return Err(validation(
                "M18 scorer identity does not match the server's distinct-fact evaluator",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionAttestation {
    pub kind: String,
    pub revision_id: String,
    pub manifest_uri: String,
    pub manifest_digest: String,
    pub source_lineage: String,
    pub immutability_method: String,
    pub issuer: String,
    pub issued_at_ms: u64,
    pub verification_uri: String,
    pub verification_digest: String,
    pub candidate_digest: Option<String>,
    pub configuration_digest: Option<String>,
    pub immutable: bool,
}
impl RevisionAttestation {
    pub(super) fn validate(&self, label: &str) -> Result<(), CogniGraphError> {
        for (field, value) in [
            ("kind", &self.kind),
            ("revision_id", &self.revision_id),
            ("source_lineage", &self.source_lineage),
            ("immutability_method", &self.immutability_method),
            ("issuer", &self.issuer),
        ] {
            validate_text(
                &format!("revisions.{label}.{field}"),
                value,
                MAX_IDENTIFIER_BYTES,
            )?;
        }
        validate_text(
            &format!("revisions.{label}.manifest_uri"),
            &self.manifest_uri,
            MAX_URI_BYTES,
        )?;
        validate_text(
            &format!("revisions.{label}.verification_uri"),
            &self.verification_uri,
            MAX_URI_BYTES,
        )?;
        validate_digest(
            &format!("revisions.{label}.manifest_digest"),
            &self.manifest_digest,
        )?;
        validate_digest(
            &format!("revisions.{label}.verification_digest"),
            &self.verification_digest,
        )?;
        if let Some(digest) = &self.candidate_digest {
            validate_digest(&format!("revisions.{label}.candidate_digest"), digest)?;
        }
        if let Some(digest) = &self.configuration_digest {
            validate_digest(&format!("revisions.{label}.configuration_digest"), digest)?;
        }
        if !self.immutable {
            return Err(validation(format!(
                "revisions.{label} is not attested immutable"
            )));
        }
        if self.issued_at_ms == 0 {
            return Err(validation(format!(
                "revisions.{label}.issued_at_ms must be positive"
            )));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionSet {
    pub corpus: RevisionAttestation,
    pub graph: RevisionAttestation,
    pub oracle: RevisionAttestation,
}
