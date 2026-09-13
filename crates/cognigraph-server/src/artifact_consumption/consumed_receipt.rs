//! Consumed receipt.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactConsumptionPurpose {
    CorpusProvenance,
    EvaluatedGraph,
    PromotionOracle,
    ScorerExecutable,
    VerifierExecutable,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumedArtifactReceipt {
    pub artifact_kind: ArtifactKind,
    pub purpose: ArtifactConsumptionPurpose,
    pub attestation_id: String,
    pub attestation_digest: String,
    pub manifest_digest: String,
    pub entry_count: u64,
    pub total_bytes: u64,
    pub verified_blob_set_digest: String,
    pub semantic_digest: String,
}
impl ConsumedArtifactReceipt {
    pub(super) fn validate_against(
        &self,
        binding: &ArtifactAttestationBinding,
        kind: ArtifactKind,
        purpose: ArtifactConsumptionPurpose,
    ) -> Result<(), CogniGraphError> {
        if self.artifact_kind != kind
            || self.purpose != purpose
            || self.attestation_id != binding.attestation_id
            || self.attestation_digest != binding.attestation_digest
            || self.manifest_digest != binding.manifest_digest
            || self.entry_count != binding.entry_count
            || self.total_bytes != binding.total_bytes
        {
            return Err(validation(
                "artifact consumption receipt does not match its M20 attestation binding",
            ));
        }
        for (label, digest) in [
            ("verified_blob_set_digest", &self.verified_blob_set_digest),
            ("semantic_digest", &self.semantic_digest),
        ] {
            validate_digest(label, digest)?;
        }
        Ok(())
    }
}
