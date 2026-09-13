//! Explicit batch entrypoint for context-expansion side-view generation
//! (decision_sideviews_provider_and_benchmark.md). A POST enqueues one durable,
//! tenant-scoped `sideviews.generate` job that generates, embeds, and stores
//! retrieval side-views for every document in a source collection. The heavy,
//! non-deterministic model work runs in the background job framework — this
//! route only captures the immutable input snapshot and hands it off, exactly
//! like the generic durable-job submit endpoint.

use axum::extract::{DefaultBodyLimit, Extension, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use cognigraph_auth::User;
use cognigraph_core::CogniGraphError;
use serde_json::{Value, json};

use crate::error::AppError;
use crate::jobs::{JobActor, JobKind, JobManager, MAX_JOB_INPUT_BYTES};
use crate::state::AppState;
use crate::tenancy::current_tenant;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/generate", post(generate))
        .layer(DefaultBodyLimit::max(MAX_JOB_INPUT_BYTES + 64 * 1024))
}

/// POST /api/sideviews/generate — enqueue side-view generation over a source
/// collection. Body: `{ collection, text_field?, count?, regenerate? }`; the
/// source-collection eligibility, provider availability, count clamp, and
/// document enumeration are all validated when the job payload is prepared. The
/// mandatory `Idempotency-Key` header makes the submission replay-safe like
/// every other durable job. Returns the job submission envelope with a Location
/// header pointing at the tenant-scoped job detail URL.
async fn generate(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: HeaderMap,
    Json(input): Json<Value>,
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
            JobKind::SideviewsGenerate,
            input,
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
