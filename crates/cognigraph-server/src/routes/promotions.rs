//! Tenant-scoped M18 evaluation evidence and promotion decisions.

use axum::extract::{Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cognigraph_auth::{Role, User};
use cognigraph_core::CogniGraphError;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::AppError;
use crate::governance::PromotionIntentSubmission;
use crate::jobs::JobManager;
use crate::promotions::{
    PromotionActor, PromotionTarget, ReconcileRequest, RegisterEvidenceRequest,
};
use crate::state::AppState;
use crate::tenancy::current_tenant;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/evidence", post(register_evidence).get(list_evidence))
        .route("/evidence/{id}", get(get_evidence))
        .route("/evidence/{id}/promote", post(promote))
        .route("/evidence/{id}/reject", post(reject))
        .route("/decisions", get(list_decisions))
        .route("/decisions/{id}", get(get_decision))
        .route("/current/{space_type}/{channel}", get(current))
        .route("/current/{space_type}/{channel}/rollback", post(rollback))
}

async fn require_role(
    state: &AppState,
    user: Option<Extension<User>>,
    role: Role,
) -> Result<User, AppError> {
    crate::routes::governance::require_live_role(state, user, role).await
}

fn promotion_actor(user: User) -> PromotionActor {
    PromotionActor {
        user_key: user.key,
        username: user.username,
        role: match user.role {
            Role::Admin => "admin",
            Role::Promoter => "promoter",
            _ => unreachable!("promotion actor roles are checked before conversion"),
        }
        .into(),
    }
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

async fn register_evidence(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(request): Json<RegisterEvidenceRequest>,
) -> Result<Response, AppError> {
    let actor = promotion_actor(require_role(&state, user, Role::Promoter).await?);
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .register_evidence(&tenant, &incarnation, actor, key, request)
        .await?;
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let id = mutation.record.id.clone();
    let mut response = (
        status,
        Json(json!({
            "evidence": mutation.record.public_value()?,
            "replayed": mutation.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/promotions/evidence/{id}")
            .parse()
            .expect("promotion evidence location is valid"),
    );
    Ok(response)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<usize>,
    cursor: Option<String>,
}

async fn list_evidence(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let page = state
        .promotions
        .list_evidence(
            &tenant,
            &incarnation,
            query.limit.unwrap_or(50),
            query.cursor.as_deref(),
        )
        .await?;
    let records = page
        .records
        .iter()
        .map(crate::promotions::PromotionEvidence::public_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({
        "evidence": records,
        "count": records.len(),
        "next_cursor": page.next_cursor,
    })))
}

async fn get_evidence(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_evidence(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn promote(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(authorization): Json<PromotionIntentSubmission>,
) -> Result<Json<Value>, AppError> {
    let actor = promotion_actor(require_role(&state, user, Role::Promoter).await?);
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .promote_signed(&tenant, &incarnation, &id, actor, key, authorization)
        .await?;
    Ok(Json(json!({
        "decision": mutation.record.public_value()?,
        "replayed": mutation.replayed,
        "head_changed": mutation.head_changed,
    })))
}

async fn reject(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(authorization): Json<PromotionIntentSubmission>,
) -> Result<Json<Value>, AppError> {
    let actor = promotion_actor(require_role(&state, user, Role::Promoter).await?);
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .reject_signed(&tenant, &incarnation, &id, actor, key, authorization)
        .await?;
    Ok(Json(json!({
        "decision": mutation.record.public_value()?,
        "replayed": mutation.replayed,
        "head_changed": mutation.head_changed,
    })))
}

async fn list_decisions(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let page = state
        .promotions
        .list_decisions(
            &tenant,
            &incarnation,
            query.limit.unwrap_or(50),
            query.cursor.as_deref(),
        )
        .await?;
    let records = page
        .records
        .iter()
        .map(crate::promotions::PromotionDecision::public_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({
        "decisions": records,
        "count": records.len(),
        "next_cursor": page.next_cursor,
    })))
}

async fn get_decision(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_decision(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn current(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path((space_type, channel)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let head = state
        .promotions
        .current(
            &tenant,
            &incarnation,
            &PromotionTarget {
                space_type,
                channel,
            },
        )
        .await?;
    let head = head
        .as_ref()
        .map(crate::promotions::PromotionHead::public_value)
        .transpose()?;
    Ok(Json(json!({ "head": head })))
}

async fn rollback(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Path((space_type, channel)): Path<(String, String)>,
    Json(authorization): Json<PromotionIntentSubmission>,
) -> Result<Json<Value>, AppError> {
    let actor = promotion_actor(require_role(&state, user, Role::Promoter).await?);
    let key = idempotency_key(&headers)?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .rollback_signed(
            &tenant,
            &incarnation,
            &PromotionTarget {
                space_type,
                channel,
            },
            actor,
            key,
            authorization,
        )
        .await?;
    Ok(Json(json!({
        "decision": mutation.record.public_value()?,
        "replayed": mutation.replayed,
        "head_changed": mutation.head_changed,
    })))
}

pub(crate) async fn operator_status(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
) -> Result<Json<Value>, AppError> {
    require_role(&state, user, Role::Admin).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .operator_status(&tenant, &incarnation)
            .await?,
    ))
}

pub(crate) async fn reconcile(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(request): Json<ReconcileRequest>,
) -> Result<Json<Value>, AppError> {
    require_role(&state, user, Role::Admin).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let result = state
        .promotions
        .reconcile(&tenant, &incarnation, &request.target, request.dry_run)
        .await?;
    Ok(Json(result.public_value()?))
}

pub(crate) async fn recover(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
) -> Result<Json<Value>, AppError> {
    require_role(&state, user, Role::Admin).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let recovery = state.promotions.recover_tenant(&tenant, &incarnation).await;
    // Recovery can commit an earlier repair before a later validation or
    // backend operation fails. Invalidate after every attempt so a partial
    // failure cannot leave cached reads ahead of durable graph state.
    state.invalidate_search_results().await;
    let repaired = recovery?;
    Ok(Json(json!({
        "tenant": tenant,
        "tenant_incarnation": incarnation,
        "repaired_heads": repaired,
        "healthy": true,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use cognigraph_auth::AuthProvider;
    use cognigraph_native::NativeBackend;
    use std::sync::Arc;

    async fn state_with_roles() -> (AppState, User, User) {
        let backend = Arc::new(NativeBackend::new());
        let auth = Arc::new(AuthProvider::new(backend.clone()).await.unwrap());
        let promoter = auth
            .create_user("promoter", "password", Role::Promoter)
            .await
            .unwrap();
        let editor = auth
            .create_user("editor", "password", Role::Editor)
            .await
            .unwrap();
        let mut state = AppState::new_shared(backend);
        state.auth = Some(auth);
        (state, promoter, editor)
    }

    #[tokio::test]
    async fn promotion_mutations_require_a_live_promoter_even_without_middleware() {
        let (state, _promoter, editor) = state_with_roles().await;
        let denied = require_role(&state, None, Role::Promoter)
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            require_role(&state, Some(Extension(editor)), Role::Promoter)
                .await
                .unwrap_err()
                .into_response()
                .status(),
            StatusCode::FORBIDDEN
        );
        state.jobs.shutdown().await;
    }

    #[tokio::test]
    async fn current_head_is_null_for_a_new_target() {
        let (state, promoter, _editor) = state_with_roles().await;
        let Json(body) = crate::tenancy::CURRENT_TENANT
            .scope(
                "default".into(),
                current(
                    State(state.clone()),
                    Some(Extension(promoter)),
                    Path(("pharma".into(), "production".into())),
                ),
            )
            .await
            .unwrap();
        assert!(body["head"].is_null());
        state.jobs.shutdown().await;
    }
}
