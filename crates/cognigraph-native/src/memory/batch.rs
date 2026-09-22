//! Atomic batch execution for the `GraphBackend` impl — synchronous inherent
//! method; `backend.rs` delegates to it.

use cognigraph_core::{BatchOp, CogniGraphError, CollectionType, Result};
use serde_json::Value;

use crate::storage::StoreOp;

use super::helpers::{collection_mut, document_key, merge_json, stamp_document, stamp_edge};
use super::{NativeBackend, NativeState, VectorMode, indexes};

impl NativeBackend {
    /// Atomic batch: everything validated and prepared under the write
    /// lock, persisted in ONE redb transaction, and only then applied to
    /// memory/cache/sidecar — all-or-nothing by construction.
    pub(super) fn execute_batch_impl(&self, ops: Vec<BatchOp>) -> Result<Vec<Value>> {
        enum Effect {
            Put { doc: Value },
            Delete,
        }
        let mut state = self.write_state()?;

        // Phase 1: validate every op and compute its final document. Unique
        // index deltas are computed in order against a batch shadow so an
        // earlier op's freed value is usable by a later one (CG-86).
        let mut planned: Vec<(String, String, CollectionType, Effect)> = Vec::new();
        let mut deltas: Vec<(indexes::Entries, indexes::Entries)> = Vec::new();
        let mut shadow: indexes::Shadow = Default::default();
        let collection_type = |state: &NativeState, collection: &str| -> CollectionType {
            state
                .collection_types
                .get(collection)
                .copied()
                .unwrap_or(CollectionType::Document)
        };
        let stamp = |collection: &str,
                     key: &str,
                     collection_type: CollectionType,
                     value: Value|
         -> Result<Value> {
            if collection_type == CollectionType::Edge {
                let object = value.as_object().ok_or_else(|| {
                    CogniGraphError::ValidationError("edge must be a JSON object".into())
                })?;
                for field in ["_from", "_to"] {
                    if object.get(field).and_then(Value::as_str).is_none() {
                        return Err(CogniGraphError::ValidationError(format!(
                            "edge missing {field}"
                        )));
                    }
                }
                Ok(stamp_edge(collection, key, value))
            } else {
                Ok(stamp_document(collection, key, value))
            }
        };
        let key_exists = |state: &NativeState,
                          planned: &[(String, String, CollectionType, Effect)],
                          collection: &str,
                          key: &str|
         -> bool {
            // Later ops see earlier ops' effects within the batch.
            for (c, k, _, effect) in planned.iter().rev() {
                if c == collection && k == key {
                    return matches!(effect, Effect::Put { .. });
                }
            }
            if self.paged() {
                state
                    .keys
                    .get(collection)
                    .is_some_and(|keys| keys.contains(key))
            } else {
                state
                    .collections
                    .get(collection)
                    .is_some_and(|docs| docs.contains_key(key))
            }
        };
        let base_doc = |state: &NativeState,
                        planned: &[(String, String, CollectionType, Effect)],
                        collection: &str,
                        key: &str|
         -> Result<Option<Value>> {
            for (c, k, _, effect) in planned.iter().rev() {
                if c == collection && k == key {
                    return Ok(match effect {
                        Effect::Put { doc } => Some(doc.clone()),
                        Effect::Delete => None,
                    });
                }
            }
            if (self.paged() || self.vector_mode == VectorMode::Sidecar)
                && let Some(store) = &self.store
            {
                return store.get_document_raw(collection, key);
            }
            Ok(state
                .collections
                .get(collection)
                .and_then(|docs| docs.get(key))
                .cloned())
        };

        for op in &ops {
            match op {
                BatchOp::Insert { collection, doc } => {
                    let collection_type = collection_type(&state, collection);
                    let key = document_key(doc);
                    if key_exists(&state, &planned, collection, &key) {
                        return Err(CogniGraphError::DocumentConflict(format!(
                            "{collection}/{key}"
                        )));
                    }
                    let doc = stamp(collection, &key, collection_type, doc.clone())?;
                    let added = indexes::check(&state, &shadow, collection, &key, &doc)?;
                    indexes::shadow_write(&mut shadow, collection, &key, &indexes::NONE, &added);
                    deltas.push((Vec::new(), added));
                    planned.push((
                        collection.clone(),
                        key,
                        collection_type,
                        Effect::Put { doc },
                    ));
                }
                BatchOp::Update {
                    collection,
                    key,
                    merge,
                } => {
                    let collection_type = collection_type(&state, collection);
                    if !key_exists(&state, &planned, collection, key) {
                        return Err(CogniGraphError::DocumentNotFound {
                            collection: collection.clone(),
                            key: key.clone(),
                        });
                    }
                    let mut updated =
                        base_doc(&state, &planned, collection, key)?.ok_or_else(|| {
                            CogniGraphError::DocumentNotFound {
                                collection: collection.clone(),
                                key: key.clone(),
                            }
                        })?;
                    let removed = indexes::entries_of(&state, collection, &updated);
                    merge_json(&mut updated, merge.clone());
                    let doc = stamp(collection, key, collection_type, updated)?;
                    let added = indexes::check(&state, &shadow, collection, key, &doc)?;
                    indexes::shadow_write(&mut shadow, collection, key, &removed, &added);
                    deltas.push((removed, added));
                    planned.push((
                        collection.clone(),
                        key.clone(),
                        collection_type,
                        Effect::Put { doc },
                    ));
                }
                BatchOp::Replace {
                    collection,
                    key,
                    doc,
                } => {
                    let collection_type = collection_type(&state, collection);
                    if !key_exists(&state, &planned, collection, key) {
                        return Err(CogniGraphError::DocumentNotFound {
                            collection: collection.clone(),
                            key: key.clone(),
                        });
                    }
                    let removed = base_doc(&state, &planned, collection, key)?
                        .map(|old| indexes::entries_of(&state, collection, &old))
                        .unwrap_or_default();
                    let doc = stamp(collection, key, collection_type, doc.clone())?;
                    let added = indexes::check(&state, &shadow, collection, key, &doc)?;
                    indexes::shadow_write(&mut shadow, collection, key, &removed, &added);
                    deltas.push((removed, added));
                    planned.push((
                        collection.clone(),
                        key.clone(),
                        collection_type,
                        Effect::Put { doc },
                    ));
                }
                BatchOp::Delete { collection, key } => {
                    let collection_type = collection_type(&state, collection);
                    if !key_exists(&state, &planned, collection, key) {
                        return Err(CogniGraphError::DocumentNotFound {
                            collection: collection.clone(),
                            key: key.clone(),
                        });
                    }
                    let removed = base_doc(&state, &planned, collection, key)?
                        .map(|old| indexes::entries_of(&state, collection, &old))
                        .unwrap_or_default();
                    indexes::shadow_write(&mut shadow, collection, key, &removed, &indexes::NONE);
                    deltas.push((removed, Vec::new()));
                    planned.push((
                        collection.clone(),
                        key.clone(),
                        collection_type,
                        Effect::Delete,
                    ));
                }
            }
        }

        // Phase 2: one committed transaction for the whole batch.
        let implicit_collections: std::collections::BTreeSet<_> = planned
            .iter()
            .filter(|(collection, _, _, _)| !state.collection_types.contains_key(collection))
            .map(|(collection, _, _, _)| collection.clone())
            .collect();
        let mut store_ops: Vec<StoreOp<'_>> = implicit_collections
            .iter()
            .map(|collection| StoreOp::PutCollection {
                name: collection,
                collection_type: CollectionType::Document,
            })
            .collect();
        store_ops.extend(
            planned
                .iter()
                .map(|(collection, key, _, effect)| match effect {
                    Effect::Put { doc } => StoreOp::PutDocument {
                        collection,
                        key,
                        doc,
                    },
                    Effect::Delete => StoreOp::DeleteDocument { collection, key },
                }),
        );
        for ((collection, key, _, _), (removed, added)) in planned.iter().zip(&deltas) {
            store_ops.extend(indexes::ops(collection, key, removed, added));
        }
        self.persist(&store_ops)?;

        // Phase 3: apply to memory, cache, and sidecar delta.
        let mut results = Vec::with_capacity(planned.len());
        for ((collection_name, key, collection_type, effect), (removed, added)) in
            planned.iter().zip(&deltas)
        {
            self.triples_invalidate(collection_name);
            indexes::apply(&mut state, collection_name, key, removed, added);
            match effect {
                Effect::Put { doc } => {
                    let stored = self.strip_embedding_if_sidecar(doc.clone());
                    if self.paged() {
                        state
                            .keys
                            .entry(collection_name.clone())
                            .or_default()
                            .insert(key.clone());
                        self.cache_put(collection_name, key, &stored);
                    } else {
                        collection_mut(&mut state, collection_name, *collection_type)
                            .insert(key.clone(), stored.clone());
                    }
                    self.sidecar_apply_write(collection_name, key, Some(doc));
                    results.push(stored);
                }
                Effect::Delete => {
                    if self.paged() {
                        if let Some(keys) = state.keys.get_mut(collection_name) {
                            keys.remove(key);
                        }
                        self.cache_remove(collection_name, key);
                    } else if let Some(docs) = state.collections.get_mut(collection_name) {
                        docs.remove(key);
                    }
                    self.sidecar_apply_write(collection_name, key, None);
                    results.push(Value::Null);
                }
            }
        }
        Ok(results)
    }
}
