//! Backend.

use super::*;

/// Delegates every operation to a real backend, except for one explicitly
/// armed `execute_batch` call. This keeps the lifecycle fixture on the
/// production atomic path while proving a backend error cannot expose a
/// partial M26 graph, decision, or head.
pub(super) struct FailNextBatchBackend {
    pub(super) inner: Arc<dyn GraphBackend>,
    pub(super) fail_next_batch: Arc<AtomicBool>,
}
#[async_trait]
impl GraphBackend for FailNextBatchBackend {
    fn backend_name(&self) -> &str {
        self.inner.backend_name()
    }

    fn query_language(&self) -> cognigraph_core::QueryLanguage {
        self.inner.query_language()
    }

    fn supports_atomic_batches(&self) -> bool {
        self.inner.supports_atomic_batches()
    }

    async fn ping(&self) -> BackendResult<()> {
        self.inner.ping().await
    }

    async fn create_document(&self, collection: &str, doc: Value) -> BackendResult<DocumentId> {
        self.inner.create_document(collection, doc).await
    }

    async fn get_document(&self, collection: &str, key: &str) -> BackendResult<Option<Value>> {
        self.inner.get_document(collection, key).await
    }

    async fn update_document(
        &self,
        collection: &str,
        key: &str,
        update: Value,
    ) -> BackendResult<Value> {
        self.inner.update_document(collection, key, update).await
    }

    async fn replace_document(
        &self,
        collection: &str,
        key: &str,
        doc: Value,
    ) -> BackendResult<Value> {
        self.inner.replace_document(collection, key, doc).await
    }

    async fn delete_document(&self, collection: &str, key: &str) -> BackendResult<bool> {
        self.inner.delete_document(collection, key).await
    }

    async fn list_documents(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> BackendResult<Vec<Value>> {
        self.inner.list_documents(collection, limit, offset).await
    }

    async fn list_documents_projected(
        &self,
        collection: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> BackendResult<Vec<Value>> {
        self.inner
            .list_documents_projected(collection, fields, limit, offset)
            .await
    }

    async fn list_documents_after_key(
        &self,
        collection: &str,
        after_key: Option<&str>,
        fields: &[String],
        limit: usize,
    ) -> BackendResult<Vec<Value>> {
        self.inner
            .list_documents_after_key(collection, after_key, fields, limit)
            .await
    }

    async fn list_documents_filtered(
        &self,
        collection: &str,
        predicates: &[FieldPredicate],
        fields: Option<&[String]>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> BackendResult<Vec<Value>> {
        self.inner
            .list_documents_filtered(collection, predicates, fields, limit, offset)
            .await
    }

    async fn list_collections(&self) -> BackendResult<Vec<CollectionInfo>> {
        self.inner.list_collections().await
    }

    async fn export_snapshot(&self) -> BackendResult<Value> {
        self.inner.export_snapshot().await
    }

    async fn import_snapshot(&self, data: &Value) -> BackendResult<()> {
        self.inner.import_snapshot(data).await
    }

    async fn create_edge(&self, collection: &str, edge: Value) -> BackendResult<DocumentId> {
        self.inner.create_edge(collection, edge).await
    }

    async fn upsert_edge(
        &self,
        collection: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: Value,
    ) -> BackendResult<Value> {
        self.inner
            .upsert_edge(collection, from, to, relation_type, data)
            .await
    }

    async fn get_edges(
        &self,
        collection: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> BackendResult<Vec<Value>> {
        self.inner.get_edges(collection, vertex_id, direction).await
    }

    async fn execute_batch(&self, ops: Vec<BatchOp>) -> BackendResult<Vec<Value>> {
        if self.fail_next_batch.swap(false, Ordering::SeqCst) {
            return Err(CogniGraphError::BackendError(
                "injected M26 atomic batch failure".into(),
            ));
        }
        self.inner.execute_batch(ops).await
    }

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> BackendResult<Vec<TraversalPath>> {
        self.inner.traverse(start_vertex, opts).await
    }

    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> BackendResult<Vec<SearchHit>> {
        self.inner
            .vector_search(collection, query_vector, opts)
            .await
    }

    async fn text_search(
        &self,
        collection: &str,
        query: &str,
        fields: &[String],
        limit: usize,
    ) -> BackendResult<Vec<SearchHit>> {
        self.inner
            .text_search(collection, query, fields, limit)
            .await
    }

    async fn query(
        &self,
        query: &str,
        bind_vars: HashMap<String, Value>,
    ) -> BackendResult<Vec<Value>> {
        self.inner.query(query, bind_vars).await
    }

    async fn ensure_collection(
        &self,
        name: &str,
        collection_type: CollectionType,
    ) -> BackendResult<()> {
        self.inner.ensure_collection(name, collection_type).await
    }

    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> BackendResult<()> {
        self.inner.ensure_index(collection, index).await
    }

    async fn drop_collection(&self, name: &str) -> BackendResult<()> {
        self.inner.drop_collection(name).await
    }
}
