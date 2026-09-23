//! CG-86: a unique-constraint violation is a 409 with a machine-readable
//! code, distinct from a duplicate `_key` conflict.

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cognigraph_core::CogniGraphError;

use super::AppError;

async fn parts(err: AppError) -> (StatusCode, serde_json::Value) {
    let response = err.into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn unique_violation_is_409_with_a_code() {
    let (status, json) = parts(AppError(CogniGraphError::UniqueViolation {
        collection: "users".into(),
        index: "email_unique".into(),
        existing: "u1".into(),
    }))
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["code"], "unique_violation");
    let message = json["error"].as_str().unwrap();
    assert!(
        message.contains("email_unique") && message.contains("users/u1"),
        "{message}"
    );
}

#[tokio::test]
async fn key_conflict_keeps_its_shape_without_a_code() {
    let (status, json) = parts(AppError(CogniGraphError::DocumentConflict(
        "users/u1".into(),
    )))
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(json.get("code").is_none(), "{json}");
}
