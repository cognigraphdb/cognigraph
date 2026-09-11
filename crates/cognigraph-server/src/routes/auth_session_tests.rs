use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use cognigraph_auth::{AuthProvider, Role, TenantStatus};
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::state::AppState;

async fn read(state: &AppState, token: Option<&str>) -> (StatusCode, Value) {
    let mut request = Request::get("/session");
    if let Some(token) = token {
        request = request.header("authorization", format!("Bearer {token}"));
    }
    let response = super::router(state.clone())
        .with_state(state.clone())
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    if status == StatusCode::OK {
        assert_eq!(response.headers()["cache-control"], "no-store");
    }
    let body = to_bytes(response.into_body(), 100_000).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn state() -> AppState {
    let backend = Arc::new(NativeBackend::new());
    let auth = Arc::new(AuthProvider::new(backend.clone()).await.unwrap());
    let mut state = AppState::new_shared(backend).with_jwt("session-test-secret".into(), 3600);
    state.auth = Some(auth);
    state
}

#[tokio::test]
async fn every_role_can_inspect_only_its_own_verified_identity_and_scopes() {
    let state = state().await;
    let auth = state.auth.as_ref().unwrap();
    assert_eq!(read(&state, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        read(&state, Some("invalid")).await.0,
        StatusCode::UNAUTHORIZED
    );
    for role in [
        Role::Admin,
        Role::Editor,
        Role::Viewer,
        Role::ScriptRunner,
        Role::PolicyAuthor,
        Role::PolicyApprover,
        Role::Promoter,
        Role::ArtifactAttestor,
        Role::HostAdmin,
    ] {
        let name = serde_json::to_value(role)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let user = auth
            .create_user(&name, "synthetic-session-test", role)
            .await
            .unwrap();
        let jwt = cognigraph_auth::jwt::issue(&user, "session-test-secret", 3600);
        let (status, body) = read(&state, Some(&jwt)).await;
        assert_eq!(status, StatusCode::OK, "{name}");
        assert_eq!(body["user"], json!(user));
        assert_eq!(body["scopes"], json!(role.scopes()));
        assert_eq!(body["auth_enabled"], true);
        assert_eq!(
            body["edition"],
            if cfg!(feature = "enterprise") {
                "enterprise"
            } else {
                "community"
            }
        );
        assert!(body.get("token").is_none());
        let token = auth
            .create_token(&user.key, "session-test", None)
            .await
            .unwrap();
        assert_eq!(read(&state, Some(&token.token)).await.1, body);
        auth.delete_user(&user.key).await.unwrap();
        assert_eq!(read(&state, Some(&jwt)).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(
            read(&state, Some(&token.token)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    #[cfg(feature = "enterprise")]
    state.jobs.shutdown().await;
}

#[tokio::test]
async fn identity_probe_obeys_suspension_without_blocking_host_control_identity() {
    let state = state().await;
    let auth = state.auth.as_ref().unwrap();
    auth.create_tenant("default", None).await.unwrap();
    let viewer = auth
        .create_user("viewer", "synthetic-session-test", Role::Viewer)
        .await
        .unwrap();
    let host = auth
        .create_user("host", "synthetic-session-test", Role::HostAdmin)
        .await
        .unwrap();
    let viewer_jwt = cognigraph_auth::jwt::issue(&viewer, "session-test-secret", 3600);
    let host_jwt = cognigraph_auth::jwt::issue(&host, "session-test-secret", 3600);
    auth.set_tenant_status("default", TenantStatus::Suspended)
        .await
        .unwrap();
    assert_eq!(
        read(&state, Some(&viewer_jwt)).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(read(&state, Some(&host_jwt)).await.0, StatusCode::OK);
    auth.set_tenant_status("default", TenantStatus::Active)
        .await
        .unwrap();
    assert_eq!(read(&state, Some(&viewer_jwt)).await.0, StatusCode::OK);
    #[cfg(feature = "enterprise")]
    state.jobs.shutdown().await;
}

#[tokio::test]
async fn disabled_auth_is_explicit_and_never_invents_an_admin_principal() {
    let state = AppState::new_shared(Arc::new(NativeBackend::new()));
    let (status, body) = read(&state, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["auth_enabled"], false);
    assert_eq!(body["user"], Value::Null);
    assert_eq!(body["scopes"], json!([]));
    #[cfg(feature = "enterprise")]
    state.jobs.shutdown().await;
}
