//! Edge write/read bodies for the `GraphBackend` impl — synchronous inherent
//! methods; `backend.rs` delegates to them.

use cognigraph_core::{CogniGraphError, CollectionType, Direction, DocumentId, Result};
use serde_json::{Value, json};
use tracing::debug;
use uuid::Uuid;

use crate::storage::StoreOp;

use super::NativeBackend;
use super::helpers::{
    check_collection_type, collection, collection_mut, edge_key, edge_matches, stamp_edge,
};

impl NativeBackend {
    pub(super) fn create_edge_impl(
        &self,
        collection_name: &str,
        edge: Value,
    ) -> Result<DocumentId> {
        let from = edge
            .get("_from")
            .and_then(Value::as_str)
            .ok_or_else(|| CogniGraphError::ValidationError("Edge missing _from".into()))?
            .to_string();
        let to = edge
            .get("_to")
            .and_then(Value::as_str)
            .ok_or_else(|| CogniGraphError::ValidationError("Edge missing _to".into()))?
            .to_string();
        let key = edge_key(&edge);
        let edge = stamp_edge(collection_name, &key, edge);
        let mut state = self.write_state()?;
        check_collection_type(&state, collection_name, CollectionType::Edge)?;
        if self.paged() {
            if state
                .keys
                .get(collection_name)
                .is_some_and(|keys| keys.contains(&key))
            {
                return Err(CogniGraphError::DocumentConflict(format!(
                    "{collection_name}/{key}"
                )));
            }
            self.persist(&[
                StoreOp::PutCollection {
                    name: collection_name,
                    collection_type: CollectionType::Edge,
                },
                StoreOp::PutDocument {
                    collection: collection_name,
                    key: &key,
                    doc: &edge,
                },
            ])?;
            state
                .collection_types
                .entry(collection_name.to_string())
                .or_insert(CollectionType::Edge);
            state
                .keys
                .entry(collection_name.to_string())
                .or_default()
                .insert(key.clone());
            self.cache_put(collection_name, &key, &edge);
            self.triples_invalidate(collection_name);
            return Ok(DocumentId::new(collection_name, key));
        }
        if state
            .collections
            .get(collection_name)
            .is_some_and(|edges| edges.contains_key(&key))
        {
            return Err(CogniGraphError::DocumentConflict(format!(
                "{collection_name}/{key}"
            )));
        }
        self.persist(&[
            StoreOp::PutCollection {
                name: collection_name,
                collection_type: CollectionType::Edge,
            },
            StoreOp::PutDocument {
                collection: collection_name,
                key: &key,
                doc: &edge,
            },
        ])?;
        collection_mut(&mut state, collection_name, CollectionType::Edge).insert(key.clone(), edge);
        self.triples_invalidate(collection_name);
        debug!(
            collection = collection_name,
            key, from, to, "native edge created"
        );
        Ok(DocumentId::new(collection_name, key))
    }

    pub(super) fn upsert_edge_impl(
        &self,
        collection_name: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        mut data: Value,
    ) -> Result<Value> {
        let mut state = self.write_state()?;
        check_collection_type(&state, collection_name, CollectionType::Edge)?;
        let existing_key = self.triple_key(&state, collection_name, from, to, relation_type)?;
        if self.paged() {
            if let Value::Object(obj) = &mut data {
                obj.insert("_from".into(), json!(from));
                obj.insert("_to".into(), json!(to));
                obj.insert("relation_type".into(), json!(relation_type));
            }
            let key = existing_key.unwrap_or_else(|| Uuid::new_v4().to_string());
            let edge = stamp_edge(collection_name, &key, data);
            self.persist(&[
                StoreOp::PutCollection {
                    name: collection_name,
                    collection_type: CollectionType::Edge,
                },
                StoreOp::PutDocument {
                    collection: collection_name,
                    key: &key,
                    doc: &edge,
                },
            ])?;
            state
                .collection_types
                .entry(collection_name.to_string())
                .or_insert(CollectionType::Edge);
            state
                .keys
                .entry(collection_name.to_string())
                .or_default()
                .insert(key.clone());
            self.cache_put(collection_name, &key, &edge);
            self.triple_record(collection_name, from, to, relation_type, &key);
            return Ok(edge);
        }
        let collection = collection_mut(&mut state, collection_name, CollectionType::Edge);
        if let Value::Object(obj) = &mut data {
            obj.insert("_from".into(), json!(from));
            obj.insert("_to".into(), json!(to));
            obj.insert("relation_type".into(), json!(relation_type));
        }
        let key = existing_key.unwrap_or_else(|| Uuid::new_v4().to_string());
        let edge = stamp_edge(collection_name, &key, data);
        self.persist(&[
            StoreOp::PutCollection {
                name: collection_name,
                collection_type: CollectionType::Edge,
            },
            StoreOp::PutDocument {
                collection: collection_name,
                key: &key,
                doc: &edge,
            },
        ])?;
        collection.insert(key.clone(), edge.clone());
        self.triple_record(collection_name, from, to, relation_type, &key);
        Ok(edge)
    }

    pub(super) fn get_edges_impl(
        &self,
        collection_name: &str,
        vertex_id: &str,
        direction: Direction,
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
                .scan_collection(collection_name)?
                .into_iter()
                .map(|(_, edge)| edge)
                .filter(|edge| edge_matches(edge, vertex_id, direction))
                .collect());
        }
        let state = self.read_state()?;
        let collection = collection(&state, collection_name)?;
        Ok(collection
            .values()
            .filter(|edge| edge_matches(edge, vertex_id, direction))
            .cloned()
            .collect())
    }
}
