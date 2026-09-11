use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    let router = Router::new()
        .route("/", get(health_check))
        .route("/database", get(database_check));
    #[cfg(feature = "enterprise")]
    let router = router
        .route("/jobs", get(jobs_check))
        .route("/promotions", get(promotions_check));
    router
}

#[cfg(feature = "enterprise")]
async fn promotions_check(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.promotions.health() {
        Ok(()) => (
            StatusCode::OK,
            Json(crate::promotions::promotion_status_value(true, None)),
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(crate::promotions::promotion_status_value(
                false,
                Some("promotion repository unavailable"),
            )),
        ),
    }
}

#[cfg(feature = "enterprise")]
async fn jobs_check(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    match state.jobs.health() {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ok",
                "jobs": "ready",
            })),
        ),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "jobs": "unavailable",
                "error": error,
            })),
        ),
    }
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "service": "cognigraph",
        "version": env!("CARGO_PKG_VERSION"),
        "edition": if cfg!(feature = "enterprise") { "enterprise" } else { "community" },
    }))
}

async fn database_check(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    database_response(state.backend.ping().await)
}

fn database_response(result: cognigraph_core::Result<()>) -> (StatusCode, Json<serde_json::Value>) {
    match result {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "ok",
                "database": "connected",
            })),
        ),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "status": "error",
                "database": "disconnected",
                "error": error.to_string(),
            })),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_core::CogniGraphError;

    #[test]
    fn database_ping_failure_is_not_ready() {
        let (status, Json(body)) = database_response(Err(CogniGraphError::ConnectionError(
            "database unavailable".into(),
        )));

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["status"], "error");
        assert_eq!(body["database"], "disconnected");
        assert!(body["error"].as_str().unwrap().contains("unavailable"));
    }

    #[test]
    fn database_ping_success_is_ready() {
        let (status, Json(body)) = database_response(Ok(()));

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["database"], "connected");
    }
}
