//! Verbatim bulk insert for offline importers (CG-66).
//!
//! Unlike the ordinary write path, documents are stored exactly as given: no
//! `created_at`/`updated_at` stamps and no edge `relation_type`/`confidence`
//! defaults, because an importer must preserve the source's JSON values. Only
//! `_id` is derived from the collection and `_key`. Every document is checked
//! under the single write lock (shape, existing and repeated keys, unique
//! constraints against the store and earlier documents in the call) before
//! one store transaction persists them all; a refusal changes nothing.

use cognigraph_core::{CogniGraphError, CollectionType, Result};
use serde_json::{Value, json};

use crate::storage::StoreOp;

use super::helpers::collection_mut;
use super::{NativeBackend, indexes};

fn prepare(
    collection: &str,
    collection_type: CollectionType,
    mut doc: Value,
) -> Result<(String, Value)> {
    let invalid =
        |message: &str| CogniGraphError::ValidationError(format!("{collection}: {message}"));
    let object = doc
        .as_object_mut()
        .ok_or_else(|| invalid("bulk insert documents must be JSON objects"))?;
    let key = match object.get("_key") {
        Some(Value::String(key)) if !key.is_empty() => key.clone(),
        _ => {
            return Err(invalid(
                "bulk insert documents need a non-empty string `_key`",
            ));
        }
    };
    let id = format!("{collection}/{key}");
    if object.get("_id").is_some_and(|given| given != &json!(id)) {
        return Err(invalid(&format!("`_id` of `{key}` must be `{id}`")));
    }
    if collection_type == CollectionType::Edge {
        for end in ["_from", "_to"] {
            if !object.get(end).is_some_and(Value::is_string) {
                return Err(invalid(&format!("edge `{key}` needs a string `{end}`")));
            }
        }
    }
    object.insert("_id".into(), json!(id));
    Ok((key, doc))
}

impl NativeBackend {
    /// Insert `docs` into an existing collection exactly as given, in one
    /// store transaction. Refuses (and writes nothing) if any document is
    /// malformed, its key already exists or repeats within the call, or it
    /// violates a declared unique constraint.
    pub fn bulk_insert(&self, collection: &str, docs: Vec<Value>) -> Result<()> {
        let mut state = self.write_state()?;
        let collection_type = *state
            .collection_types
            .get(collection)
            .ok_or_else(|| CogniGraphError::CollectionNotFound(collection.to_string()))?;
        let mut shadow: indexes::Shadow = Default::default();
        let mut seen = std::collections::HashSet::new();
        let mut planned = Vec::with_capacity(docs.len());
        for doc in docs {
            let (key, doc) = prepare(collection, collection_type, doc)?;
            let exists = if self.paged() {
                state
                    .keys
                    .get(collection)
                    .is_some_and(|keys| keys.contains(&key))
            } else {
                state
                    .collections
                    .get(collection)
                    .is_some_and(|stored| stored.contains_key(&key))
            };
            if exists || !seen.insert(key.clone()) {
                return Err(CogniGraphError::DocumentConflict(format!(
                    "{collection}/{key}"
                )));
            }
            let added = indexes::check(&state, &shadow, collection, &key, &doc)?;
            indexes::shadow_write(&mut shadow, collection, &key, &indexes::NONE, &added);
            planned.push((key, doc, added));
        }
        if planned.is_empty() {
            return Ok(());
        }

        let mut ops: Vec<StoreOp<'_>> = planned
            .iter()
            .map(|(key, doc, _)| StoreOp::PutDocument {
                collection,
                key,
                doc,
            })
            .collect();
        for (key, _, added) in &planned {
            ops.extend(indexes::ops(collection, key, &indexes::NONE, added));
        }
        self.persist(&ops)?;

        self.triples_invalidate(collection);
        for (key, doc, added) in &planned {
            indexes::apply(&mut state, collection, key, &indexes::NONE, added);
            let stored = self.strip_embedding_if_sidecar(doc.clone());
            if self.paged() {
                state
                    .keys
                    .entry(collection.to_string())
                    .or_default()
                    .insert(key.clone());
                self.cache_put(collection, key, &stored);
            } else {
                collection_mut(&mut state, collection, collection_type).insert(key.clone(), stored);
            }
            self.sidecar_apply_write(collection, key, Some(doc));
        }
        Ok(())
    }
}
