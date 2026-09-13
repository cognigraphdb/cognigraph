//! Bearer-token authentication + scope checks.
//!
//! When auth is disabled (no AuthProvider in state) every request passes,
//! preserving pre-Phase-8 behavior. When enabled, requests need a valid
//! `Authorization: Bearer cg_…` token whose user's role grants the scope
//! the route demands (read scope for GET/HEAD, write scope otherwise).

use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;

#[cfg(feature = "enterprise")]
use cognigraph_auth::DEFAULT_TENANT;
use cognigraph_auth::{Role, Scope, User};
use cognigraph_core::CogniGraphError;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Clone, Copy)]
pub struct ScopePolicy {
    pub read: Scope,
    pub write: Scope,
}

pub async fn check(
    State((state, policy)): State<(AppState, ScopePolicy)>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    admit(state, Some(policy), req, next).await
}

/// Verify the caller for session introspection without requiring a data or
/// control-plane scope. This middleware belongs only on the identity endpoint.
pub async fn identify(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    admit(state, None, req, next).await
}

async fn admit(
    state: AppState,
    policy: Option<ScopePolicy>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let Some(auth) = &state.auth else {
        return Ok(next.run(req).await);
    };

    let token = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            let (scheme, token) = value.split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then_some(token)
        })
        .ok_or_else(|| AppError(CogniGraphError::AuthError("missing bearer token".into())))?;

    // Identity validation, the active-tenant gate, and handle capture form one
    // admission operation relative to create/suspend/delete/recreate.
    let admission = state.tenant_lifecycle_lock.lock().await;
    let user: User = if token.starts_with("cg_") {
        auth.validate_token(token).await.map_err(AppError)?
    } else if let Some(jwt) = &state.jwt {
        let claimed = cognigraph_auth::jwt::verify(token, &jwt.0).map_err(AppError)?;
        let current = auth.get_user(&claimed.key).await?.ok_or_else(|| {
            AppError(CogniGraphError::AuthError(
                "JWT identity is no longer active".into(),
            ))
        })?;
        if current.username != claimed.username
            || current.role != claimed.role
            || current.tenant != claimed.tenant
        {
            return Err(AppError(CogniGraphError::AuthError(
                "JWT identity is stale".into(),
            )));
        }
        current
    } else {
        return Err(AppError(CogniGraphError::AuthError(
            "invalid bearer token".into(),
        )));
    };
    crate::edition::require_tenant(&user.tenant)?;
    if let Some(policy) = policy {
        let required = if matches!(*req.method(), Method::GET | Method::HEAD) {
            policy.read
        } else {
            policy.write
        };
        if !user.role.grants(required) {
            return Err(AppError(CogniGraphError::Forbidden(format!(
                "role `{:?}` lacks the required scope",
                user.role
            ))));
        }
    }
    // Tenant gate (decision_multi_tenancy.md, D2): resolved BEFORE any
    // route logic. The implicit default tenant passes unless a record
    // suspends it; any other tenant needs an ACTIVE record.
    if user.role != Role::HostAdmin {
        #[cfg(feature = "enterprise")]
        if user.tenant != DEFAULT_TENANT && state.tenant_registry.is_none() {
            return Err(AppError(CogniGraphError::Forbidden(
                "non-default tenant access requires isolated stores configured by COGNIGRAPH_DATA_DIR"
                    .into(),
            )));
        }
        auth.tenant_allowed(&user.tenant).await.map_err(AppError)?;
    }
    let tenant = user.tenant.clone();
    #[cfg(not(feature = "enterprise"))]
    let context = crate::tenancy::TenantContext::capture(tenant.clone());
    #[cfg(feature = "enterprise")]
    let context = if user.role == Role::HostAdmin {
        // Host control-plane routes may deliberately scope recovery to another
        // tenant. They must not open or pin a default data store on admission.
        crate::tenancy::TenantContext::capture(tenant.clone())
    } else {
        let incarnation = crate::jobs::JobManager::tenant_incarnation(&state, &tenant).await?;
        crate::tenancy::TenantContext::admitted(
            tenant.clone(),
            incarnation,
            state.tenant_registry.as_ref(),
        )?
    };
    drop(admission);
    req.extensions_mut().insert(user);
    // The whole handler future runs inside the tenant scope: the
    // RoutedBackend/RoutedCache facades resolve it from here (M2's
    // single structural choke point).
    let mut response = context.scope(next.run(req)).await;
    // Tag the response so the outer metrics layer can attribute an error
    // to this tenant without re-entering the (now-closed) tenant scope.
    response
        .extensions_mut()
        .insert(crate::hardening::TenantTag(tenant));
    Ok(response)
}

#[cfg(all(test, feature = "enterprise"))]
#[path = "tenant_request_tests.rs"]
mod tenant_request_tests;

#[cfg(all(test, feature = "enterprise"))]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use axum::middleware;
    use axum::routing::get;
    use cognigraph_auth::{AuthProvider, Role};
    use cognigraph_core::GraphBackend;
    use cognigraph_native::NativeBackend;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn jwt_identity_cannot_survive_tenant_recreation() {
        let backend = Arc::new(NativeBackend::new());
        let auth = Arc::new(AuthProvider::new(backend.clone()).await.unwrap());
        auth.create_tenant("acme", None).await.unwrap();
        let user = auth
            .create_user_in("acme-admin", "password", Role::Admin, "acme")
            .await
            .unwrap();
        let token = cognigraph_auth::jwt::issue(&user, "test-secret", 3600);
        let mut state = AppState::new_shared(backend);
        state.auth = Some(auth.clone());
        state.tenant_registry = Some(Arc::new(crate::tenancy::TenantRegistry::new(
            Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
            None,
        )));
        state = state.with_jwt("test-secret".into(), 3600);
        let policy = ScopePolicy {
            read: Scope::Admin,
            write: Scope::Admin,
        };
        let app = Router::new()
            .route("/", get(|| async { StatusCode::OK }))
            .route_layer(middleware::from_fn_with_state(
                (state.clone(), policy),
                check,
            ));
        let request = || {
            Request::get("/")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap()
        };
        assert_eq!(
            app.clone().oneshot(request()).await.unwrap().status(),
            StatusCode::OK
        );

        auth.delete_tenant("acme").await.unwrap();
        auth.create_tenant("acme", None).await.unwrap();
        assert_eq!(
            app.oneshot(request()).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
        state.jobs.shutdown().await;
    }

    #[tokio::test]
    async fn host_admin_survives_default_tenant_suspension_but_data_tenants_need_isolation() {
        let backend = Arc::new(NativeBackend::new());
        let auth = Arc::new(AuthProvider::new(backend.clone()).await.unwrap());
        auth.create_tenant(DEFAULT_TENANT, None).await.unwrap();
        let host = auth
            .create_user("host", "password", Role::HostAdmin)
            .await
            .unwrap();
        auth.set_tenant_status(DEFAULT_TENANT, cognigraph_auth::TenantStatus::Suspended)
            .await
            .unwrap();
        let host_token = cognigraph_auth::jwt::issue(&host, "test-secret", 3600);
        let mut state = AppState::new_shared(backend.clone());
        state.auth = Some(auth.clone());
        state = state.with_jwt("test-secret".into(), 3600);
        let host_app = Router::new()
            .route("/", get(|| async { StatusCode::OK }))
            .route_layer(middleware::from_fn_with_state(
                (
                    state.clone(),
                    ScopePolicy {
                        read: Scope::TenantAdmin,
                        write: Scope::TenantAdmin,
                    },
                ),
                check,
            ));
        assert_eq!(
            host_app
                .oneshot(
                    Request::get("/")
                        .header(header::AUTHORIZATION, format!("Bearer {host_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );

        auth.create_tenant("acme", None).await.unwrap();
        let acme = auth
            .create_user_in("acme-admin", "password", Role::Admin, "acme")
            .await
            .unwrap();
        let acme_token = cognigraph_auth::jwt::issue(&acme, "test-secret", 3600);
        let data_app = Router::new()
            .route("/", get(|| async { StatusCode::OK }))
            .route_layer(middleware::from_fn_with_state(
                (
                    state.clone(),
                    ScopePolicy {
                        read: Scope::Admin,
                        write: Scope::Admin,
                    },
                ),
                check,
            ));
        assert_eq!(
            data_app
                .oneshot(
                    Request::get("/")
                        .header(header::AUTHORIZATION, format!("Bearer {acme_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        state.jobs.shutdown().await;
    }
}
