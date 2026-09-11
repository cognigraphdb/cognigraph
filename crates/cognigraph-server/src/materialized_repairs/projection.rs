//! Projection.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializationPlan {
    pub schema_version: u32,
    pub materializer_id: String,
    pub materializer_version: String,
    pub materializer_semantics_digest: String,
    pub materialization_abi_digest: String,
    pub max_chunks: u64,
    pub max_semantic_facts: u64,
    pub max_total_rows: u64,
    pub max_canonical_generation_bytes: u64,
    pub plan_digest: String,
}
impl MaterializationPlan {
    pub fn current() -> Result<Self, CogniGraphError> {
        let mut plan = Self {
            schema_version: 1,
            materializer_id: MATERIALIZER_ID.into(),
            materializer_version: MATERIALIZER_VERSION.into(),
            materializer_semantics_digest: digest_bytes(MATERIALIZER_SEMANTICS.as_bytes()),
            materialization_abi_digest: digest_bytes(MATERIALIZATION_ABI.as_bytes()),
            max_chunks: MAX_M26_CHUNKS as u64,
            max_semantic_facts: MAX_M26_SEMANTIC_FACTS as u64,
            max_total_rows: MAX_M26_TOTAL_ROWS as u64,
            max_canonical_generation_bytes: MAX_M26_CANONICAL_GENERATION_BYTES as u64,
            plan_digest: String::new(),
        };
        plan.plan_digest = record_digest(&plan, "plan_digest")?;
        Ok(plan)
    }

    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        if self != &Self::current()? {
            return Err(conflict("unsupported or altered M26 materialization plan"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSemanticRepairGenerationRequest {
    pub target: PromotionTarget,
    pub expected_promotion_head_decision_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedProjection {
    pub schema_version: u32,
    pub space_id: String,
    pub entities: Vec<MaterializedEntityRow>,
    pub chunks: Vec<MaterializedChunkRow>,
    pub mentions: Vec<MaterializedMentionRow>,
    pub facts: Vec<MaterializedFactOccurrenceRow>,
    pub entity_count: u64,
    pub chunk_count: u64,
    pub mention_count: u64,
    pub fact_count: u64,
    pub entities_digest: String,
    pub chunks_digest: String,
    pub mentions_digest: String,
    pub facts_digest: String,
    pub semantic_facts: Vec<VerifiedGraphFact>,
    pub semantic_facts_digest: String,
    pub projection_digest: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProjectionCapacityCounts {
    pub(super) entities: usize,
    pub(super) chunks: usize,
    pub(super) mentions: usize,
    pub(super) fact_occurrences: usize,
    pub(super) semantic_facts: usize,
}
pub(super) fn validate_projection_capacity_counts(
    counts: ProjectionCapacityCounts,
) -> Result<(), CogniGraphError> {
    let total_rows = counts
        .entities
        .checked_add(counts.chunks)
        .and_then(|count| count.checked_add(counts.mentions))
        .and_then(|count| count.checked_add(counts.fact_occurrences))
        .ok_or_else(|| capacity("M26 materialized row count overflowed"))?;
    if counts.entities > MAX_M26_ENTITIES
        || counts.chunks == 0
        || counts.chunks > MAX_M26_CHUNKS
        || counts.mentions > MAX_M26_MENTIONS
        || counts.fact_occurrences > MAX_M26_FACT_OCCURRENCES
        || counts.semantic_facts > MAX_M26_SEMANTIC_FACTS
        || total_rows > MAX_M26_TOTAL_ROWS
    {
        return Err(conflict(
            "M26 materialized projection is malformed or exceeds limits",
        ));
    }
    Ok(())
}
impl MaterializedProjection {
    pub(super) fn from_derived(
        projection: MaterializedGraphProjection,
    ) -> Result<Self, CogniGraphError> {
        projection
            .validate()
            .map_err(|error| validation(format!("M26 materialized projection failed: {error}")))?;
        let semantic_facts = projection
            .semantic_facts()
            .map_err(|error| validation(format!("M26 semantic projection failed: {error}")))?
            .into_iter()
            .map(|fact| VerifiedGraphFact {
                source: fact.source,
                relation: fact.relation,
                target: fact.target,
                evidence_chunk_id: fact.evidence_chunk_id,
            })
            .collect::<Vec<_>>();
        let mut value = Self {
            schema_version: 1,
            space_id: projection.space_id,
            entity_count: projection.entities.len() as u64,
            chunk_count: projection.chunks.len() as u64,
            mention_count: projection.mentions.len() as u64,
            fact_count: projection.facts.len() as u64,
            entities_digest: canonical_digest(&projection.entities)?,
            chunks_digest: canonical_digest(&projection.chunks)?,
            mentions_digest: canonical_digest(&projection.mentions)?,
            facts_digest: canonical_digest(&projection.facts)?,
            semantic_facts_digest: canonical_digest(&semantic_facts)?,
            entities: projection.entities,
            chunks: projection.chunks,
            mentions: projection.mentions,
            facts: projection.facts,
            semantic_facts,
            projection_digest: String::new(),
        };
        value.projection_digest = record_digest(&value, "projection_digest")?;
        value.validate()?;
        Ok(value)
    }

    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        validate_projection_capacity_counts(ProjectionCapacityCounts {
            entities: self.entities.len(),
            chunks: self.chunks.len(),
            mentions: self.mentions.len(),
            fact_occurrences: self.facts.len(),
            semantic_facts: self.semantic_facts.len(),
        })?;
        if self.schema_version != 1
            || self.space_id.trim().is_empty()
            || self.entity_count != self.entities.len() as u64
            || self.chunk_count != self.chunks.len() as u64
            || self.mention_count != self.mentions.len() as u64
            || self.fact_count != self.facts.len() as u64
            || self.entities_digest != canonical_digest(&self.entities)?
            || self.chunks_digest != canonical_digest(&self.chunks)?
            || self.mentions_digest != canonical_digest(&self.mentions)?
            || self.facts_digest != canonical_digest(&self.facts)?
            || self.semantic_facts_digest != canonical_digest(&self.semantic_facts)?
            || self.projection_digest != record_digest(self, "projection_digest")?
        {
            return Err(conflict(
                "M26 materialized projection is malformed or exceeds limits",
            ));
        }
        require_sorted_unique_keys(&self.entities)?;
        require_sorted_unique_keys(&self.chunks)?;
        require_sorted_unique_keys(&self.mentions)?;
        require_sorted_unique_keys(&self.facts)?;
        let raw = MaterializedGraphProjection {
            space_id: self.space_id.clone(),
            entities: self.entities.clone(),
            chunks: self.chunks.clone(),
            mentions: self.mentions.clone(),
            facts: self.facts.clone(),
        };
        raw.validate().map_err(|error| {
            conflict(format!(
                "M26 deterministic projection invariant failed: {error}"
            ))
        })?;
        let semantic = raw
            .semantic_facts()
            .map_err(|error| {
                conflict(format!(
                    "M26 projection endpoint validation failed: {error}"
                ))
            })?
            .into_iter()
            .map(|fact| VerifiedGraphFact {
                source: fact.source,
                relation: fact.relation,
                target: fact.target,
                evidence_chunk_id: fact.evidence_chunk_id,
            })
            .collect::<Vec<_>>();
        if semantic != self.semantic_facts {
            return Err(conflict(
                "M26 full occurrence projection does not match its semantic facts",
            ));
        }
        Ok(())
    }
}
