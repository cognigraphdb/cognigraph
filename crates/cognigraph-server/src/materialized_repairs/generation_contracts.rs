//! Generation contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticRepairGenerationRecord {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub semantic_repair_generation_id: String,
    pub target: PromotionTarget,
    pub source_evidence_id: String,
    pub source_evidence_digest: String,
    pub source_candidate_original_job_id: String,
    pub source_candidate_original_receipt_digest: String,
    pub source_candidate_replay_job_id: String,
    pub source_candidate_replay_receipt_digest: String,
    pub promotion_head_decision_id: String,
    pub promotion_head_projection_digest: String,
    pub candidate_digest: String,
    pub semantic_repair_revision_id: String,
    pub semantic_repair_revision_digest: String,
    pub semantic_repair_review_id: String,
    pub semantic_repair_review_digest: String,
    pub prepared_corpus_digest: String,
    pub derivation_plan_digest: String,
    pub derivation_material_digest: String,
    pub materialization_plan: MaterializationPlan,
    pub projection: MaterializedProjection,
    pub impact: VerifiedSemanticRepairImpact,
    pub created_at_ms: u64,
    pub created_by: GovernanceActor,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub canonical_record_bytes: u64,
    pub semantic_repair_generation_digest: String,
}
impl SemanticRepairGenerationRecord {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_value(self)
    }

    pub fn summary_value(&self) -> Value {
        json!({
            "schema_version": self.schema_version,
            "semantic_repair_generation_id": self.semantic_repair_generation_id,
            "semantic_repair_generation_digest": self.semantic_repair_generation_digest,
            "target": self.target,
            "source_evidence_id": self.source_evidence_id,
            "candidate_digest": self.candidate_digest,
            "promotion_head_decision_id": self.promotion_head_decision_id,
            "semantic_repair_revision_id": self.semantic_repair_revision_id,
            "semantic_repair_review_id": self.semantic_repair_review_id,
            "prepared_corpus_digest": self.prepared_corpus_digest,
            "projection_digest": self.projection.projection_digest,
            "impact_digest": self.impact.impact_digest,
            "counts": {
                "entities": self.projection.entity_count,
                "chunks": self.projection.chunk_count,
                "mentions": self.projection.mention_count,
                "fact_occurrences": self.projection.fact_count,
                "semantic_facts": self.projection.semantic_facts.len(),
                "impact_added": self.impact.added_count,
                "impact_removed": self.impact.removed_count,
                "impact_unchanged": self.impact.unchanged_count,
            },
            "canonical_record_bytes": self.canonical_record_bytes,
            "created_at_ms": self.created_at_ms,
            "created_by": self.created_by,
        })
    }
}
