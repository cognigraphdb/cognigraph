//! CG-198: a CGQL error answers the status of whoever caused it, on both
//! query routes.

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use axum::response::IntoResponse;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use tower::ServiceExt;

use super::*;
use crate::state::AppState;

const ROUTES: [&str; 2] = ["/api/query", "/api/search/query"];

fn unlimited() -> ExecutionBudget {
    ExecutionBudget::unlimited()
}

async fn state(budget: ExecutionBudget) -> AppState {
    let mut state = AppState::new(NativeBackend::new());
    state.cgql_budget = budget;
    for key in ["a", "b"] {
        state
            .backend
            .create_document("notes", json!({"_key": key, "n": 1}))
            .await
            .unwrap();
    }
    state
}

async fn ask(state: &AppState, route: &str, body: Value) -> (StatusCode, Value, bool) {
    let app = Router::new()
        .nest("/api/query", crate::routes::query::router())
        .nest("/api/search", crate::routes::search::router())
        .with_state(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(route)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let retry = response.headers().contains_key(header::RETRY_AFTER);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap(), retry)
}

async fn query(state: &AppState, route: &str, text: &str) -> (StatusCode, Value, bool) {
    ask(state, route, json!({ "query": text })).await
}

const TEN_KILOBYTES: &str = r#"LET a = "0123456789"
    LET b = CONCAT(a, a, a, a, a, a, a, a, a, a)
    LET c = CONCAT(b, b, b, b, b, b, b, b, b, b)
    LET d = CONCAT(c, c, c, c, c, c, c, c, c, c)
    RETURN LENGTH(d)"#;

#[tokio::test]
async fn each_budget_answers_422_and_names_itself_and_its_limit() {
    let cases = [
        (
            ExecutionBudget {
                max_source_rows: Some(4),
                ..unlimited()
            },
            "FOR n IN [1, 2, 3, 4, 5] RETURN n",
            ("source_rows", 4, "rows"),
            "source row budget (4 rows)",
        ),
        (
            ExecutionBudget {
                max_intermediate_rows: Some(4),
                ..unlimited()
            },
            "FOR a IN [1, 2, 3] FOR b IN [1, 2, 3] RETURN 1",
            ("intermediate_rows", 4, "rows"),
            "intermediate row budget (4 rows)",
        ),
        (
            ExecutionBudget {
                max_memory_bytes: Some(4_096),
                ..unlimited()
            },
            TEN_KILOBYTES,
            ("memory", 4_096, "bytes"),
            "memory budget (4096 bytes)",
        ),
        (
            // A zero budget has expired before the first row.
            ExecutionBudget {
                time_budget_ms: Some(0),
                ..unlimited()
            },
            "FOR d IN notes RETURN d",
            ("time", 0, "ms"),
            "time budget (0 ms)",
        ),
    ];
    for (budget, text, (name, limit, unit), detail) in cases {
        let state = state(budget).await;
        for route in ROUTES {
            let (status, body, retry) = query(&state, route, text).await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{route} {name}");
            assert_eq!(body["code"], "query_budget_exceeded", "{route} {body}");
            assert_eq!(body["budget"], name, "{route} {body}");
            assert_eq!(body["limit"], limit, "{route} {body}");
            assert_eq!(body["unit"], unit, "{route} {body}");
            let message = body["error"].as_str().unwrap();
            assert!(
                message.starts_with("Query budget exceeded: ") && message.contains(detail),
                "{route}: {message}"
            );
            // Nothing invites a retry of a request that fails the same way.
            assert!(!retry, "{route} {name}");
        }
    }
}

#[tokio::test]
async fn a_query_exactly_at_its_budget_still_answers_200() {
    let state = state(ExecutionBudget {
        max_source_rows: Some(5),
        max_intermediate_rows: Some(5),
        ..unlimited()
    })
    .await;
    for route in ROUTES {
        let (status, body, _) = query(&state, route, "FOR n IN [1, 2, 3, 4, 5] RETURN n").await;
        assert_eq!(status, StatusCode::OK, "{route} {body}");
        assert_eq!(body["count"], 5);
        assert!(body.get("code").is_none());
    }
}

#[tokio::test]
async fn errors_in_the_callers_query_answer_400() {
    let state = state(unlimited()).await;
    let cases: [(&str, Value, &str); 7] = [
        (
            "FOR x IN 1 RETURN x",
            json!({}),
            "FOR source must be an array",
        ),
        (r#"RETURN 1 + "a""#, json!({}), "expected numeric value"),
        (
            "FOR d IN notes FILTER d.n RETURN d",
            json!({}),
            "expected boolean value",
        ),
        ("RETURN @missing", json!({}), "missing bind variables"),
        ("RETURN 1", json!({"extra": 1}), "unexpected bind variables"),
        ("RETURN NOPE(1)", json!({}), "NOPE"),
        ("FOR d IN notes RETURN", json!({}), ""),
    ];
    for route in ROUTES {
        for (text, bind_vars, detail) in &cases {
            let (status, body, _) = ask(
                &state,
                route,
                json!({"query": text, "bind_vars": bind_vars}),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{route} {text}: {body}");
            let message = body["error"].as_str().unwrap();
            assert!(
                message.starts_with("Validation error: ") && message.contains(detail),
                "{route} {text}: {message}"
            );
            assert!(body.get("code").is_none(), "{body}");
        }
    }
}

#[tokio::test]
async fn a_mutation_is_refused_on_the_read_route_and_runs_on_the_write_route() {
    let state = state(unlimited()).await;
    let insert = r#"INSERT { _key: "c", n: 3 } INTO notes"#;
    let (status, body, _) = query(&state, "/api/search/query", insert).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .contains("mutations are not allowed on this endpoint"),
        "{body}"
    );
    assert!(
        state
            .backend
            .get_document("notes", "c")
            .await
            .unwrap()
            .is_none()
    );
    let (status, body, _) = query(&state, "/api/query", insert).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The same key again is the caller's conflict, as before.
    let (status, _, _) = query(&state, "/api/query", insert).await;
    assert_eq!(status, StatusCode::CONFLICT);
}

async fn parts(error: ExecutionError) -> (StatusCode, Value) {
    let response = cgql_error(
        error,
        &ExecutionBudget {
            time_budget_ms: Some(30_000),
            ..unlimited()
        },
    )
    .into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn every_execution_error_has_a_decided_status() {
    use ExecutionError as E;
    let cases = [
        (E::BindVariableNotFound("x".into()), 400),
        (E::MissingBindVariables("x".into()), 400),
        (E::UnexpectedBindVariables("x".into()), 400),
        (E::IdentifierNotFound("x".into()), 400),
        (E::UnsupportedFunction("X".into()), 400),
        (E::InvalidInput("too deep".into()), 400),
        (E::ExpectedNumber, 400),
        (E::ExpectedBool, 400),
        (E::ExpectedVector, 400),
        (E::ExpectedArraySource, 400),
        (E::InvalidDocumentKey, 400),
        (
            E::MutationKeyOutsideCollection {
                id: "other/k".into(),
                collection: "notes".into(),
            },
            400,
        ),
        (E::MutationNotAllowed, 403),
        (E::Forbidden("system collection".into()), 403),
        (E::CollectionNotFound("nowhere".into()), 404),
        (
            E::DocumentNotFound {
                collection: "notes".into(),
                key: "zz".into(),
            },
            404,
        ),
        (E::Conflict("notes/a".into()), 409),
        (
            E::UniqueViolation {
                collection: "notes".into(),
                index: "by_n".into(),
                existing: "a".into(),
            },
            409,
        ),
        (E::RowBudgetExceeded(1), 422),
        (E::IntermediateRowBudgetExceeded(1), 422),
        (E::MemoryBudgetExceeded(1), 422),
        (E::TimeBudgetExceeded, 422),
        // Failures that are not the caller's stay server errors.
        (E::Backend("disk".into()), 500),
        (E::MutationUnsupported, 500),
        (E::Connection("down".into()), 503),
    ];
    for (error, expected) in cases {
        let label = format!("{error:?}");
        let (status, body) = parts(error).await;
        assert_eq!(status.as_u16(), expected, "{label}: {body}");
        assert!(body["error"].is_string(), "{label}");
    }

    // The time limit comes from the budget the query ran under.
    let (_, body) = parts(E::TimeBudgetExceeded).await;
    assert_eq!(body["limit"], 30_000);
    assert_eq!(
        body["error"],
        "Query budget exceeded: query exceeded its time budget (30000 ms)"
    );
    // A missing collection keeps its name and the unique-violation code.
    let (_, body) = parts(E::CollectionNotFound("nowhere".into())).await;
    assert_eq!(body["error"], "Collection not found: nowhere");
    let (_, body) = parts(E::UniqueViolation {
        collection: "notes".into(),
        index: "by_n".into(),
        existing: "a".into(),
    })
    .await;
    assert_eq!(body["code"], "unique_violation");
    // A server-side failure keeps its old shape.
    let (_, body) = parts(E::Backend("disk".into())).await;
    assert_eq!(
        body["error"],
        "Query execution error: backend execution error: disk"
    );
}

#[tokio::test]
async fn what_the_query_names_but_does_not_exist_answers_404() {
    let state = state(unlimited()).await;
    for route in ROUTES {
        for prefix in ["", "EXPLAIN ANALYZE "] {
            let text = format!("{prefix}FOR d IN nowhere RETURN d");
            let (status, body, _) = query(&state, route, &text).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{route} {text}: {body}");
            assert_eq!(body["error"], "Collection not found: nowhere", "{route}");
        }
        // A collation the engine does not know is the caller's input.
        let (status, body, _) = query(
            &state,
            route,
            r#"FOR d IN notes SORT d._key COLLATE "not a locale" RETURN d._key"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{route}: {body}");
    }
    let (status, body, _) = query(
        &state,
        "/api/query",
        r#"UPSERT "a" INSERT { n: 1 } UPDATE { n: 2 } IN notes"#,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // Mutations that name a missing document or collection.
    for text in [
        r#"UPDATE "zz" WITH { n: 2 } IN notes"#,
        r#"REPLACE "zz" WITH { n: 2 } IN notes"#,
        r#"UPDATE "a" WITH { n: 2 } IN nowhere"#,
    ] {
        let (status, body, _) = query(&state, "/api/query", text).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{text}: {body}");
    }
    // Nothing was created on the way.
    assert!(
        state
            .backend
            .get_document("notes", "zz")
            .await
            .unwrap()
            .is_none()
    );
}
