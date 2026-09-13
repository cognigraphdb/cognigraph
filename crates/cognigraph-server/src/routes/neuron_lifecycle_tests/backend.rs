use super::*;
use cognigraph_core::{
    CollectionType, Direction, DocumentId, FieldPredicate, IndexDef, QueryLanguage, Result,
    SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct ControlledBackend {
    pub inner: NativeBackend,
    pub pause_scan: AtomicBool,
    pub fail_scan: AtomicBool,
    pub entered: Notify,
    pub release: Notify,
}
#[async_trait::async_trait]
impl GraphBackend for ControlledBackend {
    fn backend_name(&self) -> &str {
        "cg21-controlled"
    }
    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Cgql
    }
    async fn get_document(&self, c: &str, key: &str) -> Result<Option<Value>> {
        self.inner.get_document(c, key).await
    }
    async fn list_documents_filtered(
        &self,
        c: &str,
        p: &[FieldPredicate],
        fields: Option<&[String]>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        let accepted = c == "neurons"
            && p.iter()
                .any(|p| p.path == ["status"] && p.value == "accepted");
        if accepted && self.fail_scan.swap(false, Ordering::SeqCst) {
            return Err(CogniGraphError::ConnectionError(
                "synthetic accepted-set outage".into(),
            ));
        }
        let result = self
            .inner
            .list_documents_filtered(c, p, fields, limit, offset)
            .await;
        if accepted && self.pause_scan.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        result
    }
    async fn create_document(&self, c: &str, doc: Value) -> Result<DocumentId> {
        self.inner.create_document(c, doc).await
    }
    async fn update_document(&self, c: &str, key: &str, merge: Value) -> Result<Value> {
        self.inner.update_document(c, key, merge).await
    }
    async fn replace_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn delete_document(&self, _: &str, _: &str) -> Result<bool> {
        unimplemented!()
    }
    async fn list_documents(
        &self,
        _: &str,
        _: Option<usize>,
        _: Option<usize>,
    ) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn create_edge(&self, _: &str, _: Value) -> Result<DocumentId> {
        unimplemented!()
    }
    async fn upsert_edge(&self, _: &str, _: &str, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn get_edges(&self, _: &str, _: &str, _: Direction) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn traverse(&self, _: &str, _: &TraversalOpts) -> Result<Vec<TraversalPath>> {
        unimplemented!()
    }
    async fn vector_search(
        &self,
        _: &str,
        _: &[f64],
        _: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        unimplemented!()
    }
    async fn query(&self, _: &str, _: HashMap<String, Value>) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn ensure_collection(&self, _: &str, _: CollectionType) -> Result<()> {
        unimplemented!()
    }
    async fn ensure_index(&self, _: &str, _: &IndexDef) -> Result<()> {
        unimplemented!()
    }
    async fn drop_collection(&self, _: &str) -> Result<()> {
        unimplemented!()
    }
}
