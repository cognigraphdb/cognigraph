use super::*;

use cognigraph_core::GraphBackend;

fn make_backend() -> ArangoBackend {
    ArangoBackend::connect("http://localhost:8529", "test_db", "root", "password")
}

// --- VectorSearchMode tests ---

#[test]
fn vector_search_mode_native_eq() {
    assert_eq!(VectorSearchMode::Native, VectorSearchMode::Native);
    assert_ne!(VectorSearchMode::Native, VectorSearchMode::Fallback);
}

#[test]
fn vector_search_mode_debug() {
    let native = format!("{:?}", VectorSearchMode::Native);
    let fallback = format!("{:?}", VectorSearchMode::Fallback);
    assert_eq!(native, "Native");
    assert_eq!(fallback, "Fallback");
}

#[test]
fn vector_search_mode_clone() {
    let mode = VectorSearchMode::Fallback;
    let cloned = mode;
    assert_eq!(cloned, VectorSearchMode::Fallback);
}

// --- ArangoBackend tests ---

#[test]
fn backend_name_returns_arango() {
    let backend = make_backend();
    assert_eq!(backend.backend_name(), "arango");
}

#[test]
fn connect_creates_backend_with_native_mode() {
    let backend = make_backend();
    assert_eq!(backend.vector_mode, VectorSearchMode::Native);
}

#[test]
fn with_vector_mode_sets_fallback() {
    let backend = make_backend().with_vector_mode(VectorSearchMode::Fallback);
    assert_eq!(backend.vector_mode, VectorSearchMode::Fallback);
}

#[test]
fn with_vector_mode_sets_native() {
    let backend = ArangoBackend::connect("http://localhost:8529", "db", "u", "p")
        .with_vector_mode(VectorSearchMode::Fallback)
        .with_vector_mode(VectorSearchMode::Native);
    assert_eq!(backend.vector_mode, VectorSearchMode::Native);
}

#[test]
fn new_from_client_defaults_to_native() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "db",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    let backend = ArangoBackend::new(client);
    assert_eq!(backend.vector_mode, VectorSearchMode::Native);
    assert_eq!(backend.backend_name(), "arango");
}

#[test]
fn client_accessor_returns_reference() {
    let backend = make_backend();
    // Verify the client accessor doesn't panic and returns a valid reference
    let _client = backend.client();
}

// --- map_err tests ---

#[test]
fn map_err_404_collection_becomes_collection_not_found() {
    let err = map_err(ArangoError::Server {
        code: 404,
        message: "collection or view not found".into(),
    });
    match err {
        CogniGraphError::CollectionNotFound(msg) => {
            assert!(msg.contains("collection"));
        }
        other => panic!("Expected CollectionNotFound, got: {other:?}"),
    }
}

#[test]
fn map_err_404_non_collection_becomes_backend_error() {
    let err = map_err(ArangoError::Server {
        code: 404,
        message: "document not found".into(),
    });
    match err {
        CogniGraphError::BackendError(msg) => {
            assert!(msg.contains("404"));
            assert!(msg.contains("document not found"));
        }
        other => panic!("Expected BackendError, got: {other:?}"),
    }
}

#[test]
fn map_err_401_becomes_auth_error() {
    let err = map_err(ArangoError::Server {
        code: 401,
        message: "unauthorized".into(),
    });
    match err {
        CogniGraphError::AuthError(msg) => assert_eq!(msg, "unauthorized"),
        other => panic!("Expected AuthError, got: {other:?}"),
    }
}

#[test]
fn map_err_403_becomes_auth_error() {
    let err = map_err(ArangoError::Server {
        code: 403,
        message: "forbidden".into(),
    });
    match err {
        CogniGraphError::AuthError(msg) => assert_eq!(msg, "forbidden"),
        other => panic!("Expected AuthError, got: {other:?}"),
    }
}

#[test]
fn map_err_other_server_becomes_backend_error() {
    let err = map_err(ArangoError::Server {
        code: 500,
        message: "internal error".into(),
    });
    match err {
        CogniGraphError::BackendError(msg) => assert_eq!(msg, "internal error"),
        other => panic!("Expected BackendError, got: {other:?}"),
    }
}

#[test]
fn map_err_http_becomes_backend_error() {
    let err = map_err(ArangoError::Http {
        status: 502,
        body: "bad gateway".into(),
    });
    match err {
        CogniGraphError::BackendError(msg) => {
            assert!(msg.contains("502"));
            assert!(msg.contains("bad gateway"));
        }
        other => panic!("Expected BackendError, got: {other:?}"),
    }
}

#[test]
fn map_err_deserialization_becomes_backend_error() {
    let err = map_err(ArangoError::Deserialization {
        message: "invalid json".into(),
        body: "not json".into(),
    });
    match err {
        CogniGraphError::BackendError(msg) => {
            assert!(msg.contains("Deserialization"));
            assert!(msg.contains("invalid json"));
        }
        other => panic!("Expected BackendError, got: {other:?}"),
    }
}

// --- extract_key tests ---

#[test]
fn extract_key_from_top_level() {
    let val = serde_json::json!({"_key": "12345", "_id": "col/12345"});
    assert_eq!(extract_key(&val), Some("12345".into()));
}

#[test]
fn extract_key_from_new_wrapper() {
    let val = serde_json::json!({"new": {"_key": "67890"}});
    assert_eq!(extract_key(&val), Some("67890".into()));
}

#[test]
fn extract_key_prefers_top_level_over_new() {
    let val = serde_json::json!({"_key": "top", "new": {"_key": "nested"}});
    assert_eq!(extract_key(&val), Some("top".into()));
}

#[test]
fn extract_key_returns_none_when_missing() {
    let val = serde_json::json!({"_id": "col/123"});
    assert_eq!(extract_key(&val), None);
}

#[test]
fn extract_key_returns_none_for_non_string() {
    let val = serde_json::json!({"_key": 12345});
    assert_eq!(extract_key(&val), None);
}

// --- extract_doc tests ---

#[test]
fn extract_doc_unwraps_new() {
    let val = serde_json::json!({"new": {"_key": "1", "name": "Alice"}});
    let doc = extract_doc(val);
    assert_eq!(doc, serde_json::json!({"_key": "1", "name": "Alice"}));
}

#[test]
fn extract_doc_returns_as_is_without_new() {
    let val = serde_json::json!({"_key": "1", "name": "Bob"});
    let doc = extract_doc(val.clone());
    assert_eq!(doc, val);
}
