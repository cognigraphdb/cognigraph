//! M26 verified Semantic Repair generation and deployment HTTP surface.

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
use crate::governance::GovernanceActor;
use crate::jobs::JobManager;
use crate::materialized_repairs::{
    BuildSemanticRepairGenerationRequest, MAX_M26_LIST_PAGE_SIZE,
    SemanticRepairDeploymentIntentSubmission,
};
use crate::state::AppState;
use crate::tenancy::current_tenant;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/generations", get(list_generations).post(build_generation))
        .route("/generations/{id}", get(get_generation))
        .route("/generations/{id}/deploy", post(deploy_generation))
        .route("/deployments/current/{space_type}", get(current_deployment))
}

async fn scope(state: &AppState) -> Result<(String, String), AppError> {
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(state, &tenant).await?;
    Ok((tenant, incarnation))
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

async fn require_promoter(
    state: &AppState,
    user: Option<Extension<User>>,
) -> Result<GovernanceActor, AppError> {
    crate::routes::governance::require_live_role(state, user, Role::Promoter)
        .await
        .map(GovernanceActor::from_user)
}

async fn build_generation(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(request): Json<BuildSemanticRepairGenerationRequest>,
) -> Result<Response, AppError> {
    let actor = require_promoter(&state, user).await?;
    let idempotency_key = idempotency_key(&headers)?;
    let cas = state.artifact_cas.as_deref().ok_or_else(|| {
        AppError(CogniGraphError::ConnectionError(
            "M26 generation build requires COGNIGRAPH_ARTIFACT_SOURCE=local-cas".into(),
        ))
    })?;
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .build_semantic_repair_generation(
            cas,
            &tenant,
            &incarnation,
            actor,
            idempotency_key,
            request,
        )
        .await?;
    let id = mutation.record.semantic_repair_generation_id.clone();
    let status = if mutation.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    let mut response = (
        status,
        Json(json!({
            "semantic_repair_generation": mutation.record.public_value()?,
            "replayed": mutation.replayed,
            "deployed": false,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/semantic-repairs/generations/{id}")
            .parse()
            .expect("M26 generation location is valid"),
    );
    Ok(response)
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<usize>,
    cursor: Option<String>,
}

async fn list_generations(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Query(query): Query<PageQuery>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let (records, next_cursor) = state
        .promotions
        .list_semantic_repair_generations(
            &tenant,
            &incarnation,
            query.limit.unwrap_or(MAX_M26_LIST_PAGE_SIZE),
            query.cursor.as_deref(),
        )
        .await?;
    let summaries = records
        .iter()
        .map(|record| record.summary_value())
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "semantic_repair_generations": summaries,
        "count": summaries.len(),
        "next_cursor": next_cursor,
    })))
}

async fn get_generation(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    Ok(Json(
        state
            .promotions
            .get_semantic_repair_generation(&tenant, &incarnation, &id)
            .await?
            .public_value()?,
    ))
}

async fn deploy_generation(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(authorization): Json<SemanticRepairDeploymentIntentSubmission>,
) -> Result<Json<Value>, AppError> {
    let actor = require_promoter(&state, user).await?;
    let idempotency_key = idempotency_key(&headers)?;
    if authorization
        .statement
        .payload
        .semantic_repair_generation_id
        != id
    {
        return Err(AppError(CogniGraphError::ValidationError(
            "M26 generation path id does not match the signed deployment intent".into(),
        )));
    }
    let (tenant, incarnation) = scope(&state).await?;
    let mutation = state
        .promotions
        .deploy_semantic_repair_generation(
            &tenant,
            &incarnation,
            actor,
            idempotency_key,
            &id,
            authorization,
        )
        .await?;
    state.invalidate_search_results().await;
    Ok(Json(json!({
        "deployment_decision": mutation.decision.public_value()?,
        "deployment_head": mutation.head.public_value()?,
        "replayed": mutation.replayed,
    })))
}

async fn current_deployment(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Path(space_type): Path<String>,
) -> Result<Json<Value>, AppError> {
    crate::routes::governance::require_any_reader(&state, user).await?;
    let (tenant, incarnation) = scope(&state).await?;
    let head = state
        .promotions
        .current_semantic_repair_deployment(&tenant, &incarnation, &space_type)
        .await?;
    let deployed = head.as_ref().map(|head| head.public_value()).transpose()?;
    Ok(Json(json!({
        "space_type": space_type,
        "deployment_head": deployed,
    })))
}
