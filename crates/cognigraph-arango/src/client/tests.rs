use super::*;

// --- ArangoAuth tests ---

#[test]
fn basic_auth_generates_correct_base64_header() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "test_db",
        ArangoAuth::Basic {
            username: "root".into(),
            password: "secret".into(),
        },
    );
    let headers = client.headers();
    let auth = headers.get(AUTHORIZATION).unwrap().to_str().unwrap();
    // "root:secret" in base64 is "cm9vdDpzZWNyZXQ="
    assert_eq!(auth, "Basic cm9vdDpzZWNyZXQ=");
}

#[test]
fn basic_auth_empty_password() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "_system",
        ArangoAuth::Basic {
            username: "root".into(),
            password: "".into(),
        },
    );
    let headers = client.headers();
    let auth = headers.get(AUTHORIZATION).unwrap().to_str().unwrap();
    // "root:" in base64 is "cm9vdDo="
    assert_eq!(auth, "Basic cm9vdDo=");
}

#[test]
fn bearer_auth_stores_token_correctly() {
    let token = "eyJhbGciOiJIUzI1NiJ9.test_payload.signature";
    let client = ArangoClient::new(
        "http://localhost:8529",
        "test_db",
        ArangoAuth::Bearer(token.into()),
    );
    let headers = client.headers();
    let auth = headers.get(AUTHORIZATION).unwrap().to_str().unwrap();
    assert_eq!(auth, format!("bearer {token}"));
}

// --- ArangoClient construction tests ---

#[test]
fn client_stores_base_url_and_database() {
    let client = ArangoClient::new(
        "http://my-arango:8529",
        "my_database",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    // Verify via URL building (fields are private, so test indirectly)
    let url = client.db_url("/_api/version");
    assert_eq!(url, "http://my-arango:8529/_db/my_database/_api/version");
}

#[test]
fn client_strips_trailing_slash_from_base_url() {
    let client = ArangoClient::new(
        "http://localhost:8529/",
        "test_db",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    let url = client.server_url("/_api/version");
    assert_eq!(url, "http://localhost:8529/_api/version");
}

#[test]
fn client_strips_multiple_trailing_slashes() {
    let client = ArangoClient::new(
        "http://localhost:8529///",
        "db",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    // trim_end_matches('/') removes all trailing slashes
    let url = client.server_url("/_api/version");
    assert_eq!(url, "http://localhost:8529/_api/version");
}

// --- URL building tests ---

#[test]
fn db_url_builds_correct_document_url() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "mydb",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    assert_eq!(
        client.db_url("/_api/document/users"),
        "http://localhost:8529/_db/mydb/_api/document/users"
    );
}

#[test]
fn db_url_builds_correct_document_key_url() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "mydb",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    assert_eq!(
        client.db_url("/_api/document/users/12345"),
        "http://localhost:8529/_db/mydb/_api/document/users/12345"
    );
}

#[test]
fn db_url_builds_correct_cursor_url() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "mydb",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    assert_eq!(
        client.db_url("/_api/cursor"),
        "http://localhost:8529/_db/mydb/_api/cursor"
    );
}

#[test]
fn server_url_builds_without_db_prefix() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "mydb",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    assert_eq!(
        client.server_url("/_api/version"),
        "http://localhost:8529/_api/version"
    );
}

// --- use_database tests ---

#[test]
fn use_database_switches_context() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "original_db",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    let switched = client.use_database("other_db");
    assert_eq!(
        switched.db_url("/_api/collection"),
        "http://localhost:8529/_db/other_db/_api/collection"
    );
}

#[test]
fn use_database_preserves_auth() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "db1",
        ArangoAuth::Basic {
            username: "root".into(),
            password: "secret".into(),
        },
    );
    let switched = client.use_database("db2");
    let headers = switched.headers();
    let auth = headers.get(AUTHORIZATION).unwrap().to_str().unwrap();
    assert_eq!(auth, "Basic cm9vdDpzZWNyZXQ=");
}

// --- Headers tests ---

#[test]
fn headers_include_content_type_json() {
    let client = ArangoClient::new(
        "http://localhost:8529",
        "db",
        ArangoAuth::Basic {
            username: "u".into(),
            password: "p".into(),
        },
    );
    let headers = client.headers();
    let ct = headers.get(CONTENT_TYPE).unwrap().to_str().unwrap();
    assert_eq!(ct, "application/json");
}

// --- Collection request shape tests ---

#[test]
fn ordinary_collection_create_payload_is_unchanged() {
    assert_eq!(
        collection_create_body("documents", 2),
        serde_json::json!({
            "name": "documents",
            "type": 2,
        })
    );
}

#[test]
fn system_collection_create_payload_sets_arango_flag() {
    assert_eq!(
        collection_create_body("_users", 2),
        serde_json::json!({
            "name": "_users",
            "type": 2,
            "isSystem": true,
        })
    );
}

#[test]
fn ordinary_collection_drop_path_is_unchanged() {
    assert_eq!(
        collection_drop_path("documents"),
        "/_api/collection/documents"
    );
}

#[test]
fn system_collection_drop_path_sets_arango_flag() {
    assert_eq!(
        collection_drop_path("_tokens"),
        "/_api/collection/_tokens?isSystem=true"
    );
}

// --- ArangoError Display tests ---

#[test]
fn error_display_server() {
    let err = ArangoError::Server {
        code: 404,
        message: "collection not found".into(),
    };
    assert_eq!(
        err.to_string(),
        "ArangoDB server error (404): collection not found"
    );
}

#[test]
fn error_display_http() {
    let err = ArangoError::Http {
        status: 502,
        body: "Bad Gateway".into(),
    };
    assert_eq!(err.to_string(), "HTTP error (502): Bad Gateway");
}

#[test]
fn error_display_deserialization() {
    let err = ArangoError::Deserialization {
        message: "expected value".into(),
        body: "not json".into(),
    };
    let display = err.to_string();
    assert!(display.contains("expected value"));
    assert!(display.contains("not json"));
}

// --- ArangoErrorResponse deserialization ---

#[test]
fn arango_error_response_deserializes() {
    let json = r#"{"error": true, "code": 404, "errorMessage": "collection not found"}"#;
    let resp: ArangoErrorResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.code, 404);
    assert_eq!(resp.error_message, "collection not found");
}

#[test]
fn arango_error_response_defaults() {
    let json = r#"{}"#;
    let resp: ArangoErrorResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.code, 0);
    assert_eq!(resp.error_message, "");
}
