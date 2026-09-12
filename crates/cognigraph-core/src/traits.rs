use async_trait::async_trait;
use std::collections::HashMap;

use crate::error::Result;
use crate::types::*;

/// The central storage abstraction for CogniGraph.
///
/// Any backend implements this trait
/// to provide graph storage and query capabilities. Accepted collection names,
/// keys, references, and caller-owned JSON strings retain their exact Unicode
/// contents. A backend may enforce its identifier grammar, but must not silently
/// normalize a reference to a different key. Text/evidence canonicalization is
/// an explicit caller operation, separate from backend metadata stamping.
#[async_trait]
pub trait GraphBackend: Send + Sync {
    /// Returns the backend name (e.g. "native").
    fn backend_name(&self) -> &str;

    /// Returns the query language accepted by `query()`.
    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::BackendNative(self.backend_name().to_string())
    }

    /// Whether `execute_batch` provides all-or-nothing transactional writes.
    ///
    /// Callers that require atomic replacement semantics must check this
    /// before performing any preparatory writes. Backends opt in explicitly;
    /// overriding `execute_batch` alone is not enough to claim the contract.
    fn supports_atomic_batches(&self) -> bool {
        false
    }

    /// Cheap connectivity/liveness probe. Remote and durable embedded
    /// backends should override this; a purely in-memory backend may keep the
    /// default.
    async fn ping(&self) -> Result<()> {
        Ok(())
    }

    // --- Document operations ---

    /// Insert a document into a collection, returning its assigned ID.
    /// A missing collection is materialized as a document collection before
    /// the insert is retried.
    async fn create_document(&self, collection: &str, doc: serde_json::Value)
    -> Result<DocumentId>;

    /// Retrieve a document by key. Returns None if not found.
    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<serde_json::Value>>;

    /// Update a document (partial merge). Returns the updated document.
    async fn update_document(
        &self,
        collection: &str,
        key: &str,
        update: serde_json::Value,
    ) -> Result<serde_json::Value>;

    /// Replace a document entirely. Returns the new document.
    async fn replace_document(
        &self,
        collection: &str,
        key: &str,
        doc: serde_json::Value,
    ) -> Result<serde_json::Value>;

    /// Delete a document by key. Returns true if it existed.
    async fn delete_document(&self, collection: &str, key: &str) -> Result<bool>;

    /// List documents in a collection with optional limit and offset.
    /// `None` means unbounded; backends must not substitute an implicit cap.
    async fn list_documents(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>>;

    /// List documents projected to the given top-level fields (plus `_key`
    /// and `_id`, always included). This is an optimization hint: backends
    /// may return more fields than requested, never fewer. The default
    /// returns full documents.
    async fn list_documents_projected(
        &self,
        collection: &str,
        _fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>> {
        self.list_documents(collection, limit, offset).await
    }

    /// List a projected page in ascending document-key order, strictly after
    /// `after_key`. Unlike offset pagination, a caller can resume this scan
    /// without making the backend walk every preceding row again. `_key` and
    /// `_id` are always included under the same projection contract as
    /// [`GraphBackend::list_documents_projected`].
    ///
    /// Backends with an ordered primary-key index should override this. The
    /// default is deliberately correctness-first: it projects the complete
    /// collection, validates and sorts its keys, then applies the cursor.
    async fn list_documents_after_key(
        &self,
        collection: &str,
        after_key: Option<&str>,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        let rows = self
            .list_documents_projected(collection, fields, None, None)
            .await?;
        let mut keyed = rows
            .into_iter()
            .map(|document| {
                let key = document
                    .get("_key")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        crate::error::CogniGraphError::BackendError(format!(
                            "backend `{}` returned a document without a string `_key`",
                            self.backend_name()
                        ))
                    })?
                    .to_string();
                Ok((key, document))
            })
            .collect::<Result<Vec<_>>>()?;
        keyed.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(keyed
            .into_iter()
            .filter(|(key, _)| after_key.is_none_or(|after| key.as_str() > after))
            .take(limit)
            .map(|(_, document)| document)
            .collect())
    }

    /// Filtered scan capability (filter pushdown): return the documents
    /// matching ALL `predicates`, in the backend's stable scan order, with
    /// `offset`/`limit` applied AFTER filtering. Predicate semantics are
    /// exactly `FieldPredicate::matches` — the caller (the CGQL executor)
    /// relies on that to push LIMIT through fully-pushed filters. Like
    /// projection, `fields` is an optimization contract: more fields than
    /// requested is fine, fewer is not; `None` means full documents. The
    /// default delegates to a full scan and filters here, so every backend
    /// is correct; backends override to avoid cloning non-matching rows.
    async fn list_documents_filtered(
        &self,
        collection: &str,
        predicates: &[FieldPredicate],
        fields: Option<&[String]>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<serde_json::Value>> {
        let rows = self.list_documents(collection, None, None).await?;
        Ok(rows
            .into_iter()
            .filter(|doc| predicates.iter().all(|p| p.matches(doc)))
            .skip(offset.unwrap_or(0))
            .take(limit.unwrap_or(usize::MAX))
            .map(|doc| match fields {
                Some(fields) => project_fields(&doc, fields),
                None => doc,
            })
            .collect())
    }

    /// Collection catalog: names, types, and counts. Default: unsupported —
    /// backends that can enumerate cheaply opt in (the console's collection
    /// browser is built on this).
    async fn list_collections(&self) -> Result<Vec<crate::types::CollectionInfo>> {
        Err(crate::error::CogniGraphError::BackendError(format!(
            "backend `{}` does not support collection listing",
            self.backend_name()
        )))
    }

    /// Full-database JSON snapshot (hot backup). The format is the
    /// `import_snapshot` payload: `{"collections": {name: {type, documents:
    /// {key: doc}}}}`. Default: unsupported — backends with their own
    /// durability story opt in.
    async fn export_snapshot(&self) -> Result<serde_json::Value> {
        Err(crate::error::CogniGraphError::BackendError(format!(
            "backend `{}` does not support snapshots",
            self.backend_name()
        )))
    }

    /// Restore a snapshot produced by `export_snapshot`: creates collections
    /// and overwrites documents with matching keys (additive, not a wipe).
    async fn import_snapshot(&self, _data: &serde_json::Value) -> Result<()> {
        Err(crate::error::CogniGraphError::BackendError(format!(
            "backend `{}` does not support snapshots",
            self.backend_name()
        )))
    }

    // --- Edge operations ---

    /// Create an edge in an edge collection. A missing collection is
    /// materialized as an edge collection before the insert is retried.
    async fn create_edge(&self, collection: &str, edge: serde_json::Value) -> Result<DocumentId>;

    /// Upsert an edge (insert or update based on _from, _to, relation_type).
    /// A missing collection is materialized as an edge collection first.
    async fn upsert_edge(
        &self,
        collection: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: serde_json::Value,
    ) -> Result<serde_json::Value>;

    /// Get edges connected to a document.
    async fn get_edges(
        &self,
        collection: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> Result<Vec<serde_json::Value>>;

    /// Execute a batch of writes atomically: either every operation
    /// applies or none does. Backends without transactional batches keep
    /// the default. Returns the resulting document per op (null for
    /// deletes), in input order.
    async fn execute_batch(&self, _ops: Vec<BatchOp>) -> Result<Vec<serde_json::Value>> {
        Err(crate::error::CogniGraphError::BackendError(format!(
            "backend `{}` does not support batch transactions",
            self.backend_name()
        )))
    }

    // --- Graph traversal ---

    /// Traverse the graph from a starting vertex using the per-edge confidence
    /// and depth semantics documented by [`TraversalOpts`].
    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>>;

    // --- Vector search ---

    /// Perform approximate nearest neighbor search on embeddings.
    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>>;

    // --- Full-text search ---

    /// BM25 full-text search over string fields of a collection's documents.
    /// Backends without full-text support keep the default and callers must
    /// treat the error as a capability gap, not a failure.
    async fn text_search(
        &self,
        _collection: &str,
        _query: &str,
        _fields: &[String],
        _limit: usize,
    ) -> Result<Vec<SearchHit>> {
        Err(crate::error::CogniGraphError::BackendError(format!(
            "backend `{}` does not support full-text search",
            self.backend_name()
        )))
    }

    // --- Raw query ---

    /// Execute a query in the declared language with bind variables.
    async fn query(
        &self,
        query: &str,
        bind_vars: HashMap<String, serde_json::Value>,
    ) -> Result<Vec<serde_json::Value>>;

    // --- Schema management ---

    /// Ensure a collection exists, creating it if necessary.
    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()>;

    /// Ensure an index exists on a collection.
    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()>;

    /// Drop a collection if it exists.
    async fn drop_collection(&self, name: &str) -> Result<()>;
}
