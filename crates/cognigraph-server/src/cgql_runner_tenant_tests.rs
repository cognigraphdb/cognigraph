//! CG-197: a query that runs on a blocking thread still reads and sweeps the
//! store and cache of the tenant its request was admitted for.

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use cognigraph_cache::CacheConfig;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::state::AppState;
use crate::tenancy::{RoutedBackend, RoutedCache, TenantContext, TenantRegistry};

async fn ask(app: &Router, route: &str, text: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(route)
        .header("content-type", "application/json")
        .body(Body::from(json!({ "query": text }).to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_query_reads_writes_and_sweeps_only_its_own_tenant() {
    let config = CacheConfig::default();
    let registry = Arc::new(TenantRegistry::new(
        Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
        Some(config.clone()),
    ));
    let state = AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())))
        .with_cache(RoutedCache::new(registry.clone(), config));
    let app = Router::new()
        .nest("/api/query", crate::routes::query::router())
        .nest("/api/search", crate::routes::search::router())
        .with_state(state.clone());
    for tenant in ["acme", "globex"] {
        registry
            .store(tenant)
            .unwrap()
            .create_document("notes", json!({"_key": "a", "owner": tenant}))
            .await
            .unwrap();
    }
    // The tenant's own cache, as a request admitted for it sees it.
    let generation = |tenant: &'static str| {
        let context =
            TenantContext::admitted(tenant.into(), "first".into(), Some(&registry)).unwrap();
        let cache = state.cache.clone().unwrap();
        async move { context.scope(async { cache.result_generation() }).await }
    };
    let before = (generation("acme").await, generation("globex").await);

    let acme = TenantContext::admitted("acme".into(), "first".into(), Some(&registry)).unwrap();
    acme.scope(async {
        for route in ["/api/query", "/api/search/query"] {
            let (status, body) = ask(&app, route, "FOR n IN notes RETURN n.owner").await;
            assert_eq!(status, StatusCode::OK, "{route} {body}");
            assert_eq!(body["results"], json!(["acme"]), "{route}");
        }
        let (status, body) = ask(
            &app,
            "/api/query",
            r#"INSERT { _key: "b", owner: "acme" } INTO notes"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
    })
    .await;

    assert!(generation("acme").await > before.0);
    assert_eq!(generation("globex").await, before.1);
    let count = |tenant: &'static str| {
        let registry = registry.clone();
        async move {
            registry
                .store(tenant)
                .unwrap()
                .list_documents("notes", None, None)
                .await
                .unwrap()
                .len()
        }
    };
    assert_eq!(count("acme").await, 2);
    assert_eq!(count("globex").await, 1);
}
