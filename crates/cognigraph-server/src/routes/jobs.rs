//! Tenant-scoped lifecycle API for durable governed operations (M16-M17).

use axum::extract::{DefaultBodyLimit, Extension, Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cognigraph_auth::User;
use cognigraph_core::CogniGraphError;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::AppError;
use crate::jobs::{
    ArchiveFilter, JobActor, JobKind, JobManager, JobStatus, MAX_JOB_INPUT_BYTES,
    MAX_JOB_LIST_OFFSET, RetryMode,
};
use crate::state::AppState;
use crate::tenancy::current_tenant;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(submit).get(list))
        .route("/{id}", get(status))
        .route("/{id}/cancel", post(cancel))
        .route("/{id}/retry", post(retry))
        .layer(DefaultBodyLimit::max(MAX_JOB_INPUT_BYTES + 64 * 1024))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitRequest {
    kind: JobKind,
    input: Value,
}

async fn submit(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(request): Json<SubmitRequest>,
) -> Result<Response, AppError> {
    let key = idempotency_key(&headers)?;
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    let submission = state
        .jobs
        .submit(
            state.clone(),
            tenant,
            incarnation,
            JobActor::request(user.as_ref().map(|Extension(user)| user)),
            key,
            request.kind,
            request.input,
        )
        .await?;
    let status = if submission.replayed {
        StatusCode::OK
    } else {
        StatusCode::ACCEPTED
    };
    let mut response = (
        status,
        Json(json!({
            "job": submission.job.public_value(),
            "replayed": submission.replayed,
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::LOCATION,
        format!("/api/jobs/{}", submission.job.id)
            .parse()
            .expect("job location is valid"),
    );
    Ok(response)
}

#[derive(Deserialize)]
struct ListQuery {
    kind: Option<JobKind>,
    status: Option<JobStatus>,
    limit: Option<usize>,
    cursor: Option<String>,
    #[serde(default)]
    archived: ArchiveFilter,
    offset: Option<usize>,
}

async fn list(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Value>, AppError> {
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err(AppError(CogniGraphError::ValidationError(
            "job list limit must be between 1 and 200".into(),
        )));
    }
    if query.cursor.is_some() && query.offset.is_some() {
        return Err(AppError(CogniGraphError::ValidationError(
            "job list cursor and offset are mutually exclusive".into(),
        )));
    }
    let offset = query.offset.unwrap_or(0);
    if offset > MAX_JOB_LIST_OFFSET {
        return Err(AppError(CogniGraphError::ValidationError(format!(
            "job list offset must be between 0 and {MAX_JOB_LIST_OFFSET}"
        ))));
    }
    if query.offset.is_some() {
        if query.archived != ArchiveFilter::Exclude {
            return Err(AppError(CogniGraphError::ValidationError(
                "deprecated offset pagination cannot include archived jobs; use cursor pagination"
                    .into(),
            )));
        }
        let (jobs, total) = state
            .jobs
            .list(
                &tenant,
                &incarnation,
                query.kind,
                query.status,
                limit,
                offset,
            )
            .await?;
        let jobs = jobs
            .into_iter()
            .map(|job| job.public_value())
            .collect::<Vec<_>>();
        return Ok(Json(json!({
            "pagination": "offset",
            "count": jobs.len(),
            "total": total,
            "limit": limit,
            "offset": offset,
            "jobs": jobs,
        })));
    }
    let page = state
        .jobs
        .list_cursor(
            &tenant,
            &incarnation,
            query.kind,
            query.status,
            query.archived,
            limit,
            query.cursor.as_deref(),
        )
        .await?;
    let jobs = page
        .jobs
        .into_iter()
        .map(|job| job.public_value())
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "pagination": "cursor",
        "count": jobs.len(),
        "total": null,
        "limit": limit,
        "next_cursor": page.next_cursor,
        "scanned": page.scanned,
        "jobs": jobs,
    })))
}

async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    let job = state.jobs.get(&tenant, &incarnation, &id).await?;
    Ok(Json(job.public_value()))
}

#[derive(Default, Deserialize)]
struct CancelRequest {
    reason: Option<String>,
}

async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: Option<Extension<User>>,
    body: Option<Json<CancelRequest>>,
) -> Result<Json<Value>, AppError> {
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    let job = state.jobs.get(&tenant, &incarnation, &id).await?;
    authorize_manage(&job, user.as_ref().map(|Extension(user)| user))?;
    let mutation = state
        .jobs
        .cancel(
            &tenant,
            &incarnation,
            &id,
            JobActor::request(user.as_ref().map(|Extension(user)| user)),
            body.and_then(|Json(body)| body.reason),
        )
        .await?;
    Ok(Json(json!({
        "job": mutation.job.public_value(),
        "changed": mutation.changed,
    })))
}

#[derive(Deserialize)]
struct RetryRequest {
    #[serde(default = "default_retry_mode")]
    mode: RetryMode,
    reason: Option<String>,
}

fn default_retry_mode() -> RetryMode {
    RetryMode::Resume
}

async fn retry(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    body: Option<Json<RetryRequest>>,
) -> Result<Json<Value>, AppError> {
    let key = idempotency_key(&headers)?;
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    let job = state.jobs.get(&tenant, &incarnation, &id).await?;
    authorize_manage(&job, user.as_ref().map(|Extension(user)| user))?;
    let body = body.map_or(
        RetryRequest {
            mode: RetryMode::Resume,
            reason: None,
        },
        |Json(body)| body,
    );
    let mutation = state
        .jobs
        .retry(
            state.clone(),
            &tenant,
            &incarnation,
            &id,
            JobActor::request(user.as_ref().map(|Extension(user)| user)),
            key,
            body.mode,
            body.reason,
        )
        .await?;
    Ok(Json(json!({
        "job": mutation.job.public_value(),
        "replayed": mutation.replayed,
    })))
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

fn authorize_manage(job: &crate::jobs::JobRecord, user: Option<&User>) -> Result<(), AppError> {
    if JobManager::can_manage(job, user) {
        Ok(())
    } else {
        Err(AppError(CogniGraphError::Forbidden(
            "only the submitting user or an Admin may manage this job".into(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;
    use cognigraph_auth::Role;
    use cognigraph_native::NativeBackend;

    #[test]
    fn idempotency_header_is_required() {
        let headers = HeaderMap::new();
        assert!(idempotency_key(&headers).is_err());
    }

    #[test]
    fn submission_envelope_rejects_unknown_fields() {
        assert!(
            serde_json::from_value::<SubmitRequest>(json!({
                "kind": "construct.evaluate",
                "input": { "space_type": "medical" },
                "unverified_semantics": true
            }))
            .is_err()
        );
    }

    #[tokio::test]
    async fn management_routes_enforce_owner_admin_and_tenant_boundaries() {
        let state = AppState::new(NativeBackend::new());
        let user = |key: &str, role| User {
            key: key.into(),
            username: key.into(),
            role,
            tenant: "default".into(),
        };
        let owner = user("owner", Role::Editor);
        let other = user("other", Role::Editor);
        let admin = user("admin", Role::Admin);
        let submission = state
            .jobs
            .submit(
                state.clone(),
                "default".into(),
                "default".into(),
                JobActor::request(Some(&owner)),
                "route-auth-r1",
                JobKind::ConstructEvaluate,
                json!({
                    "space_type": "route-auth",
                    "eval": { "space_id": "route-auth", "questions": [] }
                }),
            )
            .await
            .unwrap();

        let denied = crate::tenancy::CURRENT_TENANT
            .scope(
                "default".into(),
                cancel(
                    State(state.clone()),
                    Path(submission.job.id.clone()),
                    Some(Extension(other)),
                    None,
                ),
            )
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);

        let owned = crate::tenancy::CURRENT_TENANT
            .scope(
                "default".into(),
                cancel(
                    State(state.clone()),
                    Path(submission.job.id.clone()),
                    Some(Extension(owner)),
                    None,
                ),
            )
            .await;
        assert!(owned.is_ok());
        let administered = crate::tenancy::CURRENT_TENANT
            .scope(
                "default".into(),
                cancel(
                    State(state.clone()),
                    Path(submission.job.id.clone()),
                    Some(Extension(admin)),
                    None,
                ),
            )
            .await;
        assert!(administered.is_ok());

        let hidden = crate::tenancy::CURRENT_TENANT
            .scope(
                "other".into(),
                status(State(state.clone()), Path(submission.job.id)),
            )
            .await
            .unwrap_err()
            .into_response();
        assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
        state.jobs.shutdown().await;
    }
}
