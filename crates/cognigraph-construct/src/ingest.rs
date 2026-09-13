//! Ingestion: chunks → backend, with grounded, evidence-bound fact edges.

use std::collections::{BTreeMap, HashSet};

use cognigraph_core::{BatchOp, CogniGraphError, CollectionType, GraphBackend, Result};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::evidence::{canonical_chunks, canonical_text};
use crate::grounding::{
    GroundedFact, SEMANTICS_REV, VetoRule, ground_chunk, mentions, relation_semantics_signals,
    sentence_bounds,
};
use crate::types::{Chunk, SpaceType};

/// Advisory review verdicts about grounded facts, keyed by the fact's own key.
///
/// Deliberately a SIDECAR, never fields on the fact edge
/// (decision_pilot_clinical_graph.md, D2): M26 attests that the fact projection
/// is byte-for-byte what the governed configuration produces, and this detector
/// is a heuristic that is expected to keep improving. Folding it into the
/// attested record would make every detector revision invalidate previously
/// signed artifacts. Same reasoning that quarantines `side_views`: a review aid
/// is not a governed fact.
///
/// Only SUSPECT facts get a row — the collection is the review queue. Rows are
/// written in the same atomic batch as the facts they describe, so for a space
/// that has been ingested, "no row" means "analyzed and clean", never "not yet
/// analyzed".
pub const FACT_SEMANTICS_COLLECTION: &str = "fact_semantics";

// The native product has one writer process. Serializing construction here
// keeps the read-current-occurrences -> atomic replacement sequence isolated
// from another concurrent ingest in that process; otherwise two revisions of
// the same chunk could both read the same predecessor and leave one revision's
// occurrence edges behind.
static INGEST_SERIALIZER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Ingest chunks for one space: chunk documents, global entities,
/// MENTIONS edges, and grounded semantic fact occurrences — each occurrence
/// carrying its space, evidence chunk, and trigger phrase (provenance
/// intrinsic). The same canonical triple grounded in different chunks or
/// spaces is stored as a distinct edge occurrence.
///
/// Re-ingesting a chunk is an atomic replacement: its stored text, mentions,
/// and narrative fact occurrences change together. Facts grounded by other
/// chunks are independent edges and remain intact. Construction therefore
/// requires a backend that explicitly provides atomic batches.
/// Chunk text is normalized to NFC before grounding, content hashing, and
/// occurrence-key generation. Evidence byte spans index that stored text.
/// `vetoes` (from accepted relation_blocker neurons, see
/// `effective_vetoes`) suppress grounding per chunk; pass `&[]` when no
/// blockers are in play. Returns the number of grounded fact edges created.
pub async fn ingest_chunks(
    backend: &dyn GraphBackend,
    space_id: &str,
    config: &SpaceType,
    chunks: &[Chunk],
    vetoes: &[VetoRule],
) -> Result<usize> {
    ingest_chunks_grounded_by(backend, space_id, config, chunks, &|chunk| {
        ground_chunk(&chunk.id, &chunk.text, config, vetoes)
    })
    .await
}

/// The mode-independent half of ingestion: reconciliation, entity and
/// mention upserts, fact and semantics-sidecar writes, one atomic batch.
/// `grounder` supplies the facts for a chunk — rule grounding for the
/// classic path, gated LLM proposals for directed construction (D12).
/// Everything downstream of it (occurrence keys, evidence offsets, the
/// delete-and-rebuild contract) is identical by construction.
/// The callback receives NFC chunk text. Its spans must refer to those bytes;
/// precomputed spans into a different representation are rejected before the batch.
pub async fn ingest_chunks_grounded_by(
    backend: &dyn GraphBackend,
    space_id: &str,
    config: &SpaceType,
    chunks: &[Chunk],
    grounder: &(dyn Fn(&Chunk) -> Vec<GroundedFact> + Sync),
) -> Result<usize> {
    if !backend.supports_atomic_batches() {
        return Err(CogniGraphError::BackendError(format!(
            "construction ingestion requires atomic batch support; backend `{}` does not provide it",
            backend.backend_name()
        )));
    }
    if sanitize(space_id).is_empty() {
        return Err(CogniGraphError::ValidationError(
            "space_id must contain at least one ASCII letter or digit".into(),
        ));
    }

    let canonical = canonical_chunks(chunks);
    let chunks = canonical.as_ref();
    let mut chunk_by_key = BTreeMap::new();
    for chunk in chunks {
        if sanitize(&chunk.id).is_empty() {
            return Err(CogniGraphError::ValidationError(format!(
                "chunk id `{}` must contain at least one ASCII letter or digit",
                chunk.id
            )));
        }
        let key = chunk_key(space_id, &chunk.id);
        if chunk_by_key.insert(key.clone(), chunk).is_some() {
            return Err(CogniGraphError::ValidationError(format!(
                "duplicate or colliding chunk id in space `{space_id}`: `{}` maps to `{key}`",
                chunk.id
            )));
        }
    }

    let mut entities_by_key = BTreeMap::new();
    for entity in &config.entities {
        let key = entity_key(&entity.name);
        if key.is_empty() {
            return Err(CogniGraphError::ValidationError(format!(
                "entity name `{}` must contain at least one ASCII letter or digit",
                entity.name
            )));
        }
        if let Some(existing) = entities_by_key.insert(key.clone(), entity)
            && (existing.name != entity.name || existing.entity_type != entity.entity_type)
        {
            return Err(CogniGraphError::ValidationError(format!(
                "distinct entity definitions `{}` ({}) and `{}` ({}) map to the same key `{key}`",
                existing.name, existing.entity_type, entity.name, entity.entity_type
            )));
        }
    }

    // Hold this through the snapshot reads and the replacement transaction.
    // Unsupported backends and malformed requests fail before entering the
    // critical section or creating collections.
    let _ingest_guard = INGEST_SERIALIZER.lock().await;

    backend
        .ensure_collection("chunks", CollectionType::Document)
        .await?;
    backend
        .ensure_collection("entities", CollectionType::Document)
        .await?;
    backend
        .ensure_collection("mentions", CollectionType::Edge)
        .await?;
    backend
        .ensure_collection("facts", CollectionType::Edge)
        .await?;
    backend
        .ensure_collection(FACT_SEMANTICS_COLLECTION, CollectionType::Document)
        .await?;

    // A readable sanitized key is not collision-free. New occurrence-v1
    // chunks carry both raw identifiers, so validate the stored identity
    // before a Replace. Older chunks did not retain chunk_id and cannot be
    // distinguished safely from a colliding id; fail closed and require a
    // derived-construction rebuild instead of guessing.
    let mut existing_chunk_keys = HashSet::new();
    for (key, chunk) in &chunk_by_key {
        let Some(stored) = backend.get_document("chunks", key).await? else {
            continue;
        };
        let stored_space = stored.get("space_id").and_then(serde_json::Value::as_str);
        let stored_chunk = stored.get("chunk_id").and_then(serde_json::Value::as_str);
        if stored_space != Some(space_id) || stored_chunk != Some(chunk.id.as_str()) {
            let found = match (stored_space, stored_chunk) {
                (Some(found_space), Some(found_chunk)) => {
                    format!("space `{found_space}`, chunk `{found_chunk}`")
                }
                _ => "a legacy chunk without complete raw identity".into(),
            };
            return Err(CogniGraphError::ValidationError(format!(
                "chunk key collision at `{key}`: requested space `{space_id}`, chunk `{}` but found {found}; rebuild legacy chunks/mentions/facts before re-ingesting",
                chunk.id
            )));
        }
        existing_chunk_keys.insert(key.clone());
    }

    let target_chunk_ids: HashSet<&str> = chunks.iter().map(|chunk| chunk.id.as_str()).collect();
    let target_chunk_vertices: HashSet<String> = chunk_by_key
        .keys()
        .map(|key| format!("chunks/{key}"))
        .collect();

    // Read the current occurrence set before constructing one all-or-nothing
    // replacement batch. Legacy upserted edges are selected by provenance
    // fields too, so the first revision under this model cleans them up.
    let old_mentions = backend.list_documents("mentions", None, None).await?;
    let old_facts = backend.list_documents("facts", None, None).await?;
    let old_semantics = backend
        .list_documents(FACT_SEMANTICS_COLLECTION, None, None)
        .await?;

    let mut ops = Vec::new();

    // Global entities retain first-definition-wins aliases, but canonical
    // name/type identity must agree across spaces; a sanitized-key collision
    // is an error rather than a silent semantic merge.
    for (key, entity) in entities_by_key {
        match backend.get_document("entities", &key).await? {
            Some(stored)
                if stored.get("name").and_then(serde_json::Value::as_str)
                    != Some(entity.name.as_str())
                    || stored
                        .get("entity_type")
                        .and_then(serde_json::Value::as_str)
                        != Some(entity.entity_type.as_str()) =>
            {
                return Err(CogniGraphError::ValidationError(format!(
                    "entity key collision at `{key}`: stored identity does not match `{}` ({})",
                    entity.name, entity.entity_type
                )));
            }
            Some(_) => {}
            None => ops.push(BatchOp::Insert {
                collection: "entities".into(),
                doc: json!({
                    "_key": key,
                    "name": entity.name,
                    "entity_type": entity.entity_type,
                    "aliases": entity.aliases,
                }),
            }),
        }
    }

    for edge in old_mentions.iter().filter(|edge| {
        edge.get("space_id").and_then(|v| v.as_str()) == Some(space_id)
            && edge
                .get("_from")
                .and_then(|v| v.as_str())
                .is_some_and(|from| target_chunk_vertices.contains(from))
    }) {
        ops.push(delete_stored("mentions", edge)?);
    }
    for edge in old_facts.iter().filter(|edge| {
        edge.get("space_id").and_then(|v| v.as_str()) == Some(space_id)
            && edge
                .get("evidence_chunk_id")
                .and_then(|v| v.as_str())
                .is_some_and(|chunk_id| target_chunk_ids.contains(chunk_id))
    }) {
        ops.push(delete_stored("facts", edge)?);
    }
    // The sidecar is rebuilt with the facts it describes: same space, same
    // chunks, same atomic batch, so a verdict can never outlive its fact.
    for row in old_semantics.iter().filter(|row| {
        row.get("space_id").and_then(|v| v.as_str()) == Some(space_id)
            && row
                .get("evidence_chunk_id")
                .and_then(|v| v.as_str())
                .is_some_and(|chunk_id| target_chunk_ids.contains(chunk_id))
    }) {
        ops.push(delete_stored(FACT_SEMANTICS_COLLECTION, row)?);
    }

    let mut grounded_edges = 0usize;
    let mut new_mention_keys = HashSet::new();
    let mut new_fact_keys = HashSet::new();
    for (chunk_key, chunk) in chunk_by_key {
        let chunk_doc = json!({
            "_key": chunk_key,
            "space_id": space_id,
            "chunk_id": chunk.id,
            "title": chunk.title,
            "text": chunk.text,
            "content_hash": content_hash(&chunk.text),
            "construction_schema": "occurrence-v1",
        });
        if existing_chunk_keys.contains(&chunk_key) {
            ops.push(BatchOp::Replace {
                collection: "chunks".into(),
                key: chunk_key.clone(),
                doc: chunk_doc,
            });
        } else {
            ops.push(BatchOp::Insert {
                collection: "chunks".into(),
                doc: chunk_doc,
            });
        }

        for entity in mentions(&chunk.text, &config.entities) {
            let entity_key = entity_key(&entity.name);
            let key = occurrence_key("mention", &[space_id, &chunk.id, &entity_key]);
            if !new_mention_keys.insert(key.clone()) {
                continue;
            }
            ops.push(BatchOp::Insert {
                collection: "mentions".into(),
                doc: json!({
                    "_key": key,
                    "_from": format!("chunks/{chunk_key}"),
                    "_to": format!("entities/{entity_key}"),
                    "relation_type": "MENTIONS",
                    "space_id": space_id,
                    "evidence_chunk_id": chunk.id,
                    "construction_schema": "occurrence-v1",
                }),
            });
        }

        for mut grounded in grounder(chunk) {
            // Native stamps strings as NFC. Derive and validate evidence in
            // that representation before hashing offsets into occurrence keys.
            grounded.trigger = canonical_text(&grounded.trigger).into_owned();
            let (start, end) = grounded.trigger_span;
            if grounded.chunk_id != chunk.id
                || start >= end
                || chunk
                    .text
                    .get(start..end)
                    .is_none_or(|text| text.to_lowercase() != grounded.trigger.to_lowercase())
            {
                return Err(CogniGraphError::ValidationError(format!(
                    "invalid evidence span for chunk `{}`: trigger must match a non-empty UTF-8 range in canonical NFC text",
                    chunk.id
                )));
            }
            let source_key = entity_key(&grounded.fact.source);
            let target_key = entity_key(&grounded.fact.target);
            let key = occurrence_key(
                "fact",
                &[
                    space_id,
                    &chunk.id,
                    &source_key,
                    &grounded.fact.relation,
                    &target_key,
                    &grounded.trigger_span.0.to_string(),
                    &grounded.trigger_span.1.to_string(),
                ],
            );
            if !new_fact_keys.insert(key.clone()) {
                continue;
            }
            // Advisory review verdict, written alongside the fact but into the
            // quarantined sidecar — never onto the attested fact record.
            let (sentence_start, sentence_end) =
                sentence_bounds(&chunk.text, grounded.trigger_span.0);
            let signals = relation_semantics_signals(
                &grounded.fact.source,
                &grounded.fact.relation,
                &grounded.fact.target,
                &config.entities,
                &chunk.text[sentence_start..sentence_end],
            );
            if !signals.is_empty() {
                ops.push(BatchOp::Insert {
                    collection: FACT_SEMANTICS_COLLECTION.into(),
                    doc: json!({
                        "_key": key,
                        "fact_key": key,
                        "space_id": space_id,
                        "evidence_chunk_id": grounded.chunk_id,
                        "suspect": true,
                        "signals": signals,
                        "detector_rev": SEMANTICS_REV,
                    }),
                });
            }
            ops.push(BatchOp::Insert {
                collection: "facts".into(),
                doc: json!({
                    "_key": key,
                    "_from": format!("entities/{source_key}"),
                    "_to": format!("entities/{target_key}"),
                    "relation_type": grounded.fact.relation,
                    "space_id": space_id,
                    "evidence_chunk_id": grounded.chunk_id,
                    "trigger": grounded.trigger,
                    "trigger_start": grounded.trigger_span.0,
                    "trigger_end": grounded.trigger_span.1,
                    "construction_schema": "occurrence-v1",
                    // Evidence channel (decision_dailymed_ontology.md, D2):
                    // narrative facts carry chunk evidence; mechanically
                    // imported facts write "structured". Absent = narrative.
                    "provenance": "narrative",
                    // Review attribution, stamped at construction time. The
                    // fact → rule → reviewer chain is a stored link.
                    "neuron_id": grounded.licensed_by_neuron,
                    "reviewed_by": grounded.reviewed_by,
                }),
            });
            grounded_edges += 1;
        }
    }

    backend.execute_batch(ops).await?;
    Ok(grounded_edges)
}

fn delete_stored(collection: &str, value: &serde_json::Value) -> Result<BatchOp> {
    let key = value.get("_key").and_then(|v| v.as_str()).ok_or_else(|| {
        CogniGraphError::ValidationError(format!(
            "stored {collection} occurrence is missing a string `_key`"
        ))
    })?;
    Ok(BatchOp::Delete {
        collection: collection.into(),
        key: key.into(),
    })
}

pub(crate) fn occurrence_key(kind: &str, parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    for part in parts {
        digest.update([0]);
        digest.update(part.as_bytes());
    }
    format!("{kind}-{}", hex_digest(digest.finalize()))
}

pub(crate) fn content_hash(text: &str) -> String {
    format!("sha256:{}", hex_digest(Sha256::digest(text.as_bytes())))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[usize::from(byte >> 4)] as char);
        encoded.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    encoded
}

pub fn entity_key(name: &str) -> String {
    sanitize(name)
}

/// The `chunks` collection key for a space's chunk — the reverse lookup
/// from a fact edge's `evidence_chunk_id` back to the stored chunk text
/// (used by the evidence-augmented answer trace).
pub fn chunk_key(space_id: &str, chunk_id: &str) -> String {
    format!("{}-{}", sanitize(space_id), sanitize(chunk_id))
}

pub(crate) fn sanitize(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
