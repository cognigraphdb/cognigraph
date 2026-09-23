//! The `GraphBackend` impl for `NativeBackend`: a thin delegation layer.
//! Every method body except `query` is synchronous and lives as an inherent
//! `*_impl` method in `documents`, `edges`, `batch`, or `search` — the trait
//! block stays small enough to read as the backend's API surface.

use std::collections::HashMap;

use async_trait::async_trait;
use cognigraph_core::{
    BatchOp, CogniGraphError, CollectionType, Direction, DocumentId, FieldPredicate, GraphBackend,
    IndexDef, QueryLanguage, Result, SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};
use serde_json::Value;

use crate::storage::StoreOp;

use super::NativeBackend;
use super::helpers::collection_mut;

#[async_trait]
impl GraphBackend for NativeBackend {
    fn backend_name(&self) -> &str {
        "native"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Cgql
    }

    fn supports_atomic_batches(&self) -> bool {
        true
    }

    async fn ping(&self) -> Result<()> {
        // A poisoned state lock means the live backend cannot serve reads.
        // Persistent mode additionally probes a real redb read transaction;
        // merely having opened the file successfully at startup is not a
        // sufficient readiness signal.
        let state = self.read_state()?;
        drop(state);
        if let Some(store) = &self.store {
            store.ping()?;
        }
        Ok(())
    }

    async fn create_document(&self, collection: &str, doc: Value) -> Result<DocumentId> {
        self.create_document_impl(collection, doc)
    }

    async fn get_document(&self, collection_name: &str, key: &str) -> Result<Option<Value>> {
        self.get_document_impl(collection_name, key)
    }

    async fn update_document(
        &self,
        collection_name: &str,
        key: &str,
        update: Value,
    ) -> Result<Value> {
        self.update_document_impl(collection_name, key, update)
    }

    async fn replace_document(
        &self,
        collection_name: &str,
        key: &str,
        doc: Value,
    ) -> Result<Value> {
        self.replace_document_impl(collection_name, key, doc)
    }

    async fn delete_document(&self, collection_name: &str, key: &str) -> Result<bool> {
        self.delete_document_impl(collection_name, key)
    }

    async fn list_documents(
        &self,
        collection_name: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        self.list_documents_impl(collection_name, limit, offset)
    }

    /// Clones only the requested top-level fields (plus `_key`/`_id`) —
    /// scans that don't touch `embedding` skip cloning the vectors.
    async fn list_documents_projected(
        &self,
        collection_name: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        self.list_documents_projected_impl(collection_name, fields, limit, offset)
    }

    async fn list_documents_after_key(
        &self,
        collection_name: &str,
        after_key: Option<&str>,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<Value>> {
        self.list_documents_after_key_impl(collection_name, after_key, fields, limit)
    }

    async fn list_documents_filtered(
        &self,
        collection_name: &str,
        predicates: &[FieldPredicate],
        fields: Option<&[String]>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        self.list_documents_filtered_impl(collection_name, predicates, fields, limit, offset)
    }

    async fn create_edge(&self, collection_name: &str, edge: Value) -> Result<DocumentId> {
        self.create_edge_impl(collection_name, edge)
    }

    async fn upsert_edge(
        &self,
        collection_name: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: Value,
    ) -> Result<Value> {
        self.upsert_edge_impl(collection_name, from, to, relation_type, data)
    }

    async fn get_edges(
        &self,
        collection_name: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> Result<Vec<Value>> {
        self.get_edges_impl(collection_name, vertex_id, direction)
    }

    /// Atomic batch: everything validated and prepared under the write
    /// lock, persisted in ONE redb transaction, and only then applied to
    /// memory/cache/sidecar — all-or-nothing by construction.
    async fn execute_batch(&self, ops: Vec<BatchOp>) -> Result<Vec<Value>> {
        self.execute_batch_impl(ops)
    }

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        self.traverse_impl(start_vertex, opts)
    }

    async fn vector_search(
        &self,
        collection_name: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        self.vector_search_impl(collection_name, query_vector, opts)
    }

    /// BM25 via a lazily built in-RAM tantivy index (rebuilt when the
    /// collection changes — same invalidation as the vector indexes).
    async fn text_search(
        &self,
        collection_name: &str,
        query: &str,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        self.text_search_impl(collection_name, query, fields, limit)
    }

    async fn query(&self, query: &str, bind_vars: HashMap<String, Value>) -> Result<Vec<Value>> {
        cognigraph_query::parse_and_execute_backend(query, self, &bind_vars)
            .await
            .map_err(|e| CogniGraphError::QueryError(e.to_string()))
    }

    async fn export_snapshot(&self) -> Result<Value> {
        self.export_json().await
    }

    async fn import_snapshot(&self, data: &Value) -> Result<()> {
        self.import_json(data).await
    }

    async fn list_collections(&self) -> Result<Vec<cognigraph_core::CollectionInfo>> {
        let state = self.read_state()?;
        let mut infos: Vec<cognigraph_core::CollectionInfo> = if self.paged() {
            state
                .keys
                .iter()
                .map(|(name, keys)| cognigraph_core::CollectionInfo {
                    name: name.clone(),
                    collection_type: state
                        .collection_types
                        .get(name)
                        .unwrap_or(&CollectionType::Document)
                        .as_str()
                        .to_string(),
                    count: keys.len() as u64,
                })
                .collect()
        } else {
            state
                .collections
                .iter()
                .map(|(name, docs)| cognigraph_core::CollectionInfo {
                    name: name.clone(),
                    collection_type: state
                        .collection_types
                        .get(name)
                        .unwrap_or(&CollectionType::Document)
                        .as_str()
                        .to_string(),
                    count: docs.len() as u64,
                })
                .collect()
        };
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(infos)
    }

    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()> {
        let mut state = self.write_state()?;
        // The catalog and creation share the write lock: concurrent ensures
        // commit only once. Existing collections retain their original type,
        // and a no-op must not advance versions or invalidate derivatives.
        if state.collection_types.contains_key(name) {
            return Ok(());
        }
        self.persist(&[StoreOp::PutCollection {
            name,
            collection_type,
        }])?;
        if self.paged() {
            state
                .collection_types
                .entry(name.to_string())
                .or_insert(collection_type);
            state.keys.entry(name.to_string()).or_default();
        } else {
            collection_mut(&mut state, name, collection_type);
        }
        Ok(())
    }

    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()> {
        self.ensure_index_impl(collection, index)
    }

    async fn list_indexes(&self, collection: &str) -> Result<Vec<IndexDef>> {
        self.list_indexes_impl(collection)
    }

    async fn drop_index(&self, collection: &str, name: &str) -> Result<bool> {
        self.drop_index_impl(collection, name)
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        let mut state = self.write_state()?;
        self.persist(&[StoreOp::DropCollection { name }])?;
        self.triples_invalidate(name);
        state.collections.remove(name);
        state.keys.remove(name);
        state.collection_types.remove(name);
        self.indexes_forget_collection(&mut state, name);
        self.cache_remove_collection(name);
        if let Ok(mut sidecars) = self.sidecars.write() {
            sidecars.remove(name);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ping_reports_a_poisoned_native_state() {
        let backend = NativeBackend::new();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = backend.state.write().expect("state lock");
            panic!("poison native state for readiness regression");
        }));

        let error = backend.ping().await.unwrap_err();
        assert!(error.to_string().contains("lock poisoned"), "{error}");
    }
}
