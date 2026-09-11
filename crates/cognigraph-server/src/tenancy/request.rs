//! Request admission captures identity and handles before lifecycle changes can
//! retire them. Name-only scopes remain available for fenced background work.

use std::future::Future;

use super::*;

tokio::task_local! {
    static REQUEST_TENANT: Option<Arc<RequestTenant>>;
}

struct RequestTenant {
    tenant: String,
    incarnation: String,
    storage: Option<RequestStorage>,
}

struct RequestStorage {
    registry: Arc<TenantRegistry>,
    store: Arc<dyn GraphBackend>,
    cache: Option<Arc<InMemoryCache>>,
}

/// Owned context that also survives explicit transfers to spawned tasks.
#[derive(Clone)]
pub struct TenantContext {
    tenant: String,
    request: Option<Arc<RequestTenant>>,
}

impl TenantContext {
    /// The caller must hold AppState's tenant lifecycle lock from identity
    /// validation through this capture, then release it before running a route.
    pub fn admitted(
        tenant: String,
        incarnation: String,
        registry: Option<&Arc<TenantRegistry>>,
    ) -> Result<Self> {
        let storage = registry
            .map(|registry| -> Result<RequestStorage> {
                Ok(RequestStorage {
                    registry: registry.clone(),
                    store: registry.store(&tenant)?,
                    cache: registry.cache(&tenant),
                })
            })
            .transpose()?;
        let request = Arc::new(RequestTenant {
            tenant: tenant.clone(),
            incarnation,
            storage,
        });
        Ok(Self {
            tenant,
            request: Some(request),
        })
    }

    pub fn capture(tenant: String) -> Self {
        let request = current_request(&tenant);
        Self { tenant, request }
    }

    pub async fn scope<F: Future>(self, future: F) -> F::Output {
        CURRENT_TENANT
            .scope(self.tenant, REQUEST_TENANT.scope(self.request, future))
            .await
    }

    pub fn sync_scope<T>(&self, call: impl FnOnce() -> T) -> T {
        CURRENT_TENANT.sync_scope(self.tenant.clone(), || {
            REQUEST_TENANT.sync_scope(self.request.clone(), call)
        })
    }
}

fn current_request(tenant: &str) -> Option<Arc<RequestTenant>> {
    REQUEST_TENANT
        .try_with(|request| {
            request
                .as_ref()
                .filter(|request| request.tenant == tenant)
                .cloned()
        })
        .ok()
        .flatten()
}

pub fn request_incarnation(tenant: &str) -> Option<String> {
    current_request(tenant).map(|request| request.incarnation.clone())
}

pub(super) fn request_store(registry: &Arc<TenantRegistry>) -> Option<Arc<dyn GraphBackend>> {
    let request = current_request(&current_tenant())?;
    let storage = request.storage.as_ref()?;
    Arc::ptr_eq(&storage.registry, registry).then(|| storage.store.clone())
}

// An outer Some means this request owns the cache decision, including disabled
// caching. Retirement must never cause a fallback to the new registry entry.
pub(super) fn request_cache(registry: &Arc<TenantRegistry>) -> Option<Option<Arc<InMemoryCache>>> {
    let request = current_request(&current_tenant())?;
    let storage = request.storage.as_ref()?;
    Arc::ptr_eq(&storage.registry, registry).then(|| storage.cache.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use cognigraph_native::NativeBackend;
    use serde_json::json;

    #[tokio::test]
    async fn lua_and_supervisor_keep_original_store_cache_and_incarnation_after_retirement() {
        let config = CacheConfig::default();
        let registry = Arc::new(TenantRegistry::new(
            Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            Some(config.clone()),
        ));
        let state = AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())))
            .with_cache(RoutedCache::new(registry.clone(), config));
        let admitted =
            TenantContext::admitted("acme".into(), "original".into(), Some(&registry)).unwrap();
        let (context, backend) = admitted
            .scope(async {
                (
                    TenantContext::capture(current_tenant()),
                    Arc::new(TenantScoped::new(current_tenant(), state.backend.clone()))
                        as Arc<dyn GraphBackend>,
                )
            })
            .await;
        let original_store = registry.store("acme").unwrap();
        let original_cache = registry.cache("acme").unwrap();
        registry.retire_store("acme").unwrap();
        let replacement = registry.store("acme").unwrap();
        replacement
            .create_document("notes", json!({"_key": "a", "text": "replacement"}))
            .await
            .unwrap();
        let replacement_cache = registry.cache("acme").unwrap();
        let generation = replacement_cache.result_generation();
        let worker_state = state.clone();
        tokio::spawn(context.scope(async move {
            assert_eq!(
                crate::jobs::JobManager::tenant_incarnation(&worker_state, "acme")
                    .await
                    .unwrap(),
                "original"
            );
            // Same blocking-engine and async-supervisor split as the Lua HTTP
            // route, after both tasks have lost the parent request scope.
            tokio::task::spawn_blocking(move || {
                assert_eq!(current_tenant(), DEFAULT_TENANT);
                assert!(backend.supports_atomic_batches());
                let engine = cognigraph_lua::LuaEngine::with_backend_mode(
                    backend,
                    tokio::runtime::Handle::current(),
                    true,
                )
                .unwrap();
                engine
                    .execute(
                        r#"return graph.create_document("notes", {_key="a", text="original"})"#,
                    )
                    .unwrap();
            })
            .await
            .unwrap();
            worker_state
                .cache
                .as_ref()
                .unwrap()
                .put_embedding("private query", "model", vec![1.0, 0.0])
                .await;
            worker_state.invalidate_search_results().await;
        }))
        .await
        .unwrap();
        assert_eq!(
            original_store
                .get_document("notes", "a")
                .await
                .unwrap()
                .unwrap()["text"],
            "original"
        );
        assert_eq!(
            replacement
                .get_document("notes", "a")
                .await
                .unwrap()
                .unwrap()["text"],
            "replacement"
        );
        assert_eq!(
            original_cache.get_embedding("private query", "model").await,
            Some(vec![1.0, 0.0])
        );
        assert!(
            replacement_cache
                .get_embedding("private query", "model")
                .await
                .is_none()
        );
        assert_eq!(replacement_cache.result_generation(), generation);
        assert!(
            !registry
                .open_tenants()
                .contains(&DEFAULT_TENANT.to_string())
        );
        state.jobs.shutdown().await;
    }
}
