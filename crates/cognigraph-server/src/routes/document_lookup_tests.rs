//! Exercise production query and Lua routers with deterministic backend faults.
use std::collections::HashMap;

use async_trait::async_trait;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use cognigraph_core::{
    CogniGraphError, CollectionType, Direction, DocumentId, GraphBackend, IndexDef, QueryLanguage,
    Result, SearchHit, TraversalOpts, TraversalPath, VectorSearchOpts,
};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::state::AppState;

const SURFACES: &[&str] = &["/api/query", "/api/search/query", "/api/lua/execute"];

fn router() -> Router {
    Router::new()
        .nest("/api/query", super::query::router())
        .nest("/api/search", super::search::router())
        .nest("/api/lua", super::lua::router())
        .with_state(AppState::new(LookupBackend))
}

async fn post(app: &Router, path: &str, payload: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

fn payload(surface: &str, query: &str, id: &str) -> Value {
    if surface == "/api/lua/execute" {
        json!({"script":format!(
            "return graph.query([==[{query}]==], {{id={}}})",
            serde_json::to_string(id).unwrap()
        )})
    } else {
        json!({"query":query,"bind_vars":{"id":id}})
    }
}

#[tokio::test]
async fn client_plan_errors_return_matching_bad_requests_before_backend_access() {
    // LookupBackend panics on collection reads/writes: invalid plans must not
    // reach storage, including when the caller requests EXPLAIN or analysis.
    let app = router();
    for query in [
        "RETURN",
        "RETURN unknown",
        include_str!("../../../cognigraph-query/tests/corpus/parse_err/sq_postcollect_return.cgql"),
        include_str!("../../../cognigraph-query/tests/corpus/validate_err/sq_into_inline.cgql"),
    ] {
        for prefix in ["", "EXPLAIN ", "EXPLAIN ANALYZE "] {
            let query = format!("{prefix}{query}");
            let mut bodies = Vec::new();
            for surface in &SURFACES[..2] {
                let (status, body) = post(&app, surface, json!({"query": query})).await;
                assert_eq!(
                    status,
                    StatusCode::BAD_REQUEST,
                    "{surface}: {query}: {body}"
                );
                assert!(
                    body["error"]
                        .as_str()
                        .unwrap()
                        .starts_with("Validation error:")
                );
                assert!(body.get("results").is_none());
                bodies.push(body);
            }
            assert_eq!(bodies[0], bodies[1], "{query}");
        }
    }
}

#[tokio::test]
async fn runtime_errors_keep_server_error_status_on_both_query_routes() {
    let app = router();
    for prefix in ["", "EXPLAIN ANALYZE "] {
        let query = format!("{prefix}FOR x IN 1 RETURN x");
        let mut bodies = Vec::new();
        for surface in &SURFACES[..2] {
            let (status, body) = post(&app, surface, json!({"query": query})).await;
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
            assert_eq!(
                body["error"],
                "Query execution error: FOR source must be an array"
            );
            bodies.push(body);
        }
        assert_eq!(bodies[0], bodies[1]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn document_errors_keep_http_status_on_every_query_surface() {
    let app = router();
    for (id, expected, detail) in [
        ("denied/a", StatusCode::FORBIDDEN, "synthetic denial"),
        (
            "unavailable/a",
            StatusCode::SERVICE_UNAVAILABLE,
            "synthetic outage",
        ),
        (
            "broken/a",
            StatusCode::INTERNAL_SERVER_ERROR,
            "synthetic storage failure",
        ),
        ("_users/a", StatusCode::FORBIDDEN, "system-reserved"),
    ] {
        for query in [
            "RETURN DOCUMENT(@id)",
            r#"RETURN DOCUMENT(["notes/a", @id])"#,
            r#"FOR id IN [@id] FILTER DOCUMENT(id).v == 1 RETURN id"#,
            r#"FOR id IN [@id] LET d=DOCUMENT(id) LET v=d.v LIMIT 1 RETURN v"#,
        ] {
            for surface in SURFACES {
                for prefix in ["", "EXPLAIN ANALYZE "] {
                    let query = format!("{prefix}{query}");
                    let (status, body) = post(&app, surface, payload(surface, &query, id)).await;
                    assert_eq!(status, expected, "{surface}: {query}: {body}");
                    assert!(body["error"].as_str().unwrap().contains(detail), "{body}");
                    assert!(body.get("results").is_none() && body.get("result").is_none());
                }
                let query = format!("EXPLAIN {query}");
                let (status, body) = post(&app, surface, payload(surface, &query, id)).await;
                assert_eq!(status, StatusCode::OK, "plain EXPLAIN: {body}");
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_and_malformed_document_ids_remain_successful() {
    let app = router();
    for (id, absent) in [
        ("notes/a", false),
        ("notes/missing", true),
        ("not-an-id", true),
    ] {
        for surface in SURFACES {
            let query = "RETURN DOCUMENT(@id) == null";
            let (status, body) = post(&app, surface, payload(surface, query, id)).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            let field = if *surface == "/api/lua/execute" {
                "result"
            } else {
                "results"
            };
            assert_eq!(body[field], json!([absent]));
            let query = format!("EXPLAIN ANALYZE {query}");
            let (status, body) = post(&app, surface, payload(surface, &query, id)).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body[field][0]["stats"]["result_rows"], 1);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn lua_connection_classification_is_typed_and_pcall_can_handle_errors() {
    let app = router();
    for script in [
        r#"return graph.query('RETURN DOCUMENT("unavailable/a")')"#,
        r#"return graph.get_document('unavailable', 'a')"#,
    ] {
        let (status, body) = post(&app, "/api/lua/execute", json!({"script":script})).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        let caught = format!("local ok,result=pcall(function() {script} end); return {{ok=ok}}");
        let (status, body) = post(&app, "/api/lua/execute", json!({"script":caught})).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["result"]["ok"], false);
    }
    let (status, body) = post(
        &app,
        "/api/lua/execute",
        json!({
            "script":"error('Database connection error: synthetic outage; forbidden')"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
}

struct LookupBackend;

#[async_trait]
impl GraphBackend for LookupBackend {
    fn backend_name(&self) -> &str {
        "document-errors-test"
    }
    fn query_language(&self) -> QueryLanguage {
        QueryLanguage::Cgql
    }
    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<Value>> {
        match collection {
            "denied" => Err(CogniGraphError::Forbidden("synthetic denial".into())),
            "unavailable" => Err(CogniGraphError::ConnectionError("synthetic outage".into())),
            "broken" => Err(CogniGraphError::BackendError(
                "synthetic storage failure".into(),
            )),
            "notes" if key == "a" => Ok(Some(json!({"_id":"notes/a", "v":1}))),
            "notes" => Ok(None),
            _ => panic!("unexpected backend lookup: {collection}/{key}"),
        }
    }
    async fn create_document(&self, _: &str, _: Value) -> Result<DocumentId> {
        unimplemented!()
    }
    async fn update_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn replace_document(&self, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn delete_document(&self, _: &str, _: &str) -> Result<bool> {
        unimplemented!()
    }
    async fn list_documents(
        &self,
        _: &str,
        _: Option<usize>,
        _: Option<usize>,
    ) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn create_edge(&self, _: &str, _: Value) -> Result<DocumentId> {
        unimplemented!()
    }
    async fn upsert_edge(&self, _: &str, _: &str, _: &str, _: &str, _: Value) -> Result<Value> {
        unimplemented!()
    }
    async fn get_edges(&self, _: &str, _: &str, _: Direction) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn traverse(&self, _: &str, _: &TraversalOpts) -> Result<Vec<TraversalPath>> {
        unimplemented!()
    }
    async fn vector_search(
        &self,
        _: &str,
        _: &[f64],
        _: &VectorSearchOpts,
    ) -> Result<Vec<SearchHit>> {
        unimplemented!()
    }
    async fn query(&self, _: &str, _: HashMap<String, Value>) -> Result<Vec<Value>> {
        unimplemented!()
    }
    async fn ensure_collection(&self, _: &str, _: CollectionType) -> Result<()> {
        unimplemented!()
    }
    async fn ensure_index(&self, _: &str, _: &IndexDef) -> Result<()> {
        unimplemented!()
    }
    async fn drop_collection(&self, _: &str) -> Result<()> {
        unimplemented!()
    }
}
