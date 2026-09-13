//! User and API-token management (admin scope).

use axum::extract::{Extension, Path, State};
use axum::routing::{delete, post};
use axum::{Json, Router};
use serde::Deserialize;

use cognigraph_auth::{Role, Scope, User};
use cognigraph_core::CogniGraphError;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_user).get(list_users))
        .route("/{key}", delete(delete_user))
        .route("/{key}/tokens", post(create_token).get(list_tokens))
        .route("/{key}/tokens/{token_key}", delete(revoke_token))
        .route("/{key}/tokens/{token_key}/rotate", post(rotate_token))
}

fn require_auth(state: &AppState) -> Result<&cognigraph_auth::AuthProvider, AppError> {
    state.auth.as_deref().ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "auth is disabled (set COGNIGRAPH_AUTH_ENABLED=true)".into(),
        ))
    })
}

/// Reload the middleware identity from the control store before a privileged
/// user-management operation. JWT claims are otherwise stateless until expiry;
/// this closes that window for deleted or changed administrators. Keep the
/// returned lifecycle guard through the control-store operation: pinned data
/// handles cannot isolate user/token records in the shared control store.
async fn require_live_admin(
    state: &AppState,
    user: Option<Extension<User>>,
) -> Result<(User, tokio::sync::MutexGuard<'_, ()>), AppError> {
    let lifecycle = state.tenant_lifecycle_lock.lock().await;
    let auth = require_auth(state)?;
    let Extension(claimed) = user.ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "user administration requires an authenticated Admin".into(),
        ))
    })?;
    let current = auth.get_user(&claimed.key).await?.ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "authenticated user is no longer active".into(),
        ))
    })?;
    if current.username != claimed.username
        || current.role != claimed.role
        || current.tenant != claimed.tenant
    {
        return Err(AppError(CogniGraphError::AuthError(
            "authenticated user identity is stale".into(),
        )));
    }
    if !current.role.grants(Scope::Admin) {
        return Err(AppError(CogniGraphError::Forbidden(
            "user administration requires an authenticated Admin".into(),
        )));
    }
    auth.tenant_allowed(&current.tenant).await?;
    Ok((current, lifecycle))
}

fn hidden_target(key: &str) -> AppError {
    AppError(CogniGraphError::DocumentNotFound {
        collection: "users".into(),
        key: key.into(),
    })
}

/// Resolve a tenant-manageable user without revealing whether a key belongs to
/// another tenant or to the host control plane.
async fn require_tenant_user(
    auth: &cognigraph_auth::AuthProvider,
    actor: &User,
    key: &str,
) -> Result<User, AppError> {
    auth.get_user(key)
        .await?
        .filter(|target| target.tenant == actor.tenant && target.role != Role::HostAdmin)
        .ok_or_else(|| hidden_target(key))
}

async fn require_owned_token(
    auth: &cognigraph_auth::AuthProvider,
    user_key: &str,
    token_key: &str,
) -> Result<(), AppError> {
    match auth.token_owner(token_key).await? {
        Some(owner) if owner == user_key => Ok(()),
        _ => Err(AppError(CogniGraphError::DocumentNotFound {
            collection: "tokens".into(),
            key: token_key.into(),
        })),
    }
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
    role: Role,
    /// The tenant this user belongs to (default: the implicit default
    /// tenant — single-tenant deployments never set this).
    tenant: Option<String>,
}

async fn create_user(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (actor, _lifecycle) = require_live_admin(&state, user).await?;
    crate::edition::require_tenant(req.tenant.as_deref().unwrap_or(&actor.tenant))?;
    if req.role == Role::HostAdmin {
        return Err(AppError(CogniGraphError::Forbidden(
            "host-admin users cannot be created through tenant user administration".into(),
        )));
    }
    if req
        .tenant
        .as_deref()
        .is_some_and(|tenant| tenant != actor.tenant)
    {
        return Err(AppError(CogniGraphError::Forbidden(
            "users can only be created in the authenticated Admin's tenant".into(),
        )));
    }
    let created = require_auth(&state)?
        .create_user_in(&req.username, &req.password, req.role, &actor.tenant)
        .await?;
    Ok(Json(
        serde_json::to_value(created).map_err(CogniGraphError::from)?,
    ))
}

async fn list_users(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (actor, _lifecycle) = require_live_admin(&state, user).await?;
    let users = require_auth(&state)?
        .list_users()
        .await?
        .into_iter()
        .filter(|target| target.tenant == actor.tenant && target.role != Role::HostAdmin)
        .collect::<Vec<_>>();
    Ok(Json(
        serde_json::to_value(users).map_err(CogniGraphError::from)?,
    ))
}

async fn delete_user(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (actor, _lifecycle) = require_live_admin(&state, user).await?;
    let auth = require_auth(&state)?;
    require_tenant_user(auth, &actor, &key).await?;
    let deleted = auth.delete_user(&key).await?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    #[serde(default)]
    name: String,
    /// TTL for this token; falls back to COGNIGRAPH_TOKEN_TTL_SECS.
    /// 0 = explicitly non-expiring, overriding the server default.
    expires_in_secs: Option<u64>,
}

async fn create_token(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(key): Path<String>,
    Json(req): Json<CreateTokenRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (actor, _lifecycle) = require_live_admin(&state, user).await?;
    let auth = require_auth(&state)?;
    require_tenant_user(auth, &actor, &key).await?;
    let ttl = req.expires_in_secs.unwrap_or(state.token_ttl_secs);
    let grant = auth.create_token(&key, &req.name, Some(ttl)).await?;
    // The plaintext token is shown exactly once.
    Ok(Json(
        serde_json::to_value(grant).map_err(CogniGraphError::from)?,
    ))
}

#[derive(Deserialize, Default)]
struct RotateTokenRequest {
    expires_in_secs: Option<u64>,
}

async fn rotate_token(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path((key, token_key)): Path<(String, String)>,
    req: Option<Json<RotateTokenRequest>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (actor, _lifecycle) = require_live_admin(&state, user).await?;
    let auth = require_auth(&state)?;
    require_tenant_user(auth, &actor, &key).await?;
    require_owned_token(auth, &key, &token_key).await?;
    let req = req.map(|Json(r)| r).unwrap_or_default();
    let ttl = req.expires_in_secs.unwrap_or(state.token_ttl_secs);
    let grant = auth.rotate_token(&token_key, Some(ttl)).await?;
    Ok(Json(
        serde_json::to_value(grant).map_err(CogniGraphError::from)?,
    ))
}

async fn list_tokens(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(key): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (actor, _lifecycle) = require_live_admin(&state, user).await?;
    let auth = require_auth(&state)?;
    require_tenant_user(auth, &actor, &key).await?;
    let tokens = auth.list_tokens(&key).await?;
    Ok(Json(
        serde_json::to_value(tokens).map_err(CogniGraphError::from)?,
    ))
}

async fn revoke_token(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path((key, token_key)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (actor, _lifecycle) = require_live_admin(&state, user).await?;
    let auth = require_auth(&state)?;
    require_tenant_user(auth, &actor, &key).await?;
    require_owned_token(auth, &key, &token_key).await?;
    let revoked = auth.revoke_token(&token_key).await?;
    Ok(Json(serde_json::json!({ "revoked": revoked })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use cognigraph_auth::AuthProvider;
    use cognigraph_native::NativeBackend;
    use std::sync::Arc;

    async fn state() -> (AppState, Arc<AuthProvider>) {
        let backend = Arc::new(NativeBackend::new());
        let auth = Arc::new(AuthProvider::new(backend.clone()).await.unwrap());
        auth.create_tenant("acme", None).await.unwrap();
        auth.create_tenant("bravo", None).await.unwrap();
        let mut state = AppState::new_shared(backend);
        state.auth = Some(auth.clone());
        (state, auth)
    }

    fn status(error: AppError) -> StatusCode {
        error.into_response().status()
    }

    #[tokio::test]
    async fn user_administration_is_tenant_local_and_cannot_create_host_admin() {
        let (state, auth) = state().await;
        let admin = auth
            .create_user_in(
                "acme-admin",
                "password",
                Role::Admin,
                cognigraph_auth::DEFAULT_TENANT,
            )
            .await
            .unwrap();
        let bravo_admin = auth
            .create_user_in("bravo-admin", "password", Role::Admin, "bravo")
            .await
            .unwrap();
        let host = auth
            .create_user_in(
                "host",
                "password",
                Role::HostAdmin,
                cognigraph_auth::DEFAULT_TENANT,
            )
            .await
            .unwrap();

        let Json(listed) = list_users(State(state.clone()), Some(Extension(admin.clone())))
            .await
            .unwrap();
        let listed = listed.as_array().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["username"], "acme-admin");

        let Json(created) = create_user(
            State(state.clone()),
            Some(Extension(admin.clone())),
            Json(CreateUserRequest {
                username: "acme-promoter".into(),
                password: "password".into(),
                role: Role::Promoter,
                tenant: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(created["tenant"], cognigraph_auth::DEFAULT_TENANT);
        assert_eq!(created["role"], "promoter");

        let cross_tenant = create_user(
            State(state.clone()),
            Some(Extension(admin.clone())),
            Json(CreateUserRequest {
                username: "intruder".into(),
                password: "password".into(),
                role: Role::Admin,
                tenant: Some("bravo".into()),
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(status(cross_tenant), StatusCode::FORBIDDEN);

        let host_admin = create_user(
            State(state.clone()),
            Some(Extension(admin.clone())),
            Json(CreateUserRequest {
                username: "second-host".into(),
                password: "password".into(),
                role: Role::HostAdmin,
                tenant: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(status(host_admin), StatusCode::FORBIDDEN);

        let hidden_host = delete_user(
            State(state.clone()),
            Some(Extension(admin.clone())),
            Path(host.key.clone()),
        )
        .await
        .unwrap_err();
        assert_eq!(status(hidden_host), StatusCode::NOT_FOUND);
        assert!(auth.get_user(&host.key).await.unwrap().is_some());

        let cross_delete = delete_user(
            State(state.clone()),
            Some(Extension(admin)),
            Path(bravo_admin.key.clone()),
        )
        .await
        .unwrap_err();
        assert_eq!(status(cross_delete), StatusCode::NOT_FOUND);
        assert!(auth.get_user(&bravo_admin.key).await.unwrap().is_some());
        #[cfg(feature = "enterprise")]
        state.jobs.shutdown().await;
    }

    #[tokio::test]
    async fn token_paths_require_both_a_local_user_and_matching_ownership() {
        let (state, auth) = state().await;
        let admin = auth
            .create_user_in("admin", "password", Role::Admin, "acme")
            .await
            .unwrap();
        let first = auth
            .create_user_in("first", "password", Role::PolicyAuthor, "acme")
            .await
            .unwrap();
        let second = auth
            .create_user_in("second", "password", Role::PolicyApprover, "acme")
            .await
            .unwrap();
        let remote = auth
            .create_user_in("remote", "password", Role::Promoter, "bravo")
            .await
            .unwrap();
        let first_token = auth.create_token(&first.key, "first", None).await.unwrap();
        let remote_token = auth
            .create_token(&remote.key, "remote", None)
            .await
            .unwrap();

        let wrong_owner = rotate_token(
            State(state.clone()),
            Some(Extension(admin.clone())),
            Path((second.key.clone(), first_token.key.clone())),
            None,
        )
        .await
        .unwrap_err();
        assert_eq!(status(wrong_owner), StatusCode::NOT_FOUND);
        assert!(auth.validate_token(&first_token.token).await.is_ok());

        let remote_list = list_tokens(
            State(state.clone()),
            Some(Extension(admin.clone())),
            Path(remote.key.clone()),
        )
        .await
        .unwrap_err();
        assert_eq!(status(remote_list), StatusCode::NOT_FOUND);

        let remote_revoke = revoke_token(
            State(state.clone()),
            Some(Extension(admin.clone())),
            Path((remote.key, remote_token.key)),
        )
        .await
        .unwrap_err();
        assert_eq!(status(remote_revoke), StatusCode::NOT_FOUND);
        assert!(auth.validate_token(&remote_token.token).await.is_ok());

        let Json(rotated) = rotate_token(
            State(state.clone()),
            Some(Extension(admin)),
            Path((first.key, first_token.key)),
            None,
        )
        .await
        .unwrap();
        assert!(rotated["token"].as_str().unwrap().starts_with("cg_"));
        assert!(auth.validate_token(&first_token.token).await.is_err());
        #[cfg(feature = "enterprise")]
        state.jobs.shutdown().await;
    }

    #[tokio::test]
    async fn deleted_jwt_identity_cannot_administer_users() {
        let (state, auth) = state().await;
        let stale = auth
            .create_user_in("admin", "password", Role::Admin, "acme")
            .await
            .unwrap();
        assert!(
            require_live_admin(&state, Some(Extension(stale.clone())))
                .await
                .is_ok()
        );
        auth.delete_user(&stale.key).await.unwrap();

        let error = require_live_admin(&state, Some(Extension(stale)))
            .await
            .unwrap_err();
        assert_eq!(status(error), StatusCode::UNAUTHORIZED);
        #[cfg(feature = "enterprise")]
        state.jobs.shutdown().await;
    }
}
