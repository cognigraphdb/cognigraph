//! Read-write CGQL endpoint (POST /api/query).
//!
//! Gated by COGNIGRAPH_CGQL_MUTATIONS_ENABLED and `documents:write` RBAC.
//! `/search/query` remains parsed read-only CGQL; public opaque backend-native
//! passthrough is disabled for every role.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use std::collections::HashMap;

use cognigraph_core::CogniGraphError;
use cognigraph_query::{QueryMode, parse_and_execute_backend_with_options};

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(read_write_query))
}

#[derive(Deserialize)]
struct QueryRequest {
    query: String,
    #[serde(default)]
    bind_vars: HashMap<String, serde_json::Value>,
    /// Legacy per-collection cache hint. Accepted for wire compatibility;
    /// dependency-safe invalidation now clears every result entry.
    #[serde(default)]
    invalidate: Vec<String>,
}

async fn read_write_query(
    State(state): State<AppState>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // System collections answer 403 up front; the executor-level guard
    // would only surface as an opaque QueryError.
    crate::system_collections::deny_system_collections_in_cgql(&req.query).map_err(AppError)?;
    let execution = parse_and_execute_backend_with_options(
        &req.query,
        state.backend.as_ref(),
        &req.bind_vars,
        QueryMode::ReadWrite,
        state.cgql_budget,
    )
    .await;

    // The read-write executor may already have applied earlier statements
    // when a later statement fails, so the coherence barrier must run before
    // propagating its result.
    let _ = req.invalidate;
    state.invalidate_search_results().await;
    let results = execution.map_err(|error| match error {
        cognigraph_query::ExecutionError::Plan(error) => {
            AppError(CogniGraphError::ValidationError(error.to_string()))
        }
        cognigraph_query::ExecutionError::Forbidden(message) => {
            AppError(CogniGraphError::Forbidden(message))
        }
        cognigraph_query::ExecutionError::Connection(message) => {
            AppError(CogniGraphError::ConnectionError(message))
        }
        cognigraph_query::ExecutionError::Conflict(message) => {
            AppError(CogniGraphError::DocumentConflict(message))
        }
        cognigraph_query::ExecutionError::UniqueViolation {
            collection,
            index,
            existing,
        } => AppError(CogniGraphError::UniqueViolation {
            collection,
            index,
            existing,
        }),
        other => AppError(CogniGraphError::QueryError(other.to_string())),
    })?;

    Ok(Json(serde_json::json!({
        "results": results,
        "count": results.len(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_native::NativeBackend;

    #[tokio::test]
    async fn invalid_queries_return_bad_request_before_writing() {
        use axum::{http::StatusCode, response::IntoResponse};
        use serde_json::json;

        let state = AppState::new(NativeBackend::new());
        state
            .backend
            .create_document("notes", json!({"_key":"a","v":1}))
            .await
            .unwrap();
        let before = state
            .backend
            .list_documents("notes", None, None)
            .await
            .unwrap();
        for query in [
            r#"UPDATE "a" WITH {v:DOCUMENT("notes/a").v} IN notes"#,
            r#"UPDATE "a" WITH {v:2} IN notes RETURN DOCUMENT(NEW._id)"#,
            r#"FOR d IN notes LET refs=(FOR v IN 1..1 OUTBOUND d._id links RETURN v) UPDATE d._key WITH {refs:refs} IN notes"#,
            r#"UPDATE "a" WITH {v:unknown} IN notes"#,
            "UPDATE",
        ] {
            let err = read_write_query(
                State(state.clone()),
                Json(QueryRequest {
                    query: query.into(),
                    bind_vars: HashMap::new(),
                    invalidate: vec![],
                }),
            )
            .await
            .unwrap_err();
            assert!(
                matches!(err.0, CogniGraphError::ValidationError(_)),
                "{query}: {err:?}"
            );
            assert_eq!(
                err.into_response().status(),
                StatusCode::BAD_REQUEST,
                "{query}"
            );
            assert_eq!(
                state
                    .backend
                    .list_documents("notes", None, None)
                    .await
                    .unwrap(),
                before
            );
        }
    }

    /// The read-write CGQL endpoint must reject reads AND mutations that
    /// name a system collection — `FOR u IN _users UPDATE …` is the
    /// privilege-escalation path (decision_system_collections.md).
    #[tokio::test]
    async fn system_collections_answer_forbidden() {
        let state = crate::state::AppState::new(NativeBackend::new());
        for query in [
            "FOR u IN _users RETURN u",
            "INSERT { user: \"u1\", hash: \"h\" } INTO _tokens",
            "FOR u IN _users UPDATE u._key WITH { role: \"Admin\" } IN _users",
            "LET t = (FOR t IN _tokens RETURN t) RETURN t",
        ] {
            let err = read_write_query(
                State(state.clone()),
                Json(QueryRequest {
                    query: query.into(),
                    bind_vars: HashMap::new(),
                    invalidate: vec![],
                }),
            )
            .await
            .unwrap_err();
            assert!(
                matches!(err.0, CogniGraphError::Forbidden(_)),
                "query `{query}` gave {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn managed_collection_reads_pass_but_mutations_are_forbidden() {
        let state = crate::state::AppState::new(NativeBackend::new());
        state
            .managed_backend
            .create_document(
                "neurons",
                serde_json::json!({"_key": "n1", "status": "proposed"}),
            )
            .await
            .unwrap();

        let rows = read_write_query(
            State(state.clone()),
            Json(QueryRequest {
                query: "FOR n IN neurons RETURN n.status".into(),
                bind_vars: HashMap::new(),
                invalidate: vec![],
            }),
        )
        .await
        .unwrap();
        assert_eq!(rows.0["results"], serde_json::json!(["proposed"]));

        let error = read_write_query(
            State(state),
            Json(QueryRequest {
                query: "UPDATE \"n1\" WITH { status: \"accepted\" } IN neurons".into(),
                bind_vars: HashMap::new(),
                invalidate: vec![],
            }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error.0, CogniGraphError::Forbidden(_)),
            "{error:?}"
        );
    }
}
