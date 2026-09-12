//! A capability-only test double: storage access is always an error and counted.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::Value;

use crate::{
    BatchOp, CogniGraphError, CollectionInfo, CollectionType, Direction, DocumentId, GraphBackend,
    IndexDef, QueryLanguage, Result, SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};

/// Use in tests that must reject work before touching storage. Unlike an
/// unreachable network adapter, this detects access even when its error is caught.
#[derive(Default)]
pub struct NoAccessBackend {
    calls: AtomicUsize,
}

impl NoAccessBackend {
    /// Assert after the operation under test, including after any spawned work.
    pub fn assert_unused(&self) {
        assert_eq!(
            self.calls.load(Ordering::SeqCst),
            0,
            "unexpected storage access"
        );
    }

    fn unexpected<T>(&self, operation: &str) -> Result<T> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(CogniGraphError::BackendError(format!(
            "unexpected {operation} on no-access test backend"
        )))
    }
}

#[async_trait]
impl GraphBackend for NoAccessBackend {
    fn backend_name(&self) -> &str {
        "no-access-test"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::BackendNative("opaque-test-language".into())
    }

    // Atomic batches remain unsupported through the trait's default capability.
    async fn ping(&self) -> Result<()> {
        self.unexpected("ping")
    }

    async fn create_document(&self, _: &str, _: Value) -> Result<DocumentId> {
        self.unexpected("create_document")
    }

    async fn get_document(&self, _: &str, _: &str) -> Result<Option<Value>> {
        self.unexpected("get_document")
    }

    async fn update_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        self.unexpected("update_document")
    }

    async fn replace_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        self.unexpected("replace_document")
    }

    async fn delete_document(&self, _: &str, _: &str) -> Result<bool> {
        self.unexpected("delete_document")
    }

    async fn list_documents(
        &self,
        _: &str,
        _: Option<usize>,
        _: Option<usize>,
    ) -> Result<Vec<Value>> {
        self.unexpected("list_documents")
    }

    // Default projected/filtered/cursor scans call list_documents and are counted.
    async fn list_collections(&self) -> Result<Vec<CollectionInfo>> {
        self.unexpected("list_collections")
    }

    async fn export_snapshot(&self) -> Result<Value> {
        self.unexpected("export_snapshot")
    }

    async fn import_snapshot(&self, _: &Value) -> Result<()> {
        self.unexpected("import_snapshot")
    }

    async fn create_edge(&self, _: &str, _: Value) -> Result<DocumentId> {
        self.unexpected("create_edge")
    }

    async fn upsert_edge(&self, _: &str, _: &str, _: &str, _: &str, _: Value) -> Result<Value> {
        self.unexpected("upsert_edge")
    }

    async fn get_edges(&self, _: &str, _: &str, _: Direction) -> Result<Vec<Value>> {
        self.unexpected("get_edges")
    }

    async fn execute_batch(&self, _: Vec<BatchOp>) -> Result<Vec<Value>> {
        self.unexpected("execute_batch")
    }

    async fn traverse(&self, _: &str, _: &TraversalOpts) -> Result<Vec<TraversalPath>> {
        self.unexpected("traverse")
    }

    async fn vector_search(
        &self,
        _: &str,
        _: &[f64],
        _: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        self.unexpected("vector_search")
    }

    async fn text_search(
        &self,
        _: &str,
        _: &str,
        _: &[String],
        _: usize,
    ) -> Result<Vec<SearchHit>> {
        self.unexpected("text_search")
    }

    async fn query(&self, _: &str, _: HashMap<String, Value>) -> Result<Vec<Value>> {
        self.unexpected("query")
    }

    async fn ensure_collection(&self, _: &str, _: CollectionType) -> Result<()> {
        self.unexpected("ensure_collection")
    }

    async fn ensure_index(&self, _: &str, _: &IndexDef) -> Result<()> {
        self.unexpected("ensure_index")
    }

    async fn drop_collection(&self, _: &str) -> Result<()> {
        self.unexpected("drop_collection")
    }
}
