use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use cognigraph_core::CogniGraphError;

/// Wrapper to convert CogniGraphError into Axum HTTP responses.
#[derive(Debug)]
pub struct AppError(pub CogniGraphError);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let retry_after = matches!(&self.0, CogniGraphError::CapacityExceeded(_));
        let (status, message) = match &self.0 {
            CogniGraphError::DocumentNotFound { .. } => (StatusCode::NOT_FOUND, self.0.to_string()),
            CogniGraphError::CollectionNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            CogniGraphError::DocumentConflict(_) => (StatusCode::CONFLICT, self.0.to_string()),
            CogniGraphError::CapacityExceeded(_) => {
                (StatusCode::TOO_MANY_REQUESTS, self.0.to_string())
            }
            CogniGraphError::AuthError(_) => (StatusCode::UNAUTHORIZED, self.0.to_string()),
            CogniGraphError::Forbidden(_) => (StatusCode::FORBIDDEN, self.0.to_string()),
            CogniGraphError::EnterpriseFeatureRequired(_) => {
                (StatusCode::FORBIDDEN, self.0.to_string())
            }
            CogniGraphError::ValidationError(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            CogniGraphError::ConnectionError(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, self.0.to_string())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
        };

        let mut body = serde_json::json!({ "error": message });
        if matches!(&self.0, CogniGraphError::EnterpriseFeatureRequired(_)) {
            body["code"] = serde_json::json!("enterprise_feature_required");
        }
        let mut response = (status, axum::Json(body)).into_response();
        if retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
        }
        response
    }
}

impl From<CogniGraphError> for AppError {
    fn from(err: CogniGraphError) -> Self {
        AppError(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    /// Helper: convert an AppError into a (StatusCode, serde_json::Value) pair.
    async fn response_parts(err: AppError) -> (StatusCode, serde_json::Value) {
        let response = err.into_response();
        let status = response.status();
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn document_not_found_returns_404() {
        let err = AppError(CogniGraphError::DocumentNotFound {
            collection: "docs".into(),
            key: "42".into(),
        });
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(json.get("error").is_some());
        assert!(json["error"].as_str().unwrap().contains("docs/42"));
    }

    #[tokio::test]
    async fn collection_not_found_returns_404() {
        let err = AppError(CogniGraphError::CollectionNotFound("users".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(json["error"].as_str().unwrap().contains("users"));
    }

    #[tokio::test]
    async fn auth_error_returns_401() {
        let err = AppError(CogniGraphError::AuthError("bad token".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(json["error"].as_str().unwrap().contains("bad token"));
    }

    #[tokio::test]
    async fn validation_error_returns_400() {
        let err = AppError(CogniGraphError::ValidationError("missing field".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(json["error"].as_str().unwrap().contains("missing field"));
    }

    #[tokio::test]
    async fn connection_error_returns_503() {
        let err = AppError(CogniGraphError::ConnectionError("timeout".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(json["error"].as_str().unwrap().contains("timeout"));
    }

    #[tokio::test]
    async fn backend_error_returns_500() {
        let err = AppError(CogniGraphError::BackendError("disk full".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(json["error"].as_str().unwrap().contains("disk full"));
    }

    #[tokio::test]
    async fn query_error_returns_500() {
        let err = AppError(CogniGraphError::QueryError("syntax error".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(json["error"].as_str().unwrap().contains("syntax error"));
    }

    #[tokio::test]
    async fn embedding_error_returns_500() {
        let err = AppError(CogniGraphError::EmbeddingError("dim mismatch".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(json["error"].as_str().unwrap().contains("dim mismatch"));
    }

    #[tokio::test]
    async fn serialization_error_returns_500() {
        let serde_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err = AppError(CogniGraphError::SerializationError(serde_err));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(json.get("error").is_some());
    }

    #[tokio::test]
    async fn lua_error_returns_500() {
        let err = AppError(CogniGraphError::LuaError("runtime fault".into()));
        let (status, json) = response_parts(err).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(json["error"].as_str().unwrap().contains("runtime fault"));
    }

    #[test]
    fn from_cognigraph_error() {
        let core_err = CogniGraphError::BackendError("test".into());
        let app_err: AppError = core_err.into();
        assert!(matches!(app_err.0, CogniGraphError::BackendError(_)));
    }

    #[tokio::test]
    async fn document_conflict_maps_to_409() {
        let err = AppError(CogniGraphError::DocumentConflict("docs/a".into()));
        let (status, body) = response_parts(err).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "Document conflict: docs/a");
    }

    #[tokio::test]
    async fn capacity_exceeded_maps_to_429_with_retry_after() {
        let response = AppError(CogniGraphError::CapacityExceeded(
            "durable job queue is full".into(),
        ))
        .into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get(header::RETRY_AFTER).unwrap(), "1");
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(
            body["error"],
            "Capacity exceeded: durable job queue is full"
        );
    }
}
