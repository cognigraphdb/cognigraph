use super::*;
use async_trait::async_trait;
use cognigraph_core::{
    CollectionInfo, Direction, IndexDef, QueryLanguage, SearchHit, TraversalOpts, TraversalPath,
    VectorSearchOpts,
};
use std::sync::atomic::{AtomicBool, AtomicUsize};

pub(super) struct FaultBackend {
    pub(super) inner: Arc<dyn GraphBackend>,
    pub(super) atomic: AtomicBool,
    pub(super) fail_next_batch: AtomicBool,
    pub(super) fail_read: AtomicBool,
    pub(super) malformed: AtomicBool,
    pub(super) fail_drop: AtomicBool,
    pub(super) fail_side_delete_at: AtomicUsize,
    pub(super) pause_batch: AtomicBool,
    pub(super) entered: tokio::sync::Notify,
    pub(super) release: tokio::sync::Notify,
}

impl FaultBackend {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(NativeBackend::new()),
            atomic: AtomicBool::new(true),
            fail_next_batch: AtomicBool::new(false),
            fail_read: AtomicBool::new(false),
            malformed: AtomicBool::new(false),
            fail_drop: AtomicBool::new(false),
            fail_side_delete_at: AtomicUsize::new(0),
            pause_batch: AtomicBool::new(false),
            entered: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
        }
    }
}
#[async_trait]
impl GraphBackend for FaultBackend {
    fn backend_name(&self) -> &str {
        self.inner.backend_name()
    }

    fn query_language(&self) -> QueryLanguage {
        self.inner.query_language()
    }

    fn supports_atomic_batches(&self) -> bool {
        self.atomic.load(Ordering::SeqCst)
    }

    async fn ping(&self) -> Result<()> {
        self.inner.ping().await
    }

    async fn create_document(&self, collection: &str, doc: Value) -> Result<DocumentId> {
        self.inner.create_document(collection, doc).await
    }

    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<Value>> {
        self.inner.get_document(collection, key).await
    }

    async fn update_document(&self, collection: &str, key: &str, update: Value) -> Result<Value> {
        self.inner.update_document(collection, key, update).await
    }

    async fn replace_document(&self, collection: &str, key: &str, doc: Value) -> Result<Value> {
        self.inner.replace_document(collection, key, doc).await
    }

    async fn delete_document(&self, collection: &str, key: &str) -> Result<bool> {
        if collection == SIDE_VIEWS_COLLECTION {
            let n = self.fail_side_delete_at.load(Ordering::SeqCst);
            if n > 0 && self.fail_side_delete_at.fetch_sub(1, Ordering::SeqCst) == 1 {
                return Err(CogniGraphError::BackendError(
                    "injected side-view delete failure".into(),
                ));
            }
        }
        self.inner.delete_document(collection, key).await
    }

    async fn list_documents(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        self.inner.list_documents(collection, limit, offset).await
    }

    async fn list_documents_projected(
        &self,
        collection: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
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
    ) -> Result<Vec<Value>> {
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
    ) -> Result<Vec<Value>> {
        if self.fail_read.swap(false, Ordering::SeqCst) {
            return Err(CogniGraphError::BackendError(
                "injected side-view read failure".into(),
            ));
        }
        if self.malformed.swap(false, Ordering::SeqCst) {
            return Ok(vec![json!({"document_id":"notes/a"})]);
        }
        self.inner
            .list_documents_filtered(collection, predicates, fields, limit, offset)
            .await
    }

    async fn list_collections(&self) -> Result<Vec<CollectionInfo>> {
        self.inner.list_collections().await
    }

    async fn export_snapshot(&self) -> Result<Value> {
        self.inner.export_snapshot().await
    }

    async fn import_snapshot(&self, data: &Value) -> Result<()> {
        self.inner.import_snapshot(data).await
    }

    async fn create_edge(&self, collection: &str, edge: Value) -> Result<DocumentId> {
        self.inner.create_edge(collection, edge).await
    }

    async fn upsert_edge(
        &self,
        collection: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: Value,
    ) -> Result<Value> {
        self.inner
            .upsert_edge(collection, from, to, relation_type, data)
            .await
    }

    async fn get_edges(
        &self,
        collection: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> Result<Vec<Value>> {
        self.inner.get_edges(collection, vertex_id, direction).await
    }

    async fn execute_batch(&self, ops: Vec<BatchOp>) -> Result<Vec<Value>> {
        if self.fail_next_batch.swap(false, Ordering::SeqCst) {
            return Err(CogniGraphError::BackendError(
                "injected side-view batch failure".into(),
            ));
        }
        if self.pause_batch.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.inner.execute_batch(ops).await
    }

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        self.inner.traverse(start_vertex, opts).await
    }

    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
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
    ) -> Result<Vec<SearchHit>> {
        self.inner
            .text_search(collection, query, fields, limit)
            .await
    }

    async fn query(&self, query: &str, bind_vars: HashMap<String, Value>) -> Result<Vec<Value>> {
        self.inner.query(query, bind_vars).await
    }

    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()> {
        self.inner.ensure_collection(name, collection_type).await
    }

    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()> {
        self.inner.ensure_index(collection, index).await
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        if self.fail_drop.swap(false, Ordering::SeqCst) {
            return Err(CogniGraphError::BackendError(
                "injected collection drop failure".into(),
            ));
        }
        self.inner.drop_collection(name).await
    }
}
