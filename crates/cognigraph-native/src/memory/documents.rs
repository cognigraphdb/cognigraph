//! Document CRUD bodies for the `GraphBackend` impl — synchronous inherent
//! methods; `backend.rs` delegates to them.

use cognigraph_core::{
    CogniGraphError, CollectionType, DocumentId, FieldPredicate, PredicateOp, Result,
};
use serde_json::Value;
use std::ops::Bound;
use tracing::debug;

use crate::storage::StoreOp;

use cognigraph_core::project_fields;

use super::helpers::{
    check_collection_type, collection, collection_mut, document_key, merge_json, stamp_document,
};
use super::{NativeBackend, VectorMode};

impl NativeBackend {
    pub(super) fn create_document_impl(&self, collection: &str, doc: Value) -> Result<DocumentId> {
        let key = document_key(&doc);
        let doc = stamp_document(collection, &key, doc);
        let mut state = self.write_state()?;
        check_collection_type(&state, collection, CollectionType::Document)?;
        if self.paged() {
            if state
                .keys
                .get(collection)
                .is_some_and(|keys| keys.contains(&key))
            {
                return Err(CogniGraphError::DocumentConflict(format!(
                    "{collection}/{key}"
                )));
            }
            self.persist(&[
                StoreOp::PutCollection {
                    name: collection,
                    collection_type: *state
                        .collection_types
                        .get(collection)
                        .unwrap_or(&CollectionType::Document),
                },
                StoreOp::PutDocument {
                    collection,
                    key: &key,
                    doc: &doc,
                },
            ])?;
            state
                .collection_types
                .entry(collection.to_string())
                .or_insert(CollectionType::Document);
            state
                .keys
                .entry(collection.to_string())
                .or_default()
                .insert(key.clone());
            self.cache_put(
                collection,
                &key,
                &self.strip_embedding_if_sidecar(doc.clone()),
            );
            self.sidecar_apply_write(collection, &key, Some(&doc));
            return Ok(DocumentId::new(collection, key));
        }
        if state
            .collections
            .get(collection)
            .is_some_and(|docs| docs.contains_key(&key))
        {
            return Err(CogniGraphError::DocumentConflict(format!(
                "{collection}/{key}"
            )));
        }
        let recorded_type = *state
            .collection_types
            .get(collection)
            .unwrap_or(&CollectionType::Document);
        self.persist(&[
            StoreOp::PutCollection {
                name: collection,
                collection_type: recorded_type,
            },
            StoreOp::PutDocument {
                collection,
                key: &key,
                doc: &doc,
            },
        ])?;
        collection_mut(&mut state, collection, CollectionType::Document)
            .insert(key.clone(), self.strip_embedding_if_sidecar(doc.clone()));
        self.sidecar_apply_write(collection, &key, Some(&doc));
        debug!(collection, key, "native document created");
        Ok(DocumentId::new(collection, key))
    }

    pub(super) fn get_document_impl(
        &self,
        collection_name: &str,
        key: &str,
    ) -> Result<Option<Value>> {
        if self.paged() {
            return self.fetch_paged(collection_name, key);
        }
        let state = self.read_state()?;
        Ok(state
            .collections
            .get(collection_name)
            .and_then(|collection| collection.get(key))
            .cloned())
    }

    pub(super) fn update_document_impl(
        &self,
        collection_name: &str,
        key: &str,
        update: Value,
    ) -> Result<Value> {
        let not_found = || CogniGraphError::DocumentNotFound {
            collection: collection_name.to_string(),
            key: key.to_string(),
        };
        if self.paged() {
            let state = self.write_state()?;
            let Some(keys) = state.keys.get(collection_name) else {
                return Err(CogniGraphError::CollectionNotFound(
                    collection_name.to_string(),
                ));
            };
            if !keys.contains(key) {
                return Err(not_found());
            }
            let mut updated = self
                .require_store()?
                .get_document_raw(collection_name, key)?
                .ok_or_else(not_found)?;
            merge_json(&mut updated, update);
            let updated = stamp_document(collection_name, key, updated);
            self.persist(&[StoreOp::PutDocument {
                collection: collection_name,
                key,
                doc: &updated,
            }])?;
            let stored = self.strip_embedding_if_sidecar(updated.clone());
            self.cache_put(collection_name, key, &stored);
            self.sidecar_apply_write(collection_name, key, Some(&updated));
            self.triples_invalidate(collection_name);
            drop(state);
            return Ok(stored);
        }
        let mut state = self.write_state()?;
        let collection = state
            .collections
            .get_mut(collection_name)
            .ok_or_else(|| CogniGraphError::CollectionNotFound(collection_name.to_string()))?;
        let mut updated = if self.vector_mode == VectorMode::Sidecar {
            // Merge against the redb truth so a partial update cannot
            // silently drop the (stripped) embedding.
            if !collection.contains_key(key) {
                return Err(not_found());
            }
            self.store
                .as_ref()
                .ok_or_else(|| {
                    CogniGraphError::BackendError("sidecar mode requires storage".into())
                })?
                .get_document_raw(collection_name, key)?
                .ok_or_else(not_found)?
        } else {
            collection.get(key).cloned().ok_or_else(not_found)?
        };
        merge_json(&mut updated, update);
        let updated = stamp_document(collection_name, key, updated);
        self.persist(&[StoreOp::PutDocument {
            collection: collection_name,
            key,
            doc: &updated,
        }])?;
        let stored = self.strip_embedding_if_sidecar(updated.clone());
        collection.insert(key.to_string(), stored.clone());
        self.sidecar_apply_write(collection_name, key, Some(&updated));
        self.triples_invalidate(collection_name);
        Ok(stored)
    }

    pub(super) fn replace_document_impl(
        &self,
        collection_name: &str,
        key: &str,
        doc: Value,
    ) -> Result<Value> {
        if self.paged() {
            let state = self.write_state()?;
            let Some(keys) = state.keys.get(collection_name) else {
                return Err(CogniGraphError::CollectionNotFound(
                    collection_name.to_string(),
                ));
            };
            if !keys.contains(key) {
                return Err(CogniGraphError::DocumentNotFound {
                    collection: collection_name.to_string(),
                    key: key.to_string(),
                });
            }
            let doc = stamp_document(collection_name, key, doc);
            self.persist(&[StoreOp::PutDocument {
                collection: collection_name,
                key,
                doc: &doc,
            }])?;
            let stored = self.strip_embedding_if_sidecar(doc.clone());
            self.cache_put(collection_name, key, &stored);
            self.sidecar_apply_write(collection_name, key, Some(&doc));
            self.triples_invalidate(collection_name);
            drop(state);
            return Ok(stored);
        }
        let mut state = self.write_state()?;
        let collection = state
            .collections
            .get_mut(collection_name)
            .ok_or_else(|| CogniGraphError::CollectionNotFound(collection_name.to_string()))?;
        if !collection.contains_key(key) {
            return Err(CogniGraphError::DocumentNotFound {
                collection: collection_name.to_string(),
                key: key.to_string(),
            });
        }
        let doc = stamp_document(collection_name, key, doc);
        self.persist(&[StoreOp::PutDocument {
            collection: collection_name,
            key,
            doc: &doc,
        }])?;
        let stored = self.strip_embedding_if_sidecar(doc.clone());
        collection.insert(key.to_string(), stored.clone());
        self.sidecar_apply_write(collection_name, key, Some(&doc));
        self.triples_invalidate(collection_name);
        Ok(stored)
    }

    pub(super) fn delete_document_impl(&self, collection_name: &str, key: &str) -> Result<bool> {
        if self.paged() {
            let mut state = self.write_state()?;
            let Some(keys) = state.keys.get_mut(collection_name) else {
                return Ok(false);
            };
            if !keys.contains(key) {
                return Ok(false);
            }
            self.persist(&[StoreOp::DeleteDocument {
                collection: collection_name,
                key,
            }])?;
            keys.remove(key);
            self.cache_remove(collection_name, key);
            self.sidecar_apply_write(collection_name, key, None);
            self.triples_invalidate(collection_name);
            drop(state);
            return Ok(true);
        }
        let mut state = self.write_state()?;
        let Some(collection) = state.collections.get_mut(collection_name) else {
            return Ok(false);
        };
        if !collection.contains_key(key) {
            return Ok(false);
        }
        self.persist(&[StoreOp::DeleteDocument {
            collection: collection_name,
            key,
        }])?;
        let removed = collection.remove(key).is_some();
        self.sidecar_apply_write(collection_name, key, None);
        self.triples_invalidate(collection_name);
        Ok(removed)
    }

    pub(super) fn list_documents_impl(
        &self,
        collection_name: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        if self.paged() {
            let state = self.read_state()?;
            if !state.keys.contains_key(collection_name) {
                return Err(CogniGraphError::CollectionNotFound(
                    collection_name.to_string(),
                ));
            }
            drop(state);
            return Ok(self
                .require_store()?
                .scan_collection_page(collection_name, limit, offset)?
                .into_iter()
                .map(|doc| self.strip_embedding_if_sidecar(doc))
                .collect());
        }
        let state = self.read_state()?;
        let collection = collection(&state, collection_name)?;
        Ok(collection
            .values()
            .skip(offset.unwrap_or(0))
            .take(limit.unwrap_or(usize::MAX))
            .cloned()
            .collect())
    }

    pub(super) fn list_documents_projected_impl(
        &self,
        collection_name: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        if self.paged() {
            let state = self.read_state()?;
            if !state.keys.contains_key(collection_name) {
                return Err(CogniGraphError::CollectionNotFound(
                    collection_name.to_string(),
                ));
            }
            drop(state);
            return Ok(self
                .require_store()?
                .scan_collection_page(collection_name, limit, offset)?
                .iter()
                .map(|doc| project_fields(doc, fields))
                .collect());
        }
        let state = self.read_state()?;
        let collection = collection(&state, collection_name)?;
        Ok(collection
            .values()
            .skip(offset.unwrap_or(0))
            .take(limit.unwrap_or(usize::MAX))
            .map(|doc| project_fields(doc, fields))
            .collect())
    }

    pub(super) fn list_documents_after_key_impl(
        &self,
        collection_name: &str,
        after_key: Option<&str>,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<Value>> {
        if self.paged() {
            let state = self.read_state()?;
            if !state.keys.contains_key(collection_name) {
                return Err(CogniGraphError::CollectionNotFound(
                    collection_name.to_string(),
                ));
            }
            drop(state);
            return Ok(self
                .require_store()?
                .scan_collection_after_key(collection_name, after_key, limit)?
                .iter()
                .map(|doc| project_fields(doc, fields))
                .collect());
        }

        let state = self.read_state()?;
        let collection = collection(&state, collection_name)?;
        let lower = after_key.map_or(Bound::Unbounded, Bound::Excluded);
        Ok(collection
            .range::<str, _>((lower, Bound::Unbounded))
            .take(limit)
            .map(|(_, doc)| project_fields(doc, fields))
            .collect())
    }

    /// Filtered scan: predicates run BY REFERENCE before any clone, so a
    /// selective filter never materializes the losers (or their embedding
    /// arrays). Paging applies after filtering, per the trait contract.
    /// Edge keys to consider when a predicate pins `_from`/`_to` to one vertex,
    /// or None when no such predicate applies (caller falls back to the scan).
    ///
    /// Deliberately narrow: only `Eq` against a string on an EDGE collection.
    /// Anything else — a range, a non-edge collection, a non-string value —
    /// keeps the previous behaviour, so this can only make queries faster, never
    /// change which rows they return.
    fn adjacency_candidates(
        &self,
        collection_name: &str,
        predicates: &[FieldPredicate],
    ) -> Result<Option<Vec<String>>> {
        let pinned = predicates.iter().find_map(|p| {
            if p.op != PredicateOp::Eq || p.path.len() != 1 {
                return None;
            }
            let vertex = p.value.as_str()?;
            match p.path[0].as_str() {
                "_from" => Some((true, vertex.to_string())),
                "_to" => Some((false, vertex.to_string())),
                _ => None,
            }
        });
        let Some((outbound, vertex)) = pinned else {
            return Ok(None);
        };
        {
            let state = self.read_state()?;
            let Some(kind) = state.collection_types.get(collection_name) else {
                return Ok(None);
            };
            if *kind != CollectionType::Edge {
                return Ok(None);
            }
        }
        let index = self.edge_adjacency(collection_name)?;
        let side = if outbound { &index.from } else { &index.to };
        Ok(Some(side.get(&vertex).cloned().unwrap_or_default()))
    }

    pub(super) fn list_documents_filtered_impl(
        &self,
        collection_name: &str,
        predicates: &[FieldPredicate],
        fields: Option<&[String]>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        let select = |doc: &Value| -> Value {
            match fields {
                Some(fields) => project_fields(doc, fields),
                None => doc.clone(),
            }
        };
        // An equality on `_from`/`_to` is an adjacency lookup, not a scan. The
        // index already existed for traversal; using it here is what makes a
        // correlated `FILTER e._to == <vertex>` affordable — on a 17.6k-edge
        // collection the scan cost ~49 ms per outer row against ~3 ms for the
        // indexed lookup (decision_cgql_v2_workload_gaps.md, D1).
        if let Some(candidates) = self.adjacency_candidates(collection_name, predicates)? {
            let mut rows = Vec::new();
            let mut skipped = 0usize;
            let offset = offset.unwrap_or(0);
            let limit = limit.unwrap_or(usize::MAX);
            for key in candidates {
                let Some(doc) = self.get_document_impl(collection_name, &key)? else {
                    continue;
                };
                // The index narrows by endpoint only; every predicate is still
                // applied, so results are identical to the scan path.
                if !predicates.iter().all(|p| p.matches(&doc)) {
                    continue;
                }
                if skipped < offset {
                    skipped += 1;
                    continue;
                }
                rows.push(select(&doc));
                if rows.len() >= limit {
                    break;
                }
            }
            return Ok(rows);
        }
        if self.paged() {
            let state = self.read_state()?;
            if !state.keys.contains_key(collection_name) {
                return Err(CogniGraphError::CollectionNotFound(
                    collection_name.to_string(),
                ));
            }
            drop(state);
            return Ok(self
                .require_store()?
                .scan_collection_filtered(collection_name, predicates, limit, offset)?
                .into_iter()
                .map(|doc| {
                    let doc = self.strip_embedding_if_sidecar(doc);
                    match fields {
                        Some(fields) => project_fields(&doc, fields),
                        None => doc,
                    }
                })
                .collect());
        }
        let state = self.read_state()?;
        let collection = collection(&state, collection_name)?;
        Ok(collection
            .values()
            .filter(|doc| predicates.iter().all(|p| p.matches(doc)))
            .skip(offset.unwrap_or(0))
            .take(limit.unwrap_or(usize::MAX))
            .map(select)
            .collect())
    }
}
