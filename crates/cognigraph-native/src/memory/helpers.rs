use std::collections::BTreeMap;

use cognigraph_core::{CogniGraphError, CollectionType, Direction, Result};
use serde_json::{Value, json};
use uuid::Uuid;

use super::NativeState;

pub(super) fn collection_mut<'a>(
    state: &'a mut NativeState,
    name: &str,
    collection_type: CollectionType,
) -> &'a mut BTreeMap<String, Value> {
    state
        .collection_types
        .entry(name.to_string())
        .or_insert(collection_type);
    state.collections.entry(name.to_string()).or_default()
}

pub(super) fn collection<'a>(
    state: &'a NativeState,
    name: &str,
) -> Result<&'a BTreeMap<String, Value>> {
    state
        .collections
        .get(name)
        .ok_or_else(|| CogniGraphError::CollectionNotFound(name.to_string()))
}

/// Reject writes whose collection exists with a contradicting type. Reads
/// stay permissive; absent collections are handled by the caller.
pub(super) fn check_collection_type(
    state: &NativeState,
    name: &str,
    expected: CollectionType,
) -> Result<()> {
    match state.collection_types.get(name) {
        Some(actual) if *actual != expected => Err(CogniGraphError::ValidationError(format!(
            "collection `{name}` is not a {expected:?} collection"
        ))),
        _ => Ok(()),
    }
}

pub(super) fn document_key(doc: &Value) -> String {
    doc.get("_key")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

pub(super) fn edge_key(edge: &Value) -> String {
    edge.get("_key")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

/// Stamp backend metadata without changing caller-owned values. Text
/// canonicalization belongs at explicit ingestion/query boundaries; arbitrary
/// strings can be opaque references, hashes, or signed/custodied payloads.
pub(super) fn stamp_document(collection: &str, key: &str, mut doc: Value) -> Value {
    let now = chrono::Utc::now().to_rfc3339();
    if let Value::Object(obj) = &mut doc {
        obj.insert("_key".into(), json!(key));
        obj.insert("_id".into(), json!(format!("{collection}/{key}")));
        obj.entry("created_at").or_insert_with(|| json!(now));
        obj.insert("updated_at".into(), json!(now));
    }
    doc
}

pub(super) fn stamp_edge(collection: &str, key: &str, mut edge: Value) -> Value {
    if let Value::Object(obj) = &mut edge {
        obj.insert("_key".into(), json!(key));
        obj.insert("_id".into(), json!(format!("{collection}/{key}")));
        obj.entry("relation_type")
            .or_insert_with(|| json!("related_to"));
        obj.entry("confidence").or_insert_with(|| json!(1.0));
    }
    edge
}

pub(super) fn merge_json(base: &mut Value, update: Value) {
    if let (Some(base), Some(update)) = (base.as_object_mut(), update.as_object()) {
        for (key, value) in update {
            base.insert(key.clone(), value.clone());
        }
    }
}

pub(super) fn full_vertex_doc(state: &NativeState, vertex_id: &str) -> Value {
    let Some((collection, key)) = vertex_id.split_once('/') else {
        return json!({ "_id": vertex_id });
    };
    state
        .collections
        .get(collection)
        .and_then(|docs| docs.get(key))
        .cloned()
        .unwrap_or_else(|| json!({ "_id": vertex_id }))
}

pub(super) fn edge_matches(edge: &Value, vertex_id: &str, direction: Direction) -> bool {
    match direction {
        Direction::Outbound => edge.get("_from").and_then(Value::as_str) == Some(vertex_id),
        Direction::Inbound => edge.get("_to").and_then(Value::as_str) == Some(vertex_id),
        Direction::Any => {
            edge.get("_from").and_then(Value::as_str) == Some(vertex_id)
                || edge.get("_to").and_then(Value::as_str) == Some(vertex_id)
        }
    }
}

pub(super) fn next_vertex(edge: &Value, current: &str, direction: Direction) -> Option<String> {
    match direction {
        Direction::Outbound => edge.get("_to").and_then(Value::as_str).map(str::to_string),
        Direction::Inbound => edge
            .get("_from")
            .and_then(Value::as_str)
            .map(str::to_string),
        Direction::Any => {
            let from = edge.get("_from").and_then(Value::as_str)?;
            let to = edge.get("_to").and_then(Value::as_str)?;
            if from == current {
                Some(to.to_string())
            } else {
                Some(from.to_string())
            }
        }
    }
}

pub(super) fn confidence(edge: &Value) -> f64 {
    edge.get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or(1.0)
}

/// Cosine similarity straight over the JSON array — no per-document
/// Vec<f64> allocation on the scoring hot path.
pub(super) fn cosine_from_values(query: &[f64], values: &[Value]) -> Option<f64> {
    if query.is_empty() || query.len() != values.len() {
        return None;
    }
    let mut dot = 0.0;
    let mut doc_norm = 0.0;
    for (q, v) in query.iter().zip(values) {
        let v = v.as_f64()?;
        dot += q * v;
        doc_norm += v * v;
    }
    let query_norm = query.iter().map(|v| v * v).sum::<f64>().sqrt();
    let doc_norm = doc_norm.sqrt();
    if query_norm == 0.0 || doc_norm == 0.0 {
        return None;
    }
    Some(dot / (query_norm * doc_norm))
}
