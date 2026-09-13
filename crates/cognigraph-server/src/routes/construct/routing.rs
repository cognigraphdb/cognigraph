//! Routing.

use super::*;

/// Routes that mutate the graph (write scope in main.rs).
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ingest", post(ingest))
        .route("/governed-ingest", post(governed_ingest))
        .route("/propose", post(propose))
        .route("/review", post(review))
        // `async: true` hands the whole corpus to the durable job framework, so
        // this route legitimately carries job-sized input. The larger limit is
        // scoped to /draft alone — every other construct route keeps the default
        // request ceiling.
        .route(
            "/draft",
            post(draft).layer(DefaultBodyLimit::max(
                crate::jobs::MAX_JOB_INPUT_BYTES + 64 * 1024,
            )),
        )
        .route("/draft/{id}/accept", post(draft_accept))
        .route("/directed", post(directed))
}
/// Read-effect routes (read scope in main.rs, despite the POST verb).
pub fn eval_router() -> Router<AppState> {
    Router::new()
        .route("/evaluate", post(evaluate_space))
        .route("/advise", post(advise))
        .route("/answer-eval", post(answer_eval_route))
}
