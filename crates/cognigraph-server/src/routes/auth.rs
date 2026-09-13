//! Session login: exchanges credentials for a short-lived JWT.
//! Unauthenticated by necessity; requires COGNIGRAPH_AUTH_ENABLED and COGNIGRAPH_JWT_SECRET.

use axum::extract::{Extension, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

#[cfg(any(test, feature = "enterprise"))]
use cognigraph_auth::DEFAULT_TENANT;
use cognigraph_auth::{Role, Scope, User};
use cognigraph_core::CogniGraphError;

use crate::error::AppError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new().route("/login", post(login)).route(
        "/session",
        get(session).route_layer(axum::middleware::from_fn_with_state(
            state,
            crate::auth_middleware::identify,
        )),
    )
}

async fn session(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    let user = user.map(|Extension(user)| user);
    if state.auth.is_some() && user.is_none() {
        return Err(CogniGraphError::AuthError("missing verified identity".into()).into());
    }
    // No invented Admin identity in development mode. An empty scope list
    // describes the absence of an authenticated principal, not enabled guards.
    let scopes: &[Scope] = user.as_ref().map_or(&[], |user| user.role.scopes());
    Ok((
        [(axum::http::header::CACHE_CONTROL, "no-store")],
        Json(serde_json::json!({
            "auth_enabled": state.auth.is_some(),
            "user": user,
            "scopes": scopes,
            "edition": if cfg!(feature = "enterprise") { "enterprise" } else { "community" },
        })),
    ))
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth = state.auth.as_deref().ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "auth is disabled (set COGNIGRAPH_AUTH_ENABLED=true)".into(),
        ))
    })?;
    let jwt = state.jwt.as_ref().ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "sessions are disabled (set COGNIGRAPH_JWT_SECRET)".into(),
        ))
    })?;
    let (secret, ttl) = (&jwt.0, jwt.1);
    let user = auth
        .verify_password(&req.username, &req.password)
        .await
        .map_err(AppError)?;
    // A suspended (or deleted) tenant's users must not receive sessions:
    // the API middleware would refuse every call anyway, but minting a
    // token here would report a misleading "successful" sign-in.
    crate::edition::require_tenant(&user.tenant)?;
    if user.role != Role::HostAdmin {
        #[cfg(feature = "enterprise")]
        if user.tenant != DEFAULT_TENANT && state.tenant_registry.is_none() {
            return Err(AppError(CogniGraphError::Forbidden(
                "non-default tenant sessions require isolated stores configured by COGNIGRAPH_DATA_DIR"
                    .into(),
            )));
        }
        auth.tenant_allowed(&user.tenant).await.map_err(AppError)?;
    }
    let token = cognigraph_auth::jwt::issue(&user, secret, ttl);
    Ok(Json(serde_json::json!({
        "token": token,
        "token_type": "Bearer",
        "expires_in": ttl,
        "role": user.role,
        // The tenant this session is scoped to — identity-scoped tenancy
        // (D2): every data call this token makes is routed to it.
        "tenant": user.tenant,
    })))
}

#[cfg(test)]
#[path = "auth_session_tests.rs"]
mod session_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_auth::{AuthProvider, Role, TenantStatus};
    #[cfg(feature = "enterprise")]
    use cognigraph_core::GraphBackend;
    use cognigraph_native::NativeBackend;
    use std::sync::Arc;

    #[tokio::test]
    #[cfg(feature = "enterprise")]
    async fn login_refuses_a_suspended_tenants_users() {
        let backend = Arc::new(NativeBackend::new());
        let auth = AuthProvider::new(backend.clone()).await.unwrap();
        auth.create_tenant("acme", None).await.unwrap();
        auth.create_user_in("bob", "hunter2hunter2", Role::Viewer, "acme")
            .await
            .unwrap();
        let mut state = AppState::new_shared(backend);
        state.auth = Some(Arc::new(auth));
        state.tenant_registry = Some(Arc::new(crate::tenancy::TenantRegistry::new(
            Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            None,
        )));
        let state = state.with_jwt("test-secret".into(), 3600);

        let attempt = |state: AppState| async move {
            login(
                State(state),
                Json(LoginRequest {
                    username: "bob".into(),
                    password: "hunter2hunter2".into(),
                }),
            )
            .await
        };

        // Active tenant: sessions are issued.
        assert!(attempt(state.clone()).await.is_ok());

        // Suspended tenant: no session, even with valid credentials.
        state
            .auth
            .as_ref()
            .unwrap()
            .set_tenant_status("acme", TenantStatus::Suspended)
            .await
            .unwrap();
        let refused = attempt(state.clone()).await;
        assert!(refused.is_err(), "suspended tenant must not get sessions");

        // Resuming restores sign-in.
        state
            .auth
            .as_ref()
            .unwrap()
            .set_tenant_status("acme", TenantStatus::Active)
            .await
            .unwrap();
        assert!(attempt(state).await.is_ok());
    }

    #[tokio::test]
    async fn host_admin_login_is_independent_of_default_tenant_data_status() {
        let backend = Arc::new(NativeBackend::new());
        let auth = AuthProvider::new(backend.clone()).await.unwrap();
        auth.create_tenant(DEFAULT_TENANT, None).await.unwrap();
        auth.create_user("host", "host-secret", Role::HostAdmin)
            .await
            .unwrap();
        auth.set_tenant_status(DEFAULT_TENANT, TenantStatus::Suspended)
            .await
            .unwrap();
        let mut state = AppState::new_shared(backend);
        state.auth = Some(Arc::new(auth));
        let state = state.with_jwt("test-secret".into(), 3600);
        assert!(
            login(
                State(state.clone()),
                Json(LoginRequest {
                    username: "host".into(),
                    password: "host-secret".into(),
                }),
            )
            .await
            .is_ok()
        );
        #[cfg(feature = "enterprise")]
        state.jobs.shutdown().await;
    }
}
