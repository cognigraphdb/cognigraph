//! Pure, bounded projection of one complete governed construction target.
//!
//! Unlike [`crate::derive_fact_rows`], which deliberately retains only the
//! evidence-bearing semantic rows used by evaluation, this projection carries
//! every logical document needed to stage the target's `entities`, `chunks`,
//! `mentions`, and `facts` collections. It performs no backend writes and adds
//! no backend-generated metadata (`_id`, timestamps, or confidence).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::evidence::canonical_chunks;
use crate::ingest::{chunk_key, content_hash, entity_key, occurrence_key, sanitize};
use crate::{Chunk, DerivedFactRow, SpaceType, VetoRule, ground_chunk, mentions};

const CONSTRUCTION_SCHEMA: &str = "occurrence-v1";
const NARRATIVE_PROVENANCE: &str = "narrative";

/// Resource limits and cooperative scheduling for one materialization.
///
/// The caller is expected to apply its corpus/configuration byte and grounding
/// work budgets before this pure projection. These limits independently bound
/// every materialized output vector and the M22-compatible semantic fact set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterializationOptions {
    pub max_entity_count: usize,
    pub max_chunk_count: usize,
    pub max_mention_count: usize,
    pub max_fact_occurrence_count: usize,
    pub max_semantic_fact_count: usize,
    pub yield_every_chunks: usize,
}

/// One logical `entities` document, before backend-generated metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedEntityRow {
    #[serde(rename = "_key")]
    pub key: String,
    pub name: String,
    pub entity_type: String,
    pub aliases: Vec<String>,
}

/// One logical `chunks` document, before backend-generated metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedChunkRow {
    #[serde(rename = "_key")]
    pub key: String,
    pub space_id: String,
    pub chunk_id: String,
    pub title: String,
    pub text: String,
    pub content_hash: String,
    pub construction_schema: String,
}

/// One logical `mentions` edge, before backend-generated metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedMentionRow {
    #[serde(rename = "_key")]
    pub key: String,
    #[serde(rename = "_from")]
    pub from: String,
    #[serde(rename = "_to")]
    pub to: String,
    pub relation_type: String,
    pub space_id: String,
    pub evidence_chunk_id: String,
    pub construction_schema: String,
}

/// One complete logical `facts` occurrence, before backend-generated metadata.
///
/// Trigger byte offsets and review attribution intentionally remain distinct
/// even when evaluation later collapses several occurrences to one semantic
/// `(source, relation, target, evidence_chunk_id)` row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedFactOccurrenceRow {
    #[serde(rename = "_key")]
    pub key: String,
    #[serde(rename = "_from")]
    pub from: String,
    #[serde(rename = "_to")]
    pub to: String,
    pub relation_type: String,
    pub space_id: String,
    pub evidence_chunk_id: String,
    pub trigger: String,
    pub trigger_start: usize,
    pub trigger_end: usize,
    pub construction_schema: String,
    pub provenance: String,
    pub neuron_id: Option<String>,
    pub reviewed_by: Option<String>,
}

/// Sorted, unique logical rows for one complete materialization target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedGraphProjection {
    pub space_id: String,
    pub entities: Vec<MaterializedEntityRow>,
    pub chunks: Vec<MaterializedChunkRow>,
    pub mentions: Vec<MaterializedMentionRow>,
    pub facts: Vec<MaterializedFactOccurrenceRow>,
}

impl MaterializedGraphProjection {
    /// Validate every invariant that can be derived from the closed projection
    /// itself, without trusting its serialized keys or denormalized links.
    ///
    /// This proves internal occurrence integrity. It deliberately does not
    /// claim that the chunk bytes came from a particular external corpus or
    /// that the facts came from a particular governed configuration; callers
    /// must bind those external authorities separately.
    pub fn validate(&self) -> Result<(), MaterializationError> {
        if sanitize(&self.space_id).is_empty() {
            return Err(MaterializationError::InvalidSpaceId);
        }

        validate_sorted_unique_keys("entities", self.entities.iter().map(|row| row.key.as_str()))?;
        validate_sorted_unique_keys("chunks", self.chunks.iter().map(|row| row.key.as_str()))?;
        validate_sorted_unique_keys("mentions", self.mentions.iter().map(|row| row.key.as_str()))?;
        validate_sorted_unique_keys("facts", self.facts.iter().map(|row| row.key.as_str()))?;

        let mut entities_by_key = BTreeMap::<&str, &MaterializedEntityRow>::new();
        for entity in &self.entities {
            if sanitize(&entity.name).is_empty() {
                return Err(invalid_row(
                    "entities",
                    &entity.key,
                    "entity name has no deterministic key",
                ));
            }
            if entity.key != entity_key(&entity.name) {
                return Err(invalid_row(
                    "entities",
                    &entity.key,
                    "entity key does not match its canonical name",
                ));
            }
            entities_by_key.insert(entity.key.as_str(), entity);
        }

        let mut chunks_by_id = BTreeMap::<&str, &MaterializedChunkRow>::new();
        for chunk in &self.chunks {
            if sanitize(&chunk.chunk_id).is_empty() {
                return Err(invalid_row(
                    "chunks",
                    &chunk.key,
                    "chunk id has no deterministic key",
                ));
            }
            if chunk.space_id != self.space_id {
                return Err(invalid_row(
                    "chunks",
                    &chunk.key,
                    "chunk space_id differs from the projection",
                ));
            }
            if chunk.key != chunk_key(&self.space_id, &chunk.chunk_id) {
                return Err(invalid_row(
                    "chunks",
                    &chunk.key,
                    "chunk key does not match its space and raw id",
                ));
            }
            if !unicode_normalization::is_nfc(&chunk.text) {
                return Err(invalid_row(
                    "chunks",
                    &chunk.key,
                    "chunk text must be canonical NFC",
                ));
            }
            if chunk.content_hash != content_hash(&chunk.text) {
                return Err(invalid_row(
                    "chunks",
                    &chunk.key,
                    "chunk content_hash does not match its text",
                ));
            }
            if chunk.construction_schema != CONSTRUCTION_SCHEMA {
                return Err(invalid_row(
                    "chunks",
                    &chunk.key,
                    "chunk construction_schema is unsupported",
                ));
            }
            if chunks_by_id
                .insert(chunk.chunk_id.as_str(), chunk)
                .is_some()
            {
                return Err(invalid_row(
                    "chunks",
                    &chunk.key,
                    "raw chunk id is not unique",
                ));
            }
        }

        // Mentions are completely derivable from the projected chunks and
        // entities, including aliases. Comparing the whole sorted row vector
        // catches both forged rows and omitted required occurrences.
        let mut expected_mentions = BTreeMap::<String, MaterializedMentionRow>::new();
        for chunk in &self.chunks {
            let text_cf = chunk.text.to_lowercase();
            for entity in &self.entities {
                let mentioned = std::iter::once(&entity.name)
                    .chain(entity.aliases.iter())
                    .any(|surface| text_cf.contains(&surface.to_lowercase()));
                if !mentioned {
                    continue;
                }
                let key = occurrence_key(
                    "mention",
                    &[self.space_id.as_str(), &chunk.chunk_id, &entity.key],
                );
                expected_mentions.insert(
                    key.clone(),
                    MaterializedMentionRow {
                        key,
                        from: format!("chunks/{}", chunk.key),
                        to: format!("entities/{}", entity.key),
                        relation_type: "MENTIONS".into(),
                        space_id: self.space_id.clone(),
                        evidence_chunk_id: chunk.chunk_id.clone(),
                        construction_schema: CONSTRUCTION_SCHEMA.into(),
                    },
                );
            }
        }
        if self.mentions != expected_mentions.into_values().collect::<Vec<_>>() {
            return Err(MaterializationError::ProjectionRowsMismatch {
                collection: "mentions",
            });
        }

        for fact in &self.facts {
            if fact.space_id != self.space_id {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact space_id differs from the projection",
                ));
            }
            if fact.construction_schema != CONSTRUCTION_SCHEMA {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact construction_schema is unsupported",
                ));
            }
            if fact.provenance != NARRATIVE_PROVENANCE {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact provenance is unsupported",
                ));
            }
            if fact.reviewed_by.is_some() && fact.neuron_id.is_none() {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "review attribution has no licensing neuron",
                ));
            }
            let source_key = fact.from.strip_prefix("entities/").ok_or_else(|| {
                invalid_row("facts", &fact.key, "fact source is not an entity vertex")
            })?;
            let target_key = fact.to.strip_prefix("entities/").ok_or_else(|| {
                invalid_row("facts", &fact.key, "fact target is not an entity vertex")
            })?;
            if !entities_by_key.contains_key(source_key) {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact source entity is absent",
                ));
            }
            if !entities_by_key.contains_key(target_key) {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact target entity is absent",
                ));
            }
            let chunk = chunks_by_id
                .get(fact.evidence_chunk_id.as_str())
                .ok_or_else(|| invalid_row("facts", &fact.key, "fact evidence chunk is absent"))?;
            let Some(trigger_text) = chunk.text.get(fact.trigger_start..fact.trigger_end) else {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact trigger span is not a valid UTF-8 text range",
                ));
            };
            if fact.trigger.is_empty()
                || fact.trigger_start >= fact.trigger_end
                || trigger_text.to_lowercase() != fact.trigger.to_lowercase()
            {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact trigger does not match its evidence span",
                ));
            }
            let start = fact.trigger_start.to_string();
            let end = fact.trigger_end.to_string();
            let expected_key = occurrence_key(
                "fact",
                &[
                    self.space_id.as_str(),
                    fact.evidence_chunk_id.as_str(),
                    source_key,
                    fact.relation_type.as_str(),
                    target_key,
                    &start,
                    &end,
                ],
            );
            if fact.key != expected_key {
                return Err(invalid_row(
                    "facts",
                    &fact.key,
                    "fact key does not match its occurrence identity",
                ));
            }
        }

        // Reuse the public semantic projection to close endpoint resolution
        // after all stronger row invariants have passed.
        self.semantic_facts()?;
        Ok(())
    }

    /// Collapse full fact occurrences to the exact sorted semantic row shape
    /// used by [`crate::derive_fact_rows`].
    ///
    /// This remains fallible because projection fields are public and the type
    /// is deserializable: a caller must not silently accept a fact whose
    /// endpoint no longer resolves to exactly one projected entity.
    pub fn semantic_facts(&self) -> Result<Vec<DerivedFactRow>, MaterializationError> {
        let mut entity_names = HashMap::<String, String>::with_capacity(self.entities.len());
        for entity in &self.entities {
            let vertex = format!("entities/{}", entity.key);
            if let Some(existing) = entity_names.insert(vertex.clone(), entity.name.clone())
                && existing != entity.name
            {
                return Err(MaterializationError::EntityVertexCollision { vertex });
            }
        }

        let mut rows = BTreeSet::new();
        for fact in &self.facts {
            let source = entity_names.get(&fact.from).ok_or_else(|| {
                MaterializationError::MissingEntityEndpoint {
                    vertex: fact.from.clone(),
                }
            })?;
            let target = entity_names.get(&fact.to).ok_or_else(|| {
                MaterializationError::MissingEntityEndpoint {
                    vertex: fact.to.clone(),
                }
            })?;
            rows.insert(DerivedFactRow {
                source: source.clone(),
                relation: fact.relation_type.clone(),
                target: target.clone(),
                evidence_chunk_id: fact.evidence_chunk_id.clone(),
            });
        }
        Ok(rows.into_iter().collect())
    }
}

/// Closed validation and capacity failures for materialization projection.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MaterializationError {
    #[error("yield_every_chunks must be greater than zero")]
    InvalidYieldInterval,
    #[error("space_id must contain at least one ASCII letter or digit")]
    InvalidSpaceId,
    #[error("chunk id `{chunk_id}` must contain at least one ASCII letter or digit")]
    InvalidChunkId { chunk_id: String },
    #[error("entity name `{name}` must contain at least one ASCII letter or digit")]
    InvalidEntityName { name: String },
    #[error("chunk key collision at `{key}` between `{first_chunk_id}` and `{second_chunk_id}`")]
    ChunkKeyCollision {
        key: String,
        first_chunk_id: String,
        second_chunk_id: String,
    },
    #[error("distinct entity definitions map to the same key `{key}`")]
    EntityKeyCollision { key: String },
    #[error("distinct mention occurrences map to the same key `{key}`")]
    MentionKeyCollision { key: String },
    #[error("distinct fact occurrences map to the same key `{key}`")]
    FactOccurrenceKeyCollision { key: String },
    #[error("grounded fact endpoint `{name}` has no exact entity definition")]
    MissingEntityDefinition { name: String },
    #[error("materialized entity vertex `{vertex}` is ambiguous")]
    EntityVertexCollision { vertex: String },
    #[error("materialized fact endpoint `{vertex}` is absent from the entity projection")]
    MissingEntityEndpoint { vertex: String },
    #[error("materialized `{collection}` rows are not sorted and unique")]
    ProjectionRowsNotSortedUnique { collection: &'static str },
    #[error("materialized `{collection}` rows do not equal their deterministic projection")]
    ProjectionRowsMismatch { collection: &'static str },
    #[error("materialized `{collection}` row `{key}` is invalid: {reason}")]
    InvalidProjectionRow {
        collection: &'static str,
        key: String,
        reason: &'static str,
    },
    #[error("entity count exceeds configured maximum of {max_entity_count}")]
    EntityLimitExceeded { max_entity_count: usize },
    #[error("chunk count exceeds configured maximum of {max_chunk_count}")]
    ChunkLimitExceeded { max_chunk_count: usize },
    #[error("mention count exceeds configured maximum of {max_mention_count}")]
    MentionLimitExceeded { max_mention_count: usize },
    #[error("fact occurrence count exceeds configured maximum of {max_fact_occurrence_count}")]
    FactOccurrenceLimitExceeded { max_fact_occurrence_count: usize },
    #[error("semantic fact count exceeds configured maximum of {max_semantic_fact_count}")]
    SemanticFactLimitExceeded { max_semantic_fact_count: usize },
}

fn validate_sorted_unique_keys<'a>(
    collection: &'static str,
    keys: impl IntoIterator<Item = &'a str>,
) -> Result<(), MaterializationError> {
    let mut previous: Option<&str> = None;
    for key in keys {
        if previous.is_some_and(|previous| previous >= key) {
            return Err(MaterializationError::ProjectionRowsNotSortedUnique { collection });
        }
        previous = Some(key);
    }
    Ok(())
}

fn invalid_row(collection: &'static str, key: &str, reason: &'static str) -> MaterializationError {
    MaterializationError::InvalidProjectionRow {
        collection,
        key: key.into(),
        reason,
    }
}

/// Derive every logical row required to stage one complete construction
/// target, without reading or mutating a graph backend.
///
/// Keys, endpoints, content hashes, grounding, provenance, and attribution use
/// the same algorithms as [`crate::ingest_chunks`]. Output vectors are sorted
/// by key and unique; a same-key/different-row collision fails closed.
/// Evidence text is canonical NFC before grounding or byte-sensitive derivation.
pub async fn derive_materialized_graph(
    space_id: &str,
    chunks: &[Chunk],
    effective_space_type: &SpaceType,
    vetoes: &[VetoRule],
    options: MaterializationOptions,
) -> Result<MaterializedGraphProjection, MaterializationError> {
    if options.yield_every_chunks == 0 {
        return Err(MaterializationError::InvalidYieldInterval);
    }
    if sanitize(space_id).is_empty() {
        return Err(MaterializationError::InvalidSpaceId);
    }
    if effective_space_type.entities.len() > options.max_entity_count {
        return Err(MaterializationError::EntityLimitExceeded {
            max_entity_count: options.max_entity_count,
        });
    }
    if chunks.len() > options.max_chunk_count {
        return Err(MaterializationError::ChunkLimitExceeded {
            max_chunk_count: options.max_chunk_count,
        });
    }

    let canonical = canonical_chunks(chunks);
    let chunks = canonical.as_ref();
    let mut entities = BTreeMap::<String, MaterializedEntityRow>::new();
    let mut entity_keys_by_name = HashMap::<String, String>::new();
    for entity in &effective_space_type.entities {
        let key = entity_key(&entity.name);
        if key.is_empty() {
            return Err(MaterializationError::InvalidEntityName {
                name: entity.name.clone(),
            });
        }
        let row = MaterializedEntityRow {
            key: key.clone(),
            name: entity.name.clone(),
            entity_type: entity.entity_type.clone(),
            aliases: entity.aliases.clone(),
        };
        if let Some(existing) = entities.get(&key)
            && existing != &row
        {
            return Err(MaterializationError::EntityKeyCollision { key });
        }
        entities.entry(key.clone()).or_insert(row);
        entity_keys_by_name.insert(entity.name.clone(), key);
    }

    let mut chunk_rows = BTreeMap::<String, MaterializedChunkRow>::new();
    for chunk in chunks {
        if sanitize(&chunk.id).is_empty() {
            return Err(MaterializationError::InvalidChunkId {
                chunk_id: chunk.id.clone(),
            });
        }
        let key = chunk_key(space_id, &chunk.id);
        let row = MaterializedChunkRow {
            key: key.clone(),
            space_id: space_id.to_string(),
            chunk_id: chunk.id.clone(),
            title: chunk.title.clone(),
            text: chunk.text.clone(),
            content_hash: content_hash(&chunk.text),
            construction_schema: CONSTRUCTION_SCHEMA.into(),
        };
        if let Some(existing) = chunk_rows.insert(key.clone(), row) {
            return Err(MaterializationError::ChunkKeyCollision {
                key,
                first_chunk_id: existing.chunk_id,
                second_chunk_id: chunk.id.clone(),
            });
        }
    }

    let mut mention_rows = BTreeMap::<String, MaterializedMentionRow>::new();
    let mut fact_rows = BTreeMap::<String, MaterializedFactOccurrenceRow>::new();
    let mut semantic_facts = BTreeSet::<DerivedFactRow>::new();

    for (index, chunk) in chunk_rows.values().enumerate() {
        for entity in mentions(&chunk.text, &effective_space_type.entities) {
            let entity_key = entity_keys_by_name.get(&entity.name).ok_or_else(|| {
                MaterializationError::MissingEntityDefinition {
                    name: entity.name.clone(),
                }
            })?;
            let key = occurrence_key("mention", &[space_id, &chunk.chunk_id, entity_key]);
            let row = MaterializedMentionRow {
                key: key.clone(),
                from: format!("chunks/{}", chunk.key),
                to: format!("entities/{entity_key}"),
                relation_type: "MENTIONS".into(),
                space_id: space_id.to_string(),
                evidence_chunk_id: chunk.chunk_id.clone(),
                construction_schema: CONSTRUCTION_SCHEMA.into(),
            };
            match mention_rows.get(&key) {
                Some(existing) if existing != &row => {
                    return Err(MaterializationError::MentionKeyCollision { key });
                }
                Some(_) => {}
                None => {
                    mention_rows.insert(key, row);
                    if mention_rows.len() > options.max_mention_count {
                        return Err(MaterializationError::MentionLimitExceeded {
                            max_mention_count: options.max_mention_count,
                        });
                    }
                }
            }
        }

        for grounded in ground_chunk(&chunk.chunk_id, &chunk.text, effective_space_type, vetoes) {
            let source_key = entity_keys_by_name
                .get(&grounded.fact.source)
                .ok_or_else(|| MaterializationError::MissingEntityDefinition {
                    name: grounded.fact.source.clone(),
                })?;
            let target_key = entity_keys_by_name
                .get(&grounded.fact.target)
                .ok_or_else(|| MaterializationError::MissingEntityDefinition {
                    name: grounded.fact.target.clone(),
                })?;
            let key = occurrence_key(
                "fact",
                &[
                    space_id,
                    &chunk.chunk_id,
                    source_key,
                    &grounded.fact.relation,
                    target_key,
                    &grounded.trigger_span.0.to_string(),
                    &grounded.trigger_span.1.to_string(),
                ],
            );
            let semantic = DerivedFactRow {
                source: grounded.fact.source,
                relation: grounded.fact.relation.clone(),
                target: grounded.fact.target,
                evidence_chunk_id: grounded.chunk_id.clone(),
            };
            let row = MaterializedFactOccurrenceRow {
                key: key.clone(),
                from: format!("entities/{source_key}"),
                to: format!("entities/{target_key}"),
                relation_type: grounded.fact.relation,
                space_id: space_id.to_string(),
                evidence_chunk_id: grounded.chunk_id,
                trigger: grounded.trigger,
                trigger_start: grounded.trigger_span.0,
                trigger_end: grounded.trigger_span.1,
                construction_schema: CONSTRUCTION_SCHEMA.into(),
                provenance: NARRATIVE_PROVENANCE.into(),
                neuron_id: grounded.licensed_by_neuron,
                reviewed_by: grounded.reviewed_by,
            };
            match fact_rows.get(&key) {
                Some(existing) if existing != &row => {
                    return Err(MaterializationError::FactOccurrenceKeyCollision { key });
                }
                Some(_) => {}
                None => {
                    fact_rows.insert(key, row);
                    if fact_rows.len() > options.max_fact_occurrence_count {
                        return Err(MaterializationError::FactOccurrenceLimitExceeded {
                            max_fact_occurrence_count: options.max_fact_occurrence_count,
                        });
                    }
                }
            }
            if semantic_facts.insert(semantic)
                && semantic_facts.len() > options.max_semantic_fact_count
            {
                return Err(MaterializationError::SemanticFactLimitExceeded {
                    max_semantic_fact_count: options.max_semantic_fact_count,
                });
            }
        }

        if (index + 1) % options.yield_every_chunks == 0 {
            tokio::task::yield_now().await;
        }
    }

    let projection = MaterializedGraphProjection {
        space_id: space_id.to_string(),
        entities: entities.into_values().collect(),
        chunks: chunk_rows.into_values().collect(),
        mentions: mention_rows.into_values().collect(),
        facts: fact_rows.into_values().collect(),
    };
    // Exercise every closed projection invariant before returning public rows.
    projection.validate()?;
    let projected_semantic = projection.semantic_facts()?;
    debug_assert_eq!(
        projected_semantic,
        semantic_facts.into_iter().collect::<Vec<_>>()
    );
    Ok(projection)
}
