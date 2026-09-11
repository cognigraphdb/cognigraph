//! Preparation plan.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCorpusPreparationPlan {
    pub schema_version: u32,
    pub preparer_id: String,
    pub preparer_version: String,
    pub preparer_semantics_digest: String,
    pub preparation_abi_digest: String,
    pub documents_retention_bytes: u64,
    pub prepared_corpus_retention_bytes: u64,
    pub max_documents: u64,
    pub max_raw_document_bytes: u64,
    pub max_total_raw_document_bytes: u64,
    pub max_normalized_document_bytes: u64,
    pub max_total_normalized_bytes: u64,
    pub max_chunk_bytes: u64,
    pub max_chunks: u64,
    pub max_total_prepared_text_bytes: u64,
    pub yield_every_documents: u64,
    pub plan_digest: String,
}
impl RawCorpusPreparationPlan {
    pub(super) fn supported_v1() -> Self {
        let mut plan = Self {
            schema_version: M23_PREPARATION_PLAN_SCHEMA_VERSION,
            preparer_id: M23_PREPARER_ID.into(),
            preparer_version: M23_PREPARER_VERSION.into(),
            preparer_semantics_digest: digest_bytes(M23_PREPARER_SEMANTICS.as_bytes()),
            preparation_abi_digest: digest_bytes(M23_PREPARATION_ABI.as_bytes()),
            documents_retention_bytes: MAX_M23_DOCUMENTS_ARTIFACT_BYTES,
            prepared_corpus_retention_bytes: MAX_M22_CORPUS_ARTIFACT_BYTES,
            max_documents: MAX_M23_DOCUMENTS as u64,
            max_raw_document_bytes: MAX_M23_RAW_DOCUMENT_BYTES as u64,
            max_total_raw_document_bytes: MAX_M23_TOTAL_RAW_DOCUMENT_BYTES as u64,
            max_normalized_document_bytes: MAX_M23_NORMALIZED_DOCUMENT_BYTES as u64,
            max_total_normalized_bytes: MAX_M23_TOTAL_NORMALIZED_BYTES as u64,
            max_chunk_bytes: MAX_M23_CHUNK_BYTES as u64,
            max_chunks: MAX_M22_CHUNKS as u64,
            max_total_prepared_text_bytes: MAX_M23_TOTAL_PREPARED_TEXT_BYTES as u64,
            yield_every_documents: M23_YIELD_EVERY_DOCUMENTS as u64,
            plan_digest: String::new(),
        };
        plan.plan_digest = record_digest(&plan, "plan_digest")
            .expect("the pinned M23 preparation plan is canonical");
        plan
    }

    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        if self != &Self::supported_v1() {
            return Err(validation(
                "M23 preparation plan does not match the pinned UTF-8 document preparer",
            ));
        }
        Ok(())
    }
}
