//! Multi-tenancy M2 (decision_multi_tenancy.md, D1/D2): one backend
//! store per tenant, routed at a SINGLE structural choke point.
//!
//! Instead of threading a per-request backend through every handler —
//! where one missed call site is exactly the cross-tenant leak the
//! design forbids — `AppState.backend` becomes [`RoutedBackend`]: a
//! `GraphBackend` facade that resolves a request's captured store from a task
//! local (set by the auth middleware around the request future). Admission
//! pins identity and handles under the tenant lifecycle lock, so later calls
//! cannot switch to a recreated tenant's entry in [`TenantRegistry`]. The
//! query cache gets the same treatment ([`RoutedCache`]) because a
//! shared cache would leak results and embeddings across tenants (D3).
//!
//! With no task-local in scope (auth disabled, or single-tenant mode
//! never constructs these types) everything resolves to the implicit
//! default tenant — back-compat is absolute.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use cognigraph_auth::DEFAULT_TENANT;
use cognigraph_cache::{
    CacheConfig, CacheHit, CacheKey, CacheStats, CacheStatsSnapshot, InMemoryCache, QueryCache,
};
use cognigraph_core::{
    BatchOp, CogniGraphError, CollectionType, Direction, DocumentId, FieldPredicate, GraphBackend,
    IndexDef, QueryLanguage, Result, SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};
use serde_json::Value;

mod request;
pub use request::{TenantContext, request_incarnation};

tokio::task_local! {
    /// The tenant of the request being served, set by the auth
    /// middleware for the whole handler future.
    pub static CURRENT_TENANT: String;
}

/// The current tenant, or the implicit default when no request scope is
/// active (auth disabled, startup work, tests).
pub fn current_tenant() -> String {
    CURRENT_TENANT
        .try_with(|t| t.clone())
        .unwrap_or_else(|_| DEFAULT_TENANT.to_string())
}

type StoreFactory = Box<dyn Fn(&str) -> Result<Arc<dyn GraphBackend>> + Send + Sync>;

/// Lazily opened per-tenant stores and caches. Stores are opened on
/// first touch via the factory main.rs builds from the deployment's
/// durability configuration.
pub struct TenantRegistry {
    factory: StoreFactory,
    stores: RwLock<HashMap<String, Arc<dyn GraphBackend>>>,
    caches: RwLock<HashMap<String, Arc<InMemoryCache>>>,
    cache_config: Option<CacheConfig>,
    /// Where the per-tenant files live; `None` = in-memory factories
    /// (tests) with nothing on disk to retire.
    data_dir: Option<std::path::PathBuf>,
}

impl TenantRegistry {
    pub fn new(factory: StoreFactory, cache_config: Option<CacheConfig>) -> Self {
        Self {
            factory,
            stores: RwLock::new(HashMap::new()),
            caches: RwLock::new(HashMap::new()),
            cache_config,
            data_dir: None,
        }
    }

    pub fn with_data_dir(mut self, dir: std::path::PathBuf) -> Self {
        self.data_dir = Some(dir);
        self
    }

    pub fn store(&self, tenant: &str) -> Result<Arc<dyn GraphBackend>> {
        if let Some(store) = self.stores.read().expect("registry lock").get(tenant) {
            return Ok(store.clone());
        }
        let mut stores = self.stores.write().expect("registry lock");
        if let Some(store) = stores.get(tenant) {
            return Ok(store.clone());
        }
        let store = (self.factory)(tenant)?;
        stores.insert(tenant.to_string(), store.clone());
        Ok(store)
    }

    fn cache(&self, tenant: &str) -> Option<Arc<InMemoryCache>> {
        let config = self.cache_config.as_ref()?;
        if let Some(cache) = self.caches.read().expect("registry lock").get(tenant) {
            return Some(cache.clone());
        }
        let mut caches = self.caches.write().expect("registry lock");
        Some(
            caches
                .entry(tenant.to_string())
                .or_insert_with(|| Arc::new(InMemoryCache::new(config.clone())))
                .clone(),
        )
    }

    /// Retire a deleted tenant's store (decision_tenant_deletion.md):
    /// evict the registry handles and quarantine the on-disk files —
    /// `{tenant}.redb`, text-index directories, vector sidecars, and unfinished
    /// vector builds (including legacy filenames) are renamed to
    /// `<entry>.deleted-{millis}` so a recreated tenant
    /// starts empty while the operator can still recover or purge.
    /// Returns the quarantined file names.
    ///
    /// The rename happens under the stores write lock — the same lock
    /// `store()` holds while its factory opens files — so a concurrent
    /// request cannot re-open the old path mid-retire. Requests already
    /// holding the evicted Arc keep an fd to the renamed inode; their
    /// writes land in the quarantined file and the handle closes when
    /// the last Arc drops.
    pub fn retire_store(&self, tenant: &str) -> Result<Vec<String>> {
        let live_name = safe_tenant_file(tenant)?;
        let mut stores = self.stores.write().expect("registry lock");
        stores.remove(tenant);
        self.caches.write().expect("registry lock").remove(tenant);

        let Some(dir) = &self.data_dir else {
            return Ok(Vec::new());
        };
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock before epoch")
            .as_millis();
        let derivative_prefix = format!("{tenant}.");
        let mut quarantined = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(io_err)? {
            let entry = entry.map_err(io_err)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_tenant_file = name == live_name
                || (name.starts_with(&derivative_prefix)
                    && (name.ends_with(".vectors")
                        || name.ends_with(".vectors.tmp")
                        || name.ends_with(".tantivy")));
            if !is_tenant_file {
                continue;
            }
            let mut target = format!("{name}.deleted-{millis}");
            let mut n = 0;
            while dir.join(&target).exists() {
                n += 1;
                target = format!("{name}.deleted-{millis}-{n}");
            }
            std::fs::rename(entry.path(), dir.join(&target)).map_err(io_err)?;
            quarantined.push(target);
        }
        Ok(quarantined)
    }

    /// Tenants with an open store (for metrics/health).
    pub fn open_tenants(&self) -> Vec<String> {
        self.stores
            .read()
            .expect("registry lock")
            .keys()
            .cloned()
            .collect()
    }
}

/// `GraphBackend` facade: admitted requests use their captured store; fenced
/// background work and startup use name-based registry resolution. This is the
/// single point where tenancy touches the data path.
pub struct RoutedBackend {
    registry: Arc<TenantRegistry>,
}

impl RoutedBackend {
    pub fn new(registry: Arc<TenantRegistry>) -> Self {
        Self { registry }
    }

    fn store(&self) -> Result<Arc<dyn GraphBackend>> {
        match request::request_store(&self.registry) {
            Some(store) => Ok(store),
            None => self.registry.store(&current_tenant()),
        }
    }
}

#[async_trait::async_trait]
impl GraphBackend for RoutedBackend {
    fn backend_name(&self) -> &str {
        "native-multitenant"
    }

    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Cgql
    }

    fn supports_atomic_batches(&self) -> bool {
        self.store()
            .is_ok_and(|backend| backend.supports_atomic_batches())
    }

    async fn ping(&self) -> Result<()> {
        self.store()?.ping().await
    }

    async fn create_document(&self, collection: &str, doc: Value) -> Result<DocumentId> {
        self.store()?.create_document(collection, doc).await
    }

    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<Value>> {
        self.store()?.get_document(collection, key).await
    }

    async fn update_document(&self, collection: &str, key: &str, update: Value) -> Result<Value> {
        self.store()?.update_document(collection, key, update).await
    }

    async fn replace_document(&self, collection: &str, key: &str, doc: Value) -> Result<Value> {
        self.store()?.replace_document(collection, key, doc).await
    }

    async fn delete_document(&self, collection: &str, key: &str) -> Result<bool> {
        self.store()?.delete_document(collection, key).await
    }

    async fn list_documents(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        self.store()?
            .list_documents(collection, limit, offset)
            .await
    }

    async fn list_collections(&self) -> Result<Vec<cognigraph_core::CollectionInfo>> {
        self.store()?.list_collections().await
    }

    async fn list_documents_projected(
        &self,
        collection: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        self.store()?
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
        self.store()?
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
        self.store()?
            .list_documents_filtered(collection, predicates, fields, limit, offset)
            .await
    }

    async fn export_snapshot(&self) -> Result<Value> {
        self.store()?.export_snapshot().await
    }

    async fn import_snapshot(&self, data: &Value) -> Result<()> {
        self.store()?.import_snapshot(data).await
    }

    async fn create_edge(&self, collection: &str, edge: Value) -> Result<DocumentId> {
        self.store()?.create_edge(collection, edge).await
    }

    async fn upsert_edge(
        &self,
        collection: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: Value,
    ) -> Result<Value> {
        self.store()?
            .upsert_edge(collection, from, to, relation_type, data)
            .await
    }

    async fn get_edges(
        &self,
        collection: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> Result<Vec<Value>> {
        self.store()?
            .get_edges(collection, vertex_id, direction)
            .await
    }

    async fn execute_batch(&self, ops: Vec<BatchOp>) -> Result<Vec<Value>> {
        self.store()?.execute_batch(ops).await
    }

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        self.store()?.traverse(start_vertex, opts).await
    }

    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        self.store()?
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
        self.store()?
            .text_search(collection, query, fields, limit)
            .await
    }

    async fn query(
        &self,
        query: &str,
        bind_vars: std::collections::HashMap<String, Value>,
    ) -> Result<Vec<Value>> {
        self.store()?.query(query, bind_vars).await
    }

    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()> {
        self.store()?.ensure_collection(name, collection_type).await
    }

    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()> {
        self.store()?.ensure_index(collection, index).await
    }

    async fn list_indexes(&self, collection: &str) -> Result<Vec<IndexDef>> {
        self.store()?.list_indexes(collection).await
    }

    async fn drop_index(&self, collection: &str, name: &str) -> Result<bool> {
        self.store()?.drop_index(collection, name).await
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        self.store()?.drop_collection(name).await
    }
}

/// A backend handle carrying the admitted tenant's identity and store/cache
/// handles: every call re-enters that context around the inner future. The
/// task-local set by the auth middleware covers only the handler
/// future — work that crosses `spawn_blocking` (the Lua engine) loses
/// it, and `RoutedBackend` would silently fall back to the DEFAULT
/// tenant: a cross-tenant read/write hole. Wrap `state.backend` in
/// this, capturing `current_tenant()` while still on the request path,
/// before handing it to any off-task executor.
pub struct TenantScoped {
    context: TenantContext,
    inner: Arc<dyn GraphBackend>,
}

impl TenantScoped {
    pub fn new(tenant: String, inner: Arc<dyn GraphBackend>) -> Self {
        Self {
            context: TenantContext::capture(tenant),
            inner,
        }
    }
}

macro_rules! scoped {
    ($self:ident, $call:expr) => {
        $self.context.clone().scope($call).await
    };
}

#[async_trait::async_trait]
impl GraphBackend for TenantScoped {
    fn backend_name(&self) -> &str {
        self.inner.backend_name()
    }

    fn query_language(&self) -> QueryLanguage {
        self.inner.query_language()
    }

    fn supports_atomic_batches(&self) -> bool {
        self.context
            .sync_scope(|| self.inner.supports_atomic_batches())
    }

    async fn ping(&self) -> Result<()> {
        scoped!(self, self.inner.ping())
    }

    async fn create_document(&self, collection: &str, doc: Value) -> Result<DocumentId> {
        scoped!(self, self.inner.create_document(collection, doc))
    }

    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<Value>> {
        scoped!(self, self.inner.get_document(collection, key))
    }

    async fn update_document(&self, collection: &str, key: &str, update: Value) -> Result<Value> {
        scoped!(self, self.inner.update_document(collection, key, update))
    }

    async fn replace_document(&self, collection: &str, key: &str, doc: Value) -> Result<Value> {
        scoped!(self, self.inner.replace_document(collection, key, doc))
    }

    async fn delete_document(&self, collection: &str, key: &str) -> Result<bool> {
        scoped!(self, self.inner.delete_document(collection, key))
    }

    async fn list_documents(
        &self,
        collection: &str,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        scoped!(self, self.inner.list_documents(collection, limit, offset))
    }

    async fn list_collections(&self) -> Result<Vec<cognigraph_core::CollectionInfo>> {
        scoped!(self, self.inner.list_collections())
    }

    async fn list_documents_projected(
        &self,
        collection: &str,
        fields: &[String],
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        scoped!(
            self,
            self.inner
                .list_documents_projected(collection, fields, limit, offset)
        )
    }

    async fn list_documents_after_key(
        &self,
        collection: &str,
        after_key: Option<&str>,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<Value>> {
        scoped!(
            self,
            self.inner
                .list_documents_after_key(collection, after_key, fields, limit)
        )
    }

    async fn list_documents_filtered(
        &self,
        collection: &str,
        predicates: &[FieldPredicate],
        fields: Option<&[String]>,
        limit: Option<usize>,
        offset: Option<usize>,
    ) -> Result<Vec<Value>> {
        scoped!(
            self,
            self.inner
                .list_documents_filtered(collection, predicates, fields, limit, offset)
        )
    }

    async fn export_snapshot(&self) -> Result<Value> {
        scoped!(self, self.inner.export_snapshot())
    }

    async fn import_snapshot(&self, data: &Value) -> Result<()> {
        scoped!(self, self.inner.import_snapshot(data))
    }

    async fn create_edge(&self, collection: &str, edge: Value) -> Result<DocumentId> {
        scoped!(self, self.inner.create_edge(collection, edge))
    }

    async fn upsert_edge(
        &self,
        collection: &str,
        from: &str,
        to: &str,
        relation_type: &str,
        data: Value,
    ) -> Result<Value> {
        scoped!(
            self,
            self.inner
                .upsert_edge(collection, from, to, relation_type, data)
        )
    }

    async fn get_edges(
        &self,
        collection: &str,
        vertex_id: &str,
        direction: Direction,
    ) -> Result<Vec<Value>> {
        scoped!(self, self.inner.get_edges(collection, vertex_id, direction))
    }

    async fn execute_batch(&self, ops: Vec<BatchOp>) -> Result<Vec<Value>> {
        scoped!(self, self.inner.execute_batch(ops))
    }

    async fn traverse(
        &self,
        start_vertex: &str,
        opts: &TraversalOpts,
    ) -> Result<Vec<TraversalPath>> {
        scoped!(self, self.inner.traverse(start_vertex, opts))
    }

    async fn vector_search(
        &self,
        collection: &str,
        query_vector: &[f64],
        opts: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        scoped!(
            self,
            self.inner.vector_search(collection, query_vector, opts)
        )
    }

    async fn text_search(
        &self,
        collection: &str,
        query: &str,
        fields: &[String],
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        scoped!(
            self,
            self.inner.text_search(collection, query, fields, limit)
        )
    }

    async fn query(
        &self,
        query: &str,
        bind_vars: std::collections::HashMap<String, Value>,
    ) -> Result<Vec<Value>> {
        scoped!(self, self.inner.query(query, bind_vars))
    }

    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()> {
        scoped!(self, self.inner.ensure_collection(name, collection_type))
    }

    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()> {
        scoped!(self, self.inner.ensure_index(collection, index))
    }

    async fn list_indexes(&self, collection: &str) -> Result<Vec<IndexDef>> {
        scoped!(self, self.inner.list_indexes(collection))
    }

    async fn drop_index(&self, collection: &str, name: &str) -> Result<bool> {
        scoped!(self, self.inner.drop_index(collection, name))
    }

    async fn drop_collection(&self, name: &str) -> Result<()> {
        scoped!(self, self.inner.drop_collection(name))
    }
}

/// `QueryCache` facade with per-tenant caches (D3: shared caches are a
/// cross-tenant side channel — a hit would reveal another tenant's
/// queries or embedded texts). Snapshot stats and management operations report
/// the active tenant's cache. The trait's borrowed `stats()` accessor cannot return a
/// reference through the registry's short-lived `Arc`, so callers should use
/// `stats_snapshot()`, which is overridden below to read the active tenant.
pub struct RoutedCache {
    registry: Arc<TenantRegistry>,
    fallback_stats: CacheStats,
    fallback_config: CacheConfig,
}

impl RoutedCache {
    pub fn new(registry: Arc<TenantRegistry>, config: CacheConfig) -> Self {
        Self {
            registry,
            fallback_stats: CacheStats::default(),
            fallback_config: config,
        }
    }

    fn cache(&self) -> Option<Arc<InMemoryCache>> {
        request::request_cache(&self.registry)
            .unwrap_or_else(|| self.registry.cache(&current_tenant()))
    }
}

#[async_trait::async_trait]
impl QueryCache for RoutedCache {
    async fn get_embedding(&self, query: &str, model: &str) -> Option<Vec<f64>> {
        self.cache()?.get_embedding(query, model).await
    }

    async fn put_embedding(&self, query: &str, model: &str, embedding: Vec<f64>) {
        if let Some(cache) = self.cache() {
            cache.put_embedding(query, model, embedding).await;
        }
    }

    async fn get_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<&[f64]>,
    ) -> Option<CacheHit> {
        self.cache()?.get_results(key, query_embedding).await
    }

    async fn put_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<Value>,
    ) {
        if let Some(cache) = self.cache() {
            cache.put_results(key, query_embedding, results, meta).await;
        }
    }

    fn result_generation(&self) -> u64 {
        self.cache().map_or(0, |cache| cache.result_generation())
    }

    async fn put_results_if_generation(
        &self,
        expected_generation: u64,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<Value>,
    ) -> bool {
        match self.cache() {
            Some(cache) => {
                cache
                    .put_results_if_generation(
                        expected_generation,
                        key,
                        query_embedding,
                        results,
                        meta,
                    )
                    .await
            }
            None => false,
        }
    }

    async fn invalidate(&self, key: &CacheKey) {
        if let Some(cache) = self.cache() {
            cache.invalidate(key).await;
        }
    }

    async fn invalidate_collection(&self, collection: &str) {
        if let Some(cache) = self.cache() {
            cache.invalidate_collection(collection).await;
        }
    }

    async fn invalidate_results(&self) {
        if let Some(cache) = self.cache() {
            cache.invalidate_results().await;
        }
    }

    async fn clear(&self) {
        if let Some(cache) = self.cache() {
            cache.clear().await;
        }
    }

    async fn entry_count(&self) -> usize {
        match self.cache() {
            Some(cache) => cache.entry_count().await,
            None => 0,
        }
    }

    fn stats(&self) -> &CacheStats {
        &self.fallback_stats
    }

    fn stats_snapshot(&self) -> CacheStatsSnapshot {
        self.cache().map_or_else(
            || self.fallback_stats.snapshot(),
            |cache| cache.stats_snapshot(),
        )
    }

    fn config(&self) -> &CacheConfig {
        &self.fallback_config
    }
}

fn io_err(e: std::io::Error) -> CogniGraphError {
    CogniGraphError::BackendError(format!("retire store: {e}"))
}

/// Guard against path tricks in tenant names reaching the filesystem.
/// (Creation already validates; this is defense in depth for the
/// factory path since legacy user docs could carry arbitrary strings.)
pub fn safe_tenant_file(tenant: &str) -> Result<String> {
    if tenant.is_empty()
        || tenant.len() > 63
        || !tenant
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(CogniGraphError::ValidationError(format!(
            "tenant name `{tenant}` is not filesystem-safe"
        )));
    }
    Ok(format!("{tenant}.redb"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_native::NativeBackend;

    fn registry() -> Arc<TenantRegistry> {
        Arc::new(TenantRegistry::new(
            Box::new(|_tenant| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            None,
        ))
    }

    #[tokio::test]
    async fn routed_backend_isolates_tenants_structurally() {
        let routed = RoutedBackend::new(registry());

        // Tenant A writes a document inside its request scope.
        CURRENT_TENANT
            .scope("acme".to_string(), async {
                routed
                    .create_document("docs", serde_json::json!({ "_key": "d1", "secret": true }))
                    .await
                    .unwrap();
                assert!(routed.get_document("docs", "d1").await.unwrap().is_some());
                let page = routed
                    .list_documents_after_key("docs", None, &["secret".into()], 10)
                    .await
                    .unwrap();
                assert_eq!(page.len(), 1);
                assert_eq!(page[0]["_key"], "d1");
            })
            .await;

        // Tenant B cannot see it — different store, not a filter.
        CURRENT_TENANT
            .scope("umbrella".to_string(), async {
                assert!(routed.get_document("docs", "d1").await.unwrap().is_none());
                routed
                    .create_document("docs", serde_json::json!({ "_key": "u1" }))
                    .await
                    .unwrap();
                let page = routed
                    .list_documents_after_key("docs", None, &[], 10)
                    .await
                    .unwrap();
                assert_eq!(page.len(), 1);
                assert_eq!(page[0]["_key"], "u1");
            })
            .await;

        // No scope at all = the implicit default tenant, also isolated.
        assert!(routed.get_document("docs", "d1").await.unwrap().is_none());
        assert_eq!(current_tenant(), DEFAULT_TENANT);
    }

    #[tokio::test]
    async fn routed_cache_reports_the_current_tenants_stats() {
        let config = CacheConfig::default();
        let registry = Arc::new(TenantRegistry::new(
            Box::new(|_tenant| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            Some(config.clone()),
        ));
        let routed = RoutedCache::new(registry.clone(), config);

        registry.cache("acme").unwrap().stats().record_hit_direct();
        registry.cache("umbrella").unwrap().stats().record_miss();

        CURRENT_TENANT
            .scope("acme".to_string(), async {
                let snapshot = routed.stats_snapshot();
                assert_eq!(snapshot.hits, 1);
                assert_eq!(snapshot.hits_direct, 1);
                assert_eq!(snapshot.misses, 0);
            })
            .await;
        CURRENT_TENANT
            .scope("umbrella".to_string(), async {
                let snapshot = routed.stats_snapshot();
                assert_eq!(snapshot.hits, 0);
                assert_eq!(snapshot.misses, 1);
            })
            .await;

        let default_snapshot = routed.stats_snapshot();
        assert_eq!(default_snapshot.hits, 0);
        assert_eq!(default_snapshot.misses, 0);
    }

    /// The D1 overhead question, measured: open 100 real per-tenant
    /// redb stores through the registry, write + read one document in
    /// each. Prints timings (visible with --nocapture); asserts only
    /// sanity bounds so CI stays robust.
    #[tokio::test]
    async fn hundred_tenant_overhead_dry_run() {
        let dir = std::env::temp_dir().join(format!("cg-tenancy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let factory_dir = dir.clone();
        let registry = Arc::new(TenantRegistry::new(
            Box::new(move |tenant| {
                let path = factory_dir.join(safe_tenant_file(tenant)?);
                Ok(
                    Arc::new(NativeBackend::open(path.to_string_lossy().as_ref())?)
                        as Arc<dyn GraphBackend>,
                )
            }),
            None,
        ));
        let routed = RoutedBackend::new(registry.clone());

        let t = std::time::Instant::now();
        for i in 0..100 {
            registry.store(&format!("tenant-{i:03}")).unwrap();
        }
        let open_ms = t.elapsed().as_millis();

        let t = std::time::Instant::now();
        for i in 0..100 {
            CURRENT_TENANT
                .scope(format!("tenant-{i:03}"), async {
                    routed
                        .create_document("docs", serde_json::json!({ "_key": "probe", "i": i }))
                        .await
                        .unwrap();
                    assert!(
                        routed
                            .get_document("docs", "probe")
                            .await
                            .unwrap()
                            .is_some()
                    );
                })
                .await;
        }
        let rw_ms = t.elapsed().as_millis();
        println!(
            "100 tenants: open {open_ms} ms total ({:.1} ms/store), write+read {rw_ms} ms",
            open_ms as f64 / 100.0
        );
        assert_eq!(registry.open_tenants().len(), 100);
        assert!(open_ms < 30_000, "opening 100 stores took {open_ms} ms");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn retire_store_evicts_the_handle_and_quarantines_the_file() {
        let dir = std::env::temp_dir().join(format!("cg-retire-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let factory_dir = dir.clone();
        let registry = Arc::new(
            TenantRegistry::new(
                Box::new(move |tenant| {
                    let path = factory_dir.join(safe_tenant_file(tenant)?);
                    Ok(Arc::new(NativeBackend::open_with_mode(
                        &path,
                        cognigraph_native::VectorMode::Sidecar,
                    )?) as Arc<dyn GraphBackend>)
                }),
                None,
            )
            .with_data_dir(dir.clone()),
        );
        let routed = RoutedBackend::new(registry.clone());

        CURRENT_TENANT
            .scope("acme".to_string(), async {
                routed
                    .create_document("docs", serde_json::json!({ "_key": "d1", "text":"oldvocabulary", "embedding":[1.0,0.0] }))
                    .await
                    .unwrap();
                routed.text_search("docs", "oldvocabulary", &["text".into()], 10).await.unwrap();
                routed.vector_search("docs", &[1.0,0.0], &VectorSearchOpts { limit: 10, threshold: None, model_name: None }).await.unwrap();
            })
            .await;
        assert!(registry.open_tenants().contains(&"acme".to_string()));

        let quarantined = registry.retire_store("acme").unwrap();

        // Handle evicted; database, text directory, and vector file quarantined.
        assert!(registry.open_tenants().is_empty());
        assert!(!dir.join("acme.redb").exists());
        assert_eq!(quarantined.len(), 3);
        let database = quarantined
            .iter()
            .find(|name| name.starts_with("acme.redb.deleted-"))
            .unwrap();
        assert!(
            quarantined
                .iter()
                .any(|name| name.contains(".tantivy.deleted-"))
        );
        assert!(
            quarantined
                .iter()
                .any(|name| name.contains(".vectors.deleted-"))
        );

        // The redb handle really closed: the quarantined file opens
        // fresh (redb refuses a second live handle on the same file)
        // and still holds the tenant's data — that's the recovery path.
        let recovered = NativeBackend::open(dir.join(database)).unwrap();
        assert!(
            recovered
                .get_document("docs", "d1")
                .await
                .unwrap()
                .is_some()
        );

        // A recreated tenant with the same name starts empty.
        CURRENT_TENANT
            .scope("acme".to_string(), async {
                assert!(routed.get_document("docs", "d1").await.unwrap().is_none());
            })
            .await;

        // Retiring a tenant that was never opened is a no-op, not an error.
        assert!(registry.retire_store("ghost").unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tenant_file_names_are_pinned_to_the_safe_charset() {
        assert_eq!(safe_tenant_file("acme").unwrap(), "acme.redb");
        assert!(safe_tenant_file("../escape").is_err());
        assert!(safe_tenant_file("Upper").is_err());
        assert!(safe_tenant_file("").is_err());
    }

    #[test]
    fn retirement_includes_legacy_indexes_and_pending_builds_only_for_its_tenant() {
        let dir = std::env::temp_dir().join(format!(
            "cg-retire-artifacts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let owned = [
            "acme.redb",
            "acme.redb.hash.tantivy",
            "acme.legacy.tantivy",
            "acme.redb.hash.vectors",
            "acme.redb.hash.vectors.tmp",
        ];
        let retained = [
            "acme2.redb",
            "acme2.redb.hash.tantivy",
            "acme.notes.txt",
            "acme.redb.deleted-before",
            "acme.redb.hash.tantivy.deleted-before",
        ];
        for name in owned.iter().chain(&retained) {
            if name.contains("tantivy") {
                std::fs::create_dir(dir.join(name)).unwrap();
                std::fs::write(dir.join(name).join("marker"), b"synthetic-index").unwrap();
            } else {
                std::fs::write(dir.join(name), b"synthetic-file").unwrap();
            }
        }
        let registry = TenantRegistry::new(Box::new(|_| Ok(Arc::new(NativeBackend::new()))), None)
            .with_data_dir(dir.clone());
        let quarantined = registry.retire_store("acme").unwrap();
        assert_eq!(quarantined.len(), owned.len());
        for name in owned {
            assert!(!dir.join(name).exists());
            let moved = quarantined
                .iter()
                .find(|moved| moved.starts_with(&format!("{name}.deleted-")))
                .unwrap();
            assert!(dir.join(moved).exists());
            if name.contains("tantivy") {
                assert_eq!(
                    std::fs::read(dir.join(moved).join("marker")).unwrap(),
                    b"synthetic-index"
                );
            }
        }
        for name in retained {
            assert!(dir.join(name).exists(), "unrelated entry moved: {name}");
        }
        assert!(registry.retire_store("acme").unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
