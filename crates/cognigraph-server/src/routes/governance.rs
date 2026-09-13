//! Tenant-scoped M19 signed-governance API.

use axum::extract::{DefaultBodyLimit, Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cognigraph_auth::{Role, User};
use cognigraph_core::CogniGraphError;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::artifact_attestations::{
    CreateArtifactAttestationRequest, MAX_ARTIFACT_ATTESTATION_REQUEST_BYTES,
    ResolveArtifactBindingsRequest,
};
use crate::error::AppError;
use crate::governance::{
    ApprovePolicyRevisionRequest, CreatePolicyRevisionRequest, GovernanceActor,
    RegisterGovernanceKeyRequest, RevokeGovernanceKeyRequest,
};
use crate::jobs::JobManager;
use crate::state::AppState;
use crate::tenancy::current_tenant;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(status))
        .route("/keys", get(list_keys).post(register_key))
        .route("/keys/{id}", get(get_key))
        .route("/keys/{id}/revoke", post(revoke_key))
        .route("/revocations", get(list_revocations))
        .route("/revocations/{id}", get(get_revocation))
        .route("/policies", get(list_policies).post(create_policy))
        .route("/policies/{id}", get(get_policy))
        .route("/policies/{id}/approve", post(approve_policy))
        .route("/approvals/{id}", get(get_approval))
        .route("/bindings/{approval_id}", get(get_binding))
        .route(
            "/artifact-attestations",
            get(list_artifact_attestations)
                .post(create_artifact_attestation)
                // A canonical manifest may reach 16 MiB. Reserve another MiB
                // for the signed envelope and reject larger transport bodies
                // before JSON deserialization. This limit is local to the
                // artifact route; other governance requests keep Axum's
                // default bound.
                .layer(DefaultBodyLimit::max(
                    MAX_ARTIFACT_ATTESTATION_REQUEST_BYTES,
                )),
        )
        .route("/artifact-attestations/{id}", get(get_artifact_attestation))
        .route(
            "/artifact-bindings/resolve",
            post(resolve_artifact_bindings),
        )
}

pub(crate) async fn require_live_role(
    state: &AppState,
    user: Option<Extension<User>>,
    expected: Role,
) -> Result<User, AppError> {
    let auth = state.auth.as_deref().ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "signed governance requires authentication to be enabled".into(),
        ))
    })?;
    let Extension(claimed) = user.ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "signed governance requires an authenticated user".into(),
        ))
    })?;
    let current = auth.get_user(&claimed.key).await?.ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "authenticated governance user is no longer active".into(),
        ))
    })?;
    if current.username != claimed.username
        || current.role != claimed.role
        || current.tenant != claimed.tenant
    {
        return Err(AppError(CogniGraphError::AuthError(
            "authenticated governance identity is stale".into(),
        )));
    }
    if current.role != expected {
        return Err(AppError(CogniGraphError::Forbidden(format!(
            "operation requires the `{}` role",
            role_name(expected)
        ))));
    }
    if current.tenant != current_tenant() {
        return Err(AppError(CogniGraphError::Forbidden(
            "authenticated governance tenant does not match request routing".into(),
        )));
    }
    auth.tenant_allowed(&current.tenant).await?;
    Ok(current)
}

fn idempotency_key(headers: &HeaderMap) -> Result<&str, AppError> {
    headers
        .get("Idempotency-Key")
        .ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(
                "missing Idempotency-Key header".into(),
            ))
        })?
        .to_str()
        .map_err(|_| {
            AppError(CogniGraphError::ValidationError(
                "Idempotency-Key must be printable ASCII".into(),
            ))
        })
}

async fn scope(state: &AppState) -> Result<(String, String), AppError> {
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(state, &tenant).await?;
    Ok((tenant, incarnation))
}

async fn status(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
) -> Result<Json<Value>, AppError> {
    let user = require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let root_key_id = state.promotions.governance_root_key_id();
    Ok(Json(json!({
        "schema_version": 1,
        "tenant": tenant,
        "tenant_incarnation": incarnation,
        "root_configured": root_key_id.is_some(),
        "root_key_id": root_key_id,
        "actor": { "user_key": user.key, "role": user.role },
        "private_keys_accepted": false,
    })))
}

pub(crate) async fn require_any_reader(
    state: &AppState,
    user: Option<Extension<User>>,
) -> Result<User, AppError> {
    let claimed = user
        .as_ref()
        .map(|Extension(user)| user.role)
        .ok_or_else(|| {
            AppError(CogniGraphError::AuthError(
                "governance reads require authentication".into(),
            ))
        })?;
    if !matches!(
        claimed,
        Role::Admin
            | Role::PolicyAuthor
            | Role::PolicyApprover
            | Role::Promoter
            | Role::ArtifactAttestor
    ) {
        return Err(AppError(CogniGraphError::Forbidden(
            "role cannot read signed governance authority".into(),
        )));
    }
    require_live_role(state, user, claimed).await
}

async fn create_artifact_attestation(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(request): Json<CreateArtifactAttestationRequest>,
) -> Result<Response, AppError> {
    let actor = require_live_role(&state, user, Role::ArtifactAttestor).await?;
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .create_artifact_attestation(
            &tenant,
            &incarnation,
            GovernanceActor::from_user(actor),
            key,
            request,
        )
        .await?;
    let id = mutation.record.attestation_id.clone();
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let mut response = (
        status,
        Json(json!({
            "artifact_attestation": mutation.record.public_value()?,
            "replayed": mutation.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/governance/artifact-attestations/{id}")
            .parse()
            .expect("artifact attestation location is valid"),
    );
    Ok(response)
}

async fn list_artifact_attestations(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let page = state
        .promotions
        .list_artifact_attestations(
            &tenant,
            &incarnation,
            query.limit.unwrap_or(50),
            query.cursor.as_deref(),
        )
        .await?;
    let records = page.records;
    Ok(Json(json!({
        "artifact_attestations": records,
        "count": records.len(),
        "next_cursor": page.next_cursor,
    })))
}

async fn get_artifact_attestation(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_artifact_attestation(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn resolve_artifact_bindings(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(request): Json<ResolveArtifactBindingsRequest>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        serde_json::to_value(
            state
                .promotions
                .resolve_artifact_bindings(&tenant, &incarnation, &request)
                .await?,
        )
        .map_err(CogniGraphError::from)?,
    ))
}

async fn register_key(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(request): Json<RegisterGovernanceKeyRequest>,
) -> Result<Response, AppError> {
    let actor = require_live_role(&state, user, Role::Admin).await?;
    let key = idempotency_key(&headers)?;
    let auth = state.auth.as_deref().ok_or_else(|| {
        AppError(CogniGraphError::AuthError(
            "signed governance requires authentication".into(),
        ))
    })?;
    let subject_key = request.statement.payload.subject_user_key.clone();
    let subject = auth
        .get_user(&subject_key)
        .await?
        .filter(|subject| subject.tenant == actor.tenant)
        .ok_or_else(|| {
            AppError(CogniGraphError::DocumentNotFound {
                collection: "users".into(),
                key: subject_key,
            })
        })?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .register_governance_key(
            &tenant,
            &incarnation,
            GovernanceActor::from_user(actor),
            subject,
            key,
            request,
        )
        .await?;
    let id = mutation.record.registration_id.clone();
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let mut response = (
        status,
        Json(json!({
            "key": mutation.record.public_value()?,
            "replayed": mutation.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/governance/keys/{id}")
            .parse()
            .expect("governance key location is valid"),
    );
    Ok(response)
}

async fn revoke_key(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<RevokeGovernanceKeyRequest>,
) -> Result<Json<Value>, AppError> {
    let actor = require_live_role(&state, user, Role::Admin).await?;
    if request.statement.payload.registration_id != id {
        return Err(AppError(CogniGraphError::ValidationError(
            "key revocation path and signed registration id differ".into(),
        )));
    }
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .revoke_governance_key(
            &tenant,
            &incarnation,
            GovernanceActor::from_user(actor),
            key,
            request,
        )
        .await?;
    Ok(Json(json!({
        "revocation": mutation.record.public_value()?,
        "replayed": mutation.replayed,
    })))
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<usize>,
    cursor: Option<String>,
}

async fn list_keys(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let page = state
        .promotions
        .list_governance_keys(
            &tenant,
            &incarnation,
            query.limit.unwrap_or(50),
            query.cursor.as_deref(),
        )
        .await?;
    let keys = page
        .records
        .iter()
        .map(crate::governance::GovernanceKeyRecord::public_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({
        "keys": keys,
        "count": keys.len(),
        "next_cursor": page.next_cursor,
    })))
}

async fn get_key(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_governance_key(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn list_revocations(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let page = state
        .promotions
        .list_governance_revocations(
            &tenant,
            &incarnation,
            query.limit.unwrap_or(50),
            query.cursor.as_deref(),
        )
        .await?;
    let revocations = page
        .records
        .iter()
        .map(crate::governance::GovernanceKeyRevocation::public_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({
        "revocations": revocations,
        "count": revocations.len(),
        "next_cursor": page.next_cursor,
    })))
}

async fn get_revocation(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_governance_revocation(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn create_policy(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(request): Json<CreatePolicyRevisionRequest>,
) -> Result<Response, AppError> {
    let actor = require_live_role(&state, user, Role::PolicyAuthor).await?;
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .create_policy_revision(
            &tenant,
            &incarnation,
            GovernanceActor::from_user(actor),
            key,
            request,
        )
        .await?;
    let id = mutation.record.policy_revision_id.clone();
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let mut response = (
        status,
        Json(json!({
            "policy": mutation.record.public_value()?,
            "replayed": mutation.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/governance/policies/{id}")
            .parse()
            .expect("policy location is valid"),
    );
    Ok(response)
}

async fn approve_policy(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<ApprovePolicyRevisionRequest>,
) -> Result<Response, AppError> {
    let actor = require_live_role(&state, user, Role::PolicyApprover).await?;
    if request.statement.payload.policy_revision_id != id {
        return Err(AppError(CogniGraphError::ValidationError(
            "policy approval path and signed revision id differ".into(),
        )));
    }
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .approve_policy_revision(
            &tenant,
            &incarnation,
            GovernanceActor::from_user(actor),
            key,
            request,
        )
        .await?;
    let approval_id = mutation.record.approval_id.clone();
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let mut response = (
        status,
        Json(json!({
            "approval": mutation.record.public_value()?,
            "replayed": mutation.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/governance/approvals/{approval_id}")
            .parse()
            .expect("approval location is valid"),
    );
    Ok(response)
}

async fn list_policies(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let page = state
        .promotions
        .list_policy_revisions(
            &tenant,
            &incarnation,
            query.limit.unwrap_or(50),
            query.cursor.as_deref(),
        )
        .await?;
    let policies = page
        .records
        .iter()
        .map(crate::governance::PolicyRevisionRecord::public_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({
        "policies": policies,
        "count": policies.len(),
        "next_cursor": page.next_cursor,
    })))
}

async fn get_policy(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_policy_revision(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn get_approval(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_policy_approval(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn get_binding(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(approval_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let binding = state
        .promotions
        .policy_binding(&tenant, &incarnation, &approval_id)
        .await?;
    Ok(Json(
        serde_json::to_value(binding).map_err(CogniGraphError::from)?,
    ))
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Admin => "admin",
        Role::Editor => "editor",
        Role::Viewer => "viewer",
        Role::ScriptRunner => "script-runner",
        Role::PolicyAuthor => "policy-author",
        Role::PolicyApprover => "policy-approver",
        Role::Promoter => "promoter",
        Role::ArtifactAttestor => "artifact-attestor",
        Role::HostAdmin => "host-admin",
    }
}
