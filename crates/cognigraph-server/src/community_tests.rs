//! Community request boundaries exercised with real auth and Native storage.

use crate::{
    auth_middleware::{self, ScopePolicy},
    state::AppState,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    middleware,
};
use cognigraph_auth::{AuthProvider, Role, Scope, User};
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

async fn setup() -> (AppState, User, User) {
    let raw = Arc::new(NativeBackend::new());
    let auth = AuthProvider::new(raw.clone()).await.unwrap();
    let admin = auth
        .create_user("admin", "test-password", Role::Admin)
        .await
        .unwrap();
    let host = auth
        .create_user("host-admin", "test-host-password", Role::HostAdmin)
        .await
        .unwrap();
    (
        AppState::new_shared(raw)
            .with_auth(auth)
            .with_jwt("test-secret".into(), 3600),
        admin,
        host,
    )
}

fn guarded(router: Router<AppState>, state: &AppState, scope: Scope) -> Router {
    router
        .route_layer(middleware::from_fn_with_state(
            (
                state.clone(),
                ScopePolicy {
                    read: scope,
                    write: scope,
                },
            ),
            auth_middleware::check,
        ))
        .with_state(state.clone())
}

async fn request(
    app: Router,
    method: &str,
    path: &str,
    user: Option<&User>,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(user) = user {
        let token = cognigraph_auth::jwt::issue(user, "test-secret", 3600);
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn tenant_lifecycle_refuses_without_creating_records() {
    let (state, _, host) = setup().await;
    let app = guarded(crate::routes::tenants::router(), &state, Scope::TenantAdmin);
    for (method, path, body) in [
        ("GET", "/", json!({})),
        ("POST", "/", json!({"name":"acme"})),
        ("POST", "/acme", json!({"status":"suspended"})),
        ("DELETE", "/acme", json!({})),
        (
            "POST",
            "/acme/admin",
            json!({"username":"x","password":"x"}),
        ),
        ("POST", "/acme/quotas", json!({"max_active_jobs":1})),
    ] {
        let (status, body) = request(app.clone(), method, path, Some(&host), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
        assert_eq!(body["code"], "enterprise_feature_required");
    }
    assert!(
        state
            .auth
            .as_ref()
            .unwrap()
            .list_tenants()
            .await
            .unwrap()
            .iter()
            .all(|t| t.name == "default")
    );
    assert_eq!(
        request(app, "POST", "/", None, json!({"name":"acme"}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn foreign_tenant_sessions_and_user_creation_are_refused() {
    let (state, admin, _) = setup().await;
    let auth = state.auth.as_ref().unwrap();
    auth.create_tenant("acme", None).await.unwrap();
    let foreign = auth
        .create_user_in("foreign", "test-password", Role::Admin, "acme")
        .await
        .unwrap();
    let app = guarded(crate::routes::users::router(), &state, Scope::Admin);
    for (user, body) in [
        (
            &foreign,
            json!({"username":"x","password":"x","role":"viewer"}),
        ),
        (
            &admin,
            json!({"username":"x","password":"x","role":"viewer","tenant":"acme"}),
        ),
    ] {
        let (status, body) = request(app.clone(), "POST", "/", Some(user), body).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "enterprise_feature_required");
    }
    let login = crate::routes::auth::router().with_state(state);
    let (status, body) = request(
        login,
        "POST",
        "/login",
        None,
        json!({"username":"foreign","password":"test-password"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["code"], "enterprise_feature_required");
}

#[tokio::test]
async fn governed_snapshot_rejection_precedes_any_write() {
    let (state, admin, _) = setup().await;
    let before = state.backend.export_snapshot().await.unwrap();
    let app = guarded(crate::routes::admin::router(), &state, Scope::Admin);
    for collection in [
        "neurons",
        "side_views",
        "_cognigraph_jobs",
        "_cognigraph_governance_keys",
    ] {
        let snapshot = json!({"collections": {
            "ordinary": {"type":"document", "documents":{"new":{"_key":"new"}}},
            collection: {"type":"document", "documents":{"x":{"_key":"x"}}}
        }});
        let (status, body) = request(app.clone(), "POST", "/import", Some(&admin), snapshot).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "enterprise_feature_required");
        assert_eq!(state.backend.export_snapshot().await.unwrap(), before);
    }
}

#[tokio::test]
async fn startup_requires_enterprise_for_existing_governed_records() {
    let raw = NativeBackend::new();
    raw.create_document("ordinary", json!({"_key":"one"}))
        .await
        .unwrap();
    crate::edition::validate_store(&raw).await.unwrap();
    raw.create_document("side_views", json!({"_key":"derived"}))
        .await
        .unwrap();
    let before = raw.export_snapshot().await.unwrap();
    assert!(matches!(
        crate::edition::validate_store(&raw).await,
        Err(cognigraph_core::CogniGraphError::EnterpriseFeatureRequired(
            _
        ))
    ));
    assert_eq!(raw.export_snapshot().await.unwrap(), before);
}

#[test]
fn community_spec_has_only_community_operations_and_resolvable_references() {
    fn check(value: &Value, spec: &Value) {
        match value {
            Value::Object(map) => {
                if let Some(reference) = map.get("$ref").and_then(Value::as_str) {
                    assert!(
                        spec.pointer(reference.strip_prefix('#').unwrap()).is_some(),
                        "{reference}"
                    );
                }
                for value in map.values() {
                    check(value, spec);
                }
            }
            Value::Array(values) => {
                for value in values {
                    check(value, spec);
                }
            }
            _ => {}
        }
    }
    let spec: Value = serde_json::from_str(crate::edition::OPENAPI).unwrap();
    assert_eq!(spec["info"]["version"], env!("CARGO_PKG_VERSION"));
    for path in spec["paths"].as_object().unwrap().keys() {
        assert!(
            ![
                "/api/construct",
                "/api/neurons",
                "/api/tenants",
                "/api/jobs",
                "/api/governance",
                "/api/promotions",
                "/api/semantic-repairs",
                "/api/sideviews",
                "/health/jobs",
                "/health/promotions",
                "/api/admin/jobs",
                "/api/admin/promotions",
                "/api/admin/artifact-custody"
            ]
            .iter()
            .any(|prefix| path.starts_with(prefix)),
            "{path}"
        );
    }
    for path in [
        "/api/documents",
        "/api/query",
        "/api/search/graph-augmented",
        "/api/admin/import",
        "/health/database",
    ] {
        assert!(spec["paths"].get(path).is_some(), "{path}");
    }
    assert!(spec["paths"]["/api/search/graph-augmented"]["post"]["requestBody"]
        ["content"]["application/json"]["schema"]["properties"]
        .get("neurons_collection").is_none());
    check(&spec, &spec);
}
