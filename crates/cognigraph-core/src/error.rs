use thiserror::Error;

#[derive(Error, Debug)]
pub enum CogniGraphError {
    #[error("Document not found: {collection}/{key}")]
    DocumentNotFound { collection: String, key: String },

    #[error("Collection not found: {0}")]
    CollectionNotFound(String),

    #[error("Document conflict: {0}")]
    DocumentConflict(String),

    #[error("Capacity exceeded: {0}")]
    CapacityExceeded(String),

    #[error("Database connection error: {0}")]
    ConnectionError(String),

    #[error("Query execution error: {0}")]
    QueryError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("enterprise_feature_required: {0}")]
    EnterpriseFeatureRequired(String),

    #[error("Embedding error: {0}")]
    EmbeddingError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Backend error: {0}")]
    BackendError(String),

    #[error("Lua script error: {0}")]
    LuaError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}

pub type Result<T> = std::result::Result<T, CogniGraphError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_document_not_found() {
        let err = CogniGraphError::DocumentNotFound {
            collection: "docs".into(),
            key: "123".into(),
        };
        assert_eq!(format!("{err}"), "Document not found: docs/123");
    }

    #[test]
    fn display_collection_not_found() {
        let err = CogniGraphError::CollectionNotFound("users".into());
        assert_eq!(format!("{err}"), "Collection not found: users");
    }

    #[test]
    fn display_capacity_exceeded() {
        let err = CogniGraphError::CapacityExceeded("durable job queue is full".into());
        assert_eq!(
            format!("{err}"),
            "Capacity exceeded: durable job queue is full"
        );
    }

    #[test]
    fn display_connection_error() {
        let err = CogniGraphError::ConnectionError("timeout".into());
        assert_eq!(format!("{err}"), "Database connection error: timeout");
    }

    #[test]
    fn display_query_error() {
        let err = CogniGraphError::QueryError("syntax".into());
        assert_eq!(format!("{err}"), "Query execution error: syntax");
    }

    #[test]
    fn display_auth_error() {
        let err = CogniGraphError::AuthError("invalid token".into());
        assert_eq!(format!("{err}"), "Authentication error: invalid token");
    }

    #[test]
    fn display_embedding_error() {
        let err = CogniGraphError::EmbeddingError("dim mismatch".into());
        assert_eq!(format!("{err}"), "Embedding error: dim mismatch");
    }

    #[test]
    fn display_backend_error() {
        let err = CogniGraphError::BackendError("unavailable".into());
        assert_eq!(format!("{err}"), "Backend error: unavailable");
    }

    #[test]
    fn display_lua_error() {
        let err = CogniGraphError::LuaError("runtime fault".into());
        assert_eq!(format!("{err}"), "Lua script error: runtime fault");
    }

    #[test]
    fn display_validation_error() {
        let err = CogniGraphError::ValidationError("missing field".into());
        assert_eq!(format!("{err}"), "Validation error: missing field");
    }

    #[test]
    fn serialization_error_from_serde() {
        let bad_json = "not json";
        let serde_err = serde_json::from_str::<serde_json::Value>(bad_json).unwrap_err();
        let err = CogniGraphError::from(serde_err);
        let msg = format!("{err}");
        assert!(msg.starts_with("Serialization error:"));
    }

    #[test]
    fn error_is_debug() {
        let err = CogniGraphError::BackendError("test".into());
        let debug = format!("{:?}", err);
        assert!(debug.contains("BackendError"));
    }
}
