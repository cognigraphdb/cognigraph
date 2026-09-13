use std::sync::Arc;

use cognigraph_auth::AuthProvider;
use cognigraph_cache::QueryCache;
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::EmbeddingProvider;
#[cfg(feature = "enterprise")]
use cognigraph_embeddings::completion::CompletionProvider;

/// Shared application state available to all route handlers.
pub type AuthArc = Arc<AuthProvider>;

#[derive(Clone)]
pub struct AppState {
    /// Public route facade. It hides system collections and makes Semantic
    /// Neuron authority plus derived graph collections read-only.
    pub backend: Arc<dyn GraphBackend>,
    /// Raw tenant-scoped backend for typed internal construction, repair, and
    /// governance workflows. Never expose this handle to generic data routes.
    #[cfg(any(test, feature = "enterprise"))]
    pub(crate) managed_backend: Arc<dyn GraphBackend>,
    #[cfg(feature = "enterprise")]
    pub(crate) side_views: Arc<crate::side_views::SideViews>,
    #[cfg(feature = "enterprise")]
    pub(crate) neuron_lifecycle: Arc<crate::neuron_lifecycle::NeuronLifecycle>,
    pub embedder: Option<Arc<dyn EmbeddingProvider>>,
    /// Completion model for gap-directed neuron proposing
    /// (POST /api/construct/propose); independent of the embedding provider.
    #[cfg(feature = "enterprise")]
    pub completion: Option<Arc<dyn CompletionProvider>>,
    /// Provider for context-expansion side-view generation (POST
    /// /api/sideviews/generate), resolved SEPARATELY from `completion` so it can
    /// run on a cheaper model (decision_sideviews_provider_and_benchmark.md, D2).
    /// Absent = side-view generation is disabled.
    #[cfg(feature = "enterprise")]
    pub sideviews_completion: Option<Arc<dyn CompletionProvider>>,
    /// Review judge for POST /api/construct/review — separable from the
    /// proposer (COGNIGRAPH_JUDGE_MODEL) for separation of duties;
    /// falls back to the completion provider when unset.
    #[cfg(feature = "enterprise")]
    pub judge: Option<Arc<dyn CompletionProvider>>,
    /// Second judge for the agreement lane (Lane A+,
    /// COGNIGRAPH_JUDGE_PARTNER_MODEL): concordance of two judges with
    /// measured-disjoint blind spots extends auto-accept to the
    /// direction-critical hint class (decision_agreement_lane.md).
    /// Absent = Lane A+ degrades to queue; Lane A is unaffected.
    #[cfg(feature = "enterprise")]
    pub judge_partner: Option<Arc<dyn CompletionProvider>>,
    pub cache: Option<Arc<dyn QueryCache>>,
    pub auth: Option<AuthArc>,
    /// Multi-tenant registry used by admission to pin request handles and by
    /// lifecycle/observability routes. Data access uses the RoutedBackend facade.
    #[cfg(feature = "enterprise")]
    pub tenant_registry: Option<Arc<crate::tenancy::TenantRegistry>>,
    /// Serializes control-plane tenant create/bootstrap/update/delete through
    /// store retirement and request admission so a recreated incarnation cannot
    /// race its predecessor. Never held while executing an admitted handler.
    pub tenant_lifecycle_lock: Arc<tokio::sync::Mutex<()>>,
    /// (secret, ttl_secs) — JWT sessions enabled when set.
    pub jwt: Option<Arc<(String, u64)>>,
    pub cgql_budget: cognigraph_query::ExecutionBudget,
    pub lua_instruction_limit: u32,
    /// Default TTL for newly issued API tokens (0 = non-expiring).
    pub token_ttl_secs: u64,
    /// Externally pinned M19 governance-root public key. This value comes from
    /// process configuration, never tenant storage or a snapshot.
    #[cfg(feature = "enterprise")]
    pub governance_root_public_key: Option<String>,
    /// Optional operator-managed, read-only source for verified M21 artifact
    /// bytes. `None` keeps artifact consumption disabled and fail-closed.
    #[cfg(feature = "enterprise")]
    pub artifact_cas: Option<Arc<crate::artifact_cas::LocalArtifactCas>>,
    /// Maximum distinct blob bytes projected into one read-only M24 artifact
    /// recovery plan. This is independent of whether live CAS consumption is
    /// enabled because plan derivation never dereferences artifact locations.
    #[cfg(feature = "enterprise")]
    pub artifact_max_custody_bytes: u64,
    /// SHA-256 of the server executable opened and pinned before the HTTP
    /// listener starts. M21 jobs never re-resolve a mutable executable path.
    #[cfg(feature = "enterprise")]
    pub artifact_executable_digest: Option<String>,
    /// Tenant-local durable operation repository and the singleton fair
    /// dispatcher shared across tenant queues.
    #[cfg(feature = "enterprise")]
    pub jobs: Arc<crate::jobs::JobManager>,
    /// Immutable M18 evaluation evidence, attributed promotion decisions,
    /// and repairable per-target promotion heads.
    #[cfg(feature = "enterprise")]
    pub promotions: Arc<crate::promotions::PromotionManager>,
    /// Bounded ring of recent non-2xx responses (the console's server-logs
    /// panel reads it via GET /api/admin/logs). Shared with the metrics
    /// middleware, which writes it.
    pub recent_errors: std::sync::Arc<crate::hardening::RecentErrors>,
}

impl AppState {
    /// Flush every cached search result for the active tenant while keeping
    /// deterministic query embeddings warm. Search responses can depend on
    /// source documents, embeddings, edges, and neurons simultaneously, so a
    /// single-collection invalidation is not a safe write barrier.
    pub async fn invalidate_search_results(&self) {
        if let Some(cache) = self.cache.as_deref() {
            cache.invalidate_results().await;
        }
    }

    /// Wrap a concrete backend. Production wiring uses `new_shared`
    /// (main.rs keeps the raw handle for the AuthProvider), so this
    /// convenience form survives for the route tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(backend: impl GraphBackend + 'static) -> Self {
        Self::new_shared(Arc::new(backend))
    }

    /// Like `new`, but sharing an existing backend handle (auth and data
    /// over the same store in tests and single-store deployments; the
    /// M2 tenant registry constructs states this way per tenant).
    ///
    /// Every state wraps its public backend in the collection guard, so
    /// underscore-prefixed control storage is unreachable and governed
    /// Semantic Neuron collections are read-only through generic routes.
    /// Components that legitimately need control storage keep their own raw
    /// handles; typed construction/governance routes use `managed_backend`.
    pub fn new_shared(backend: Arc<dyn GraphBackend>) -> Self {
        #[cfg(feature = "enterprise")]
        let jobs = Arc::new(crate::jobs::JobManager::new(backend.clone(), 25));
        #[cfg(feature = "enterprise")]
        let promotions = Arc::new(crate::promotions::PromotionManager::new(
            backend.clone(),
            jobs.clone(),
        ));
        #[cfg(feature = "enterprise")]
        let side_views = Arc::new(crate::side_views::SideViews::default());
        Self {
            #[cfg(not(feature = "enterprise"))]
            backend: Arc::new(crate::system_collections::GuardedBackend::new(
                backend.clone(),
            )),
            #[cfg(feature = "enterprise")]
            backend: Arc::new(crate::system_collections::GuardedBackend::with_side_views(
                backend.clone(),
                side_views.clone(),
            )),
            #[cfg(feature = "enterprise")]
            side_views,
            #[cfg(feature = "enterprise")]
            neuron_lifecycle: Arc::new(crate::neuron_lifecycle::NeuronLifecycle::default()),
            #[cfg(any(test, feature = "enterprise"))]
            managed_backend: backend,
            embedder: None,
            #[cfg(feature = "enterprise")]
            completion: None,
            #[cfg(feature = "enterprise")]
            sideviews_completion: None,
            #[cfg(feature = "enterprise")]
            judge: None,
            #[cfg(feature = "enterprise")]
            judge_partner: None,
            cache: None,
            auth: None,
            #[cfg(feature = "enterprise")]
            tenant_registry: None,
            tenant_lifecycle_lock: Arc::new(tokio::sync::Mutex::new(())),
            jwt: None,
            cgql_budget: cognigraph_query::ExecutionBudget::default(),
            lua_instruction_limit: 0,
            token_ttl_secs: 0,
            #[cfg(feature = "enterprise")]
            governance_root_public_key: None,
            #[cfg(feature = "enterprise")]
            artifact_cas: None,
            #[cfg(feature = "enterprise")]
            artifact_max_custody_bytes: crate::config::DEFAULT_ARTIFACT_MAX_CUSTODY_BYTES,
            #[cfg(feature = "enterprise")]
            artifact_executable_digest: None,
            #[cfg(feature = "enterprise")]
            jobs,
            #[cfg(feature = "enterprise")]
            promotions,
            recent_errors: std::sync::Arc::new(crate::hardening::RecentErrors::new(200)),
        }
    }

    pub fn with_embedder(mut self, embedder: impl EmbeddingProvider + 'static) -> Self {
        self.embedder = Some(Arc::new(embedder));
        self
    }

    #[cfg(all(test, feature = "enterprise"))]
    pub fn with_completion(mut self, provider: impl CompletionProvider + 'static) -> Self {
        self.completion = Some(Arc::new(provider));
        self
    }

    /// Wire the side-view generation provider. Takes an already-boxed handle
    /// because it is resolved via `CompletionConfig` (which yields a
    /// `Box<dyn CompletionProvider>`), independently of the completion provider.
    #[cfg(feature = "enterprise")]
    pub fn with_sideviews_completion(mut self, provider: Arc<dyn CompletionProvider>) -> Self {
        self.sideviews_completion = Some(provider);
        self
    }

    #[cfg(all(test, feature = "enterprise"))]
    pub fn with_judge(mut self, provider: impl CompletionProvider + 'static) -> Self {
        self.judge = Some(Arc::new(provider));
        self
    }

    #[cfg(all(test, feature = "enterprise"))]
    pub fn with_judge_partner(mut self, provider: impl CompletionProvider + 'static) -> Self {
        self.judge_partner = Some(Arc::new(provider));
        self
    }

    pub fn with_cache(mut self, cache: impl QueryCache + 'static) -> Self {
        self.cache = Some(Arc::new(cache));
        self
    }

    pub fn with_auth(mut self, auth: AuthProvider) -> Self {
        self.auth = Some(Arc::new(auth));
        self
    }

    pub fn with_jwt(mut self, secret: String, ttl_secs: u64) -> Self {
        self.jwt = Some(Arc::new((secret, ttl_secs)));
        self
    }
}
