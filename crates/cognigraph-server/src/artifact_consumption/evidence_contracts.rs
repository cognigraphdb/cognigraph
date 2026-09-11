//! Evidence contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceConsumptionAuthority {
    pub candidate_original_receipt_digest: String,
    pub candidate_replay_receipt_digest: String,
    pub candidate_material_digest: String,
    pub baseline_original_receipt_digest: String,
    pub baseline_replay_receipt_digest: String,
    pub baseline_material_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<Box<EvidenceDerivationAuthority>>,
    pub authority_digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceDerivationAuthority {
    pub candidate_original_derivation_digest: String,
    pub candidate_replay_derivation_digest: String,
    pub candidate_graph_digest: String,
    pub baseline_original_derivation_digest: String,
    pub baseline_replay_derivation_digest: String,
    pub baseline_graph_digest: String,
    pub derivation_plan_digest: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub preparation: Option<Box<EvidencePreparationAuthority>>,
    pub authority_digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidencePreparationAuthority {
    pub candidate_original_preparation_digest: String,
    pub candidate_replay_preparation_digest: String,
    pub baseline_original_preparation_digest: String,
    pub baseline_replay_preparation_digest: String,
    pub raw_document_set_digest: String,
    pub prepared_corpus_digest: String,
    pub preparation_plan_digest: String,
    pub authority_digest: String,
}
