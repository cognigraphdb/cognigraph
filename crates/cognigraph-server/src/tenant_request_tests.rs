//! Exercise real middleware and routes while a synchronous provider is paused.
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware;
use cognigraph_auth::{AuthProvider, Role, Scope};
use cognigraph_cache::CacheConfig;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tower::ServiceExt;

use super::{ScopePolicy, check};
use crate::state::AppState;
use crate::tenancy::{RoutedBackend, RoutedCache, TenantRegistry};

#[derive(Clone, Default)]
struct PausedEmbedder {
    entered: Arc<Notify>,
    resume: Arc<Notify>,
}

#[async_trait::async_trait]
impl cognigraph_embeddings::EmbeddingProvider for PausedEmbedder {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>> {
        self.entered.notify_one();
        self.resume.notified().await;
        Ok(vec![vec![1.0, 0.0]; texts.len()])
    }
}

struct Fixture {
    state: AppState,
    registry: Arc<TenantRegistry>,
    app: Router,
    tenant_token: String,
    host_token: String,
    embedder: PausedEmbedder,
}

impl Fixture {
    async fn new() -> Self {
        let auth = Arc::new(
            AuthProvider::new(Arc::new(NativeBackend::new()))
                .await
                .unwrap(),
        );
        auth.create_tenant("acme", None).await.unwrap();
        let user = auth
            .create_user_in("acme-admin", "password", Role::Admin, "acme")
            .await
            .unwrap();
        let host = auth
            .create_user("host", "password", Role::HostAdmin)
            .await
            .unwrap();
        let tenant_token = cognigraph_auth::jwt::issue(&user, "test-secret", 3600);
        let host_token = cognigraph_auth::jwt::issue(&host, "test-secret", 3600);
        let registry = Arc::new(TenantRegistry::new(
            Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            Some(CacheConfig::default()),
        ));
        let embedder = PausedEmbedder::default();
        let mut state = AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())))
            .with_jwt("test-secret".into(), 3600)
            .with_embedder(embedder.clone())
            .with_cache(RoutedCache::new(registry.clone(), CacheConfig::default()));
        state.auth = Some(auth);
        state.tenant_registry = Some(registry.clone());
        let documents =
            crate::routes::documents::router().route_layer(middleware::from_fn_with_state(
                (
                    state.clone(),
                    ScopePolicy {
                        read: Scope::DocumentsRead,
                        write: Scope::DocumentsWrite,
                    },
                ),
                check,
            ));
        let tenants = crate::routes::tenants::router().route_layer(middleware::from_fn_with_state(
            (
                state.clone(),
                ScopePolicy {
                    read: Scope::TenantAdmin,
                    write: Scope::TenantAdmin,
                },
            ),
            check,
        ));
        let app = Router::new()
            .nest("/documents", documents)
            .nest("/tenants", tenants)
            .with_state(state.clone());
        Self {
            state,
            registry,
            app,
            tenant_token,
            host_token,
            embedder,
        }
    }

    fn request(&self, method: &str, path: &str, body: Value, host: bool) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(path)
            .header(
                "authorization",
                format!(
                    "Bearer {}",
                    if host {
                        &self.host_token
                    } else {
                        &self.tenant_token
                    }
                ),
            )
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn call(&self, method: &str, path: &str, body: Value, host: bool) -> StatusCode {
        tokio::time::timeout(
            Duration::from_secs(5),
            self.app
                .clone()
                .oneshot(self.request(method, path, body, host)),
        )
        .await
        .expect("route deadlocked")
        .unwrap()
        .status()
    }

    async fn paused_request(&self) -> tokio::task::JoinHandle<StatusCode> {
        let request = self.request("POST", "/documents/embed", json!({
            "collection": "notes", "items": [{"_key": "a", "text": "old request"}], "upsert": true,
        }), false);
        let app = self.app.clone();
        let task = tokio::spawn(async move { app.oneshot(request).await.unwrap().status() });
        tokio::time::timeout(Duration::from_secs(5), self.embedder.entered.notified())
            .await
            .expect("provider was not called");
        task
    }
}

async fn deletion_during_embedding(recreate_before_resume: bool) {
    let fixture = Fixture::new().await;
    // No data access has opened this tenant yet. Admission must capture the
    // store even though embedding's first backend operation follows the await.
    assert!(fixture.registry.open_tenants().is_empty());
    let task = fixture.paused_request().await;
    let original = fixture.registry.store("acme").unwrap();
    assert_eq!(
        fixture
            .call("DELETE", "/tenants/acme", Value::Null, true)
            .await,
        StatusCode::OK
    );
    assert!(fixture.registry.open_tenants().is_empty());
    let replacement = if recreate_before_resume {
        assert_eq!(
            fixture
                .call("POST", "/tenants", json!({"name": "acme"}), true)
                .await,
            StatusCode::OK
        );
        let store = fixture.registry.store("acme").unwrap();
        store
            .create_document("notes", json!({"_key": "a", "text": "replacement"}))
            .await
            .unwrap();
        Some(store)
    } else {
        None
    };
    fixture.embedder.resume.notify_one();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        StatusCode::OK
    );
    let old_document = original.get_document("notes", "a").await.unwrap().unwrap();
    assert_eq!(old_document["text"], "old request");
    assert_eq!(old_document["embedding"], json!([1.0, 0.0]));
    if let Some(replacement) = replacement {
        let document = replacement
            .get_document("notes", "a")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(document["text"], "replacement");
        assert!(document.get("embedding").is_none());
    } else {
        assert!(
            fixture.registry.open_tenants().is_empty(),
            "old request reopened a deleted tenant"
        );
        assert_eq!(
            fixture
                .call("POST", "/tenants", json!({"name": "acme"}), true)
                .await,
            StatusCode::OK
        );
        assert!(
            fixture
                .registry
                .store("acme")
                .unwrap()
                .get_document("notes", "a")
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        fixture
            .call("GET", "/documents/notes/a", Value::Null, false)
            .await,
        StatusCode::UNAUTHORIZED
    );
    fixture.state.jobs.shutdown().await;
}

#[tokio::test]
async fn admitted_embedding_never_opens_a_deleted_tenant() {
    deletion_during_embedding(false).await;
}

#[tokio::test]
async fn admitted_embedding_cannot_overwrite_a_recreated_tenant() {
    deletion_during_embedding(true).await;
}

#[tokio::test]
async fn suspension_stops_new_requests_while_admitted_embedding_drains() {
    let fixture = Fixture::new().await;
    let task = fixture.paused_request().await;
    assert_eq!(
        fixture
            .call(
                "POST",
                "/tenants/acme",
                json!({"status": "suspended"}),
                true
            )
            .await,
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .call("GET", "/documents/notes/a", Value::Null, false)
            .await,
        StatusCode::FORBIDDEN
    );
    fixture.embedder.resume.notify_one();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .call("POST", "/tenants/acme", json!({"status": "active"}), true)
            .await,
        StatusCode::OK
    );
    assert_eq!(
        fixture
            .call("GET", "/documents/notes/a", Value::Null, false)
            .await,
        StatusCode::OK
    );
    fixture.state.jobs.shutdown().await;
}

#[tokio::test]
async fn admission_waits_for_lifecycle_then_rechecks_identity_before_opening_store() {
    let fixture = Fixture::new().await;
    let lifecycle = fixture.state.tenant_lifecycle_lock.lock().await;
    let request = fixture.request("GET", "/documents/notes/a", Value::Null, false);
    let app = fixture.app.clone();
    let mut task = tokio::spawn(async move { app.oneshot(request).await.unwrap().status() });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut task)
            .await
            .is_err()
    );
    let auth = fixture.state.auth.as_ref().unwrap();
    auth.delete_tenant("acme").await.unwrap();
    fixture.registry.retire_store("acme").unwrap();
    auth.create_tenant("acme", None).await.unwrap();
    drop(lifecycle);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        StatusCode::UNAUTHORIZED
    );
    assert!(fixture.registry.open_tenants().is_empty());
    fixture.state.jobs.shutdown().await;
}

#[tokio::test]
async fn admitted_user_administration_rechecks_under_lifecycle_lock() {
    let fixture = Fixture::new().await;
    // Start at the handler with an already-admitted identity, so this verifies
    // the control-store fence independently of the middleware admission lock.
    let actor = cognigraph_auth::jwt::verify(&fixture.tenant_token, "test-secret").unwrap();
    let app = crate::routes::users::router().with_state(fixture.state.clone());
    let mut request = fixture.request(
        "POST",
        "/",
        json!({
            "username": "stale-created-user", "password": "password", "role": "admin",
        }),
        false,
    );
    request.extensions_mut().insert(actor);
    let lifecycle = fixture.state.tenant_lifecycle_lock.lock().await;
    let mut task = tokio::spawn(async move { app.oneshot(request).await.unwrap().status() });
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut task)
            .await
            .is_err()
    );
    let auth = fixture.state.auth.as_ref().unwrap();
    auth.delete_tenant("acme").await.unwrap();
    fixture.registry.retire_store("acme").unwrap();
    auth.create_tenant("acme", None).await.unwrap();
    drop(lifecycle);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap(),
        StatusCode::UNAUTHORIZED
    );
    assert!(
        !auth
            .list_users()
            .await
            .unwrap()
            .iter()
            .any(|user| user.username == "stale-created-user")
    );
    fixture.state.jobs.shutdown().await;
}
