//! Derivation plan.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusGraphDerivationPlan {
    pub schema_version: u32,
    pub deriver_id: String,
    pub deriver_version: String,
    pub deriver_semantics_digest: String,
    pub derivation_abi_digest: String,
    pub corpus_retention_bytes: u64,
    pub candidate_retention_bytes: u64,
    pub graph_retention_bytes: u64,
    pub max_chunks: u64,
    pub max_chunk_text_bytes: u64,
    pub max_config_items: u64,
    pub max_grounding_work: u64,
    pub max_chunk_grounding_work: u64,
    pub max_derived_facts: u64,
    pub plan_digest: String,
}
impl CorpusGraphDerivationPlan {
    pub(super) fn supported_v1() -> Self {
        let mut plan = Self {
            schema_version: M22_DERIVATION_PLAN_SCHEMA_VERSION,
            deriver_id: M22_DERIVER_ID.into(),
            deriver_version: M22_DERIVER_VERSION.into(),
            deriver_semantics_digest: digest_bytes(M22_DERIVER_SEMANTICS.as_bytes()),
            derivation_abi_digest: digest_bytes(M22_DERIVATION_ABI.as_bytes()),
            corpus_retention_bytes: MAX_M22_CORPUS_ARTIFACT_BYTES,
            candidate_retention_bytes: MAX_M22_CANDIDATE_ARTIFACT_BYTES,
            graph_retention_bytes: MAX_GRAPH_ARTIFACT_BYTES,
            max_chunks: MAX_M22_CHUNKS as u64,
            max_chunk_text_bytes: MAX_M22_CHUNK_TEXT_BYTES as u64,
            max_config_items: MAX_M22_CONFIG_ITEMS as u64,
            max_grounding_work: MAX_M22_GROUNDING_WORK,
            max_chunk_grounding_work: MAX_M22_CHUNK_GROUNDING_WORK,
            max_derived_facts: MAX_GRAPH_FACTS as u64,
            plan_digest: String::new(),
        };
        plan.plan_digest = record_digest(&plan, "plan_digest")
            .expect("the pinned M22 derivation plan is canonical");
        plan
    }

    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        if self != &Self::supported_v1() {
            return Err(validation(
                "M22 derivation plan does not match the pinned prepared-corpus grounder",
            ));
        }
        Ok(())
    }
}
