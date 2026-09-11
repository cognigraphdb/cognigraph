//! Backend capability.

use super::*;

pub(super) fn materialization_options() -> MaterializationOptions {
    MaterializationOptions {
        max_entity_count: MAX_M26_ENTITIES,
        max_chunk_count: MAX_M26_CHUNKS,
        max_mention_count: MAX_M26_MENTIONS,
        max_fact_occurrence_count: MAX_M26_FACT_OCCURRENCES,
        max_semantic_fact_count: MAX_M26_SEMANTIC_FACTS,
        yield_every_chunks: 16,
    }
}
pub(super) fn require_native_atomic_backend(
    manager: &PromotionManager,
) -> Result<(), CogniGraphError> {
    if !manager.backend.supports_atomic_batches() {
        return Err(CogniGraphError::ConnectionError(format!(
            "M26 verified Semantic Repair materialization requires atomic batch support; backend `{}` is unsupported",
            manager.backend.backend_name()
        )));
    }
    Ok(())
}
pub(super) fn validate_non_atomic_repository_probe(
    backend_name: &str,
    collection: &str,
    probe: Result<Vec<Value>, CogniGraphError>,
) -> Result<(), CogniGraphError> {
    match probe {
        Ok(_) => Err(CogniGraphError::ConnectionError(format!(
            "M26 protected collection `{collection}` is present on non-atomic backend `{backend_name}`; verified Semantic Repair recovery requires atomic batch support"
        ))),
        Err(CogniGraphError::CollectionNotFound(_)) => Ok(()),
        Err(error) => Err(error),
    }
}
pub(super) fn source_by_role(
    evidence: &PromotionEvidence,
    role: EvidenceRunRole,
) -> Result<&crate::promotions::PromotionEvaluationSource, CogniGraphError> {
    evidence
        .runs
        .iter()
        .find(|run| run.role == role)
        .map(|run| &run.source)
        .ok_or_else(|| conflict("M26 source evidence is missing a required evaluation run"))
}
pub(super) fn derivation_for(
    source: &crate::promotions::PromotionEvaluationSource,
) -> Result<&crate::artifact_consumption::CorpusGraphDerivationReceipt, CogniGraphError> {
    source
        .artifact_consumption
        .as_deref()
        .and_then(|receipt| receipt.derivation.as_deref())
        .ok_or_else(|| conflict("M26 generation requires an M22/M23 derivation receipt"))
}
