//! CG-108: requests rejected by the auth middleware, driven through the real
//! middleware, the metrics layer and the admin logs route on a multi-tenant
//! server.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware;
use cognigraph_auth::{AuthProvider, Role, Scope, TenantStatus};
use cognigraph_cache::CacheConfig;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::Value;
use tower::ServiceExt;

use super::{ScopePolicy, check};
use crate::hardening::{Metrics, Observability, record_metrics};
use crate::state::AppState;
use crate::tenancy::{RoutedBackend, TenantRegistry};

struct Fixture {
    app: Router,
    auth: Arc<AuthProvider>,
    acme_admin: String,
    beta_admin: String,
    beta_viewer: String,
}

fn guard(state: &AppState, read: Scope, write: Scope) -> (AppState, ScopePolicy) {
    (state.clone(), ScopePolicy { read, write })
}

impl Fixture {
    async fn new() -> Self {
        let auth = Arc::new(
            AuthProvider::new(Arc::new(NativeBackend::new()))
                .await
                .unwrap(),
        );
        let mut tokens = Vec::new();
        for (tenant, name, role) in [
            ("acme", "acme-admin", Role::Admin),
            ("beta", "beta-admin", Role::Admin),
            ("beta", "beta-viewer", Role::Viewer),
        ] {
            if auth.get_tenant(tenant).await.unwrap().is_none() {
                auth.create_tenant(tenant, None).await.unwrap();
            }
            let user = auth
                .create_user_in(name, "password", role, tenant)
                .await
                .unwrap();
            tokens.push(cognigraph_auth::jwt::issue(&user, "test-secret", 3600));
        }
        let registry = Arc::new(TenantRegistry::new(
            Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            Some(CacheConfig::default()),
        ));
        let mut state = AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())))
            .with_jwt("test-secret".into(), 3600);
        state.auth = Some(auth.clone());
        state.tenant_registry = Some(registry);

        let documents =
            crate::routes::documents::router().route_layer(middleware::from_fn_with_state(
                guard(&state, Scope::DocumentsRead, Scope::DocumentsWrite),
                check,
            ));
        let admin = crate::routes::admin::router().route_layer(middleware::from_fn_with_state(
            guard(&state, Scope::Admin, Scope::Admin),
            check,
        ));
        let observability = Observability {
            metrics: Arc::new(Metrics::new()),
            errors: state.recent_errors.clone(),
        };
        let app = Router::new()
            .nest(
                "/api",
                Router::new()
                    .nest("/documents", documents)
                    .nest("/admin", admin),
            )
            .with_state(state)
            .layer(middleware::from_fn_with_state(
                observability,
                record_metrics,
            ));
        let mut tokens = tokens.into_iter();
        Self {
            app,
            auth,
            acme_admin: tokens.next().unwrap(),
            beta_admin: tokens.next().unwrap(),
            beta_viewer: tokens.next().unwrap(),
        }
    }

    async fn send(&self, method: &str, path: &str, token: Option<&str>) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn logs(&self, token: &str) -> Vec<Value> {
        let (status, body) = self.send("GET", "/api/admin/logs", Some(token)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["events"].as_array().unwrap().clone()
    }
}

fn text(events: &[Value]) -> String {
    serde_json::to_string(events).unwrap()
}

#[tokio::test]
async fn another_tenants_rejected_requests_are_invisible_and_unattributed_ones_are_redacted() {
    let fixture = Fixture::new().await;

    // Tenant beta: a viewer attempts a write. Rejected after the caller is known.
    let (status, _) = fixture
        .send(
            "DELETE",
            "/api/documents/patients/p-123",
            Some(&fixture.beta_viewer),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Nobody in particular: no token, and a token that is not one.
    let (status, _) = fixture
        .send("GET", "/api/documents/anonymous_probe/doc-9", None)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = fixture
        .send(
            "GET",
            "/api/documents/forged_probe/doc-7",
            Some("not-a-token"),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Tenant beta is suspended; its admin's next request names the tenant.
    fixture
        .auth
        .set_tenant_status("beta", TenantStatus::Suspended)
        .await
        .unwrap();
    let (status, body) = fixture
        .send(
            "GET",
            "/api/documents/ledger/entry-1",
            Some(&fixture.beta_admin),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body["error"].as_str().unwrap().contains("beta"), "{body}");

    // What tenant acme's admin sees.
    let acme = fixture.logs(&fixture.acme_admin).await;
    let seen = text(&acme);
    for secret in [
        "patients",
        "p-123",
        "ledger",
        "entry-1",
        "beta",
        "anonymous_probe",
        "forged_probe",
        "doc-9",
    ] {
        assert!(
            !seen.contains(secret),
            "acme's admin can read `{secret}`: {seen}"
        );
    }
    // The two unattributed rejections are listed, reduced to their route.
    let unattributed: Vec<&Value> = acme
        .iter()
        .filter(|event| event["tenant"].is_null())
        .collect();
    assert_eq!(unattributed.len(), 2, "{seen}");
    for event in unattributed {
        assert_eq!(event["path"], "/api/documents/…", "{event}");
        assert_eq!(event["status"], 401, "{event}");
    }
    // Nothing of beta's is listed at all.
    assert!(acme.iter().all(|event| event["tenant"] != "beta"), "{seen}");

    // The suspended tenant's own admin is locked out, so check the tags
    // through acme's view of the raw ring instead: reactivate and read.
    fixture
        .auth
        .set_tenant_status("beta", TenantStatus::Active)
        .await
        .unwrap();
    let beta = fixture.logs(&fixture.beta_admin).await;
    let beta_paths: Vec<&str> = beta
        .iter()
        .filter(|event| event["tenant"] == "beta")
        .map(|event| event["path"].as_str().unwrap())
        .collect();
    assert!(
        beta_paths.contains(&"/api/documents/patients/p-123"),
        "{beta_paths:?}"
    );
    assert!(
        beta_paths.contains(&"/api/documents/ledger/entry-1"),
        "{beta_paths:?}"
    );
}
