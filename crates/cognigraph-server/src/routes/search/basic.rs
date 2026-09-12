use axum::extract::State;
use axum::{Extension, Json};
use serde::Deserialize;
use std::collections::HashMap;

use cognigraph_auth::User;
use cognigraph_core::{CogniGraphError, VectorSearchOpts};

use crate::error::AppError;
use crate::state::AppState;

use super::{default_embeddings_collection, default_limit, default_threshold};

// ---------------------------------------------------------------------------
// Vector search (pre-computed embeddings)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(super) struct VectorSearchRequest {
    #[serde(default = "default_embeddings_collection")]
    collection: String,
    vector: Vec<f64>,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    model_name: Option<String>,
}

pub(super) async fn vector_search(
    State(state): State<AppState>,
    Json(req): Json<VectorSearchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let opts = VectorSearchOpts {
        threshold: Some(req.threshold),
        limit: req.limit,
        model_name: req.model_name,
    };

    let hits = state
        .backend
        .vector_search(&req.collection, &req.vector, &opts)
        .await?;

    Ok(Json(serde_json::json!({
        "results": hits,
        "count": hits.len(),
    })))
}

// ---------------------------------------------------------------------------
// Full-text search (BM25 over string fields; no embedder required)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(super) struct TextSearchRequest {
    collection: String,
    query: String,
    #[serde(default = "default_text_fields")]
    fields: Vec<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_text_fields() -> Vec<String> {
    vec!["content".into(), "title".into()]
}

/// Plain BM25 text search — the embedder-free retrieval mode. Backends
/// without full-text support answer with their capability-gap error.
pub(super) async fn text_search(
    State(state): State<AppState>,
    Json(req): Json<TextSearchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let hits = state
        .backend
        .text_search(&req.collection, &req.query, &req.fields, req.limit)
        .await?;

    let results: Vec<serde_json::Value> = hits
        .iter()
        .map(|hit| {
            serde_json::json!({
                // Deliberately the document's own _id, not result_doc_id():
                // documents may carry an application-level `document_id`
                // field (e.g. "dailymed:…") that must not shadow the
                // collection/key address the browser links by.
                "document_id": hit.document.get("_id").cloned().unwrap_or_default(),
                "score": hit.score,
                "document": hit.document,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "results": results,
        "count": results.len(),
    })))
}

// ---------------------------------------------------------------------------
// Raw AQL query
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(super) struct RawQueryRequest {
    query: String,
    #[serde(default)]
    bind_vars: HashMap<String, serde_json::Value>,
    #[serde(default)]
    language: Option<String>,
}

pub(super) async fn raw_query(
    State(state): State<AppState>,
    _user: Option<Extension<User>>,
    Json(req): Json<RawQueryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if req
        .language
        .as_deref()
        .is_some_and(|language| !language.eq_ignore_ascii_case("cgql"))
    {
        return Err(AppError(CogniGraphError::Forbidden(
            "public opaque backend-native queries are disabled; use parsed read-only CGQL".into(),
        )));
    }
    // Public query text is always parsed CGQL. Server-authored backend-native
    // queries remain internal; a textual AQL denylist is defense-in-depth, not
    // a security boundary (quoted identifiers can encode Unicode escapes).
    crate::system_collections::deny_system_collections_in_cgql(&req.query).map_err(AppError)?;
    let results = cognigraph_query::parse_and_execute_backend_with_options(
        &req.query,
        state.backend.as_ref(),
        &req.bind_vars,
        cognigraph_query::QueryMode::ReadOnly,
        state.cgql_budget,
    )
    .await
    .map_err(cgql_error)?;

    Ok(Json(serde_json::json!({
        "results": results,
        "count": results.len(),
    })))
}

fn cgql_error(error: cognigraph_query::ExecutionError) -> AppError {
    match error {
        cognigraph_query::ExecutionError::Plan(error) => {
            AppError(CogniGraphError::ValidationError(error.to_string()))
        }
        cognigraph_query::ExecutionError::Forbidden(message) => {
            AppError(CogniGraphError::Forbidden(message))
        }
        cognigraph_query::ExecutionError::Connection(message) => {
            AppError(CogniGraphError::ConnectionError(message))
        }
        other => AppError(CogniGraphError::QueryError(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use cognigraph_auth::Role;
    use cognigraph_core::contract::NoAccessBackend;
    use cognigraph_native::NativeBackend;

    fn forbidden(err: AppError) {
        assert!(matches!(err.0, CogniGraphError::Forbidden(_)), "{err:?}");
    }

    fn user(role: Role) -> Extension<User> {
        Extension(User {
            key: format!("{role:?}"),
            username: format!("{role:?}"),
            role,
            tenant: "default".into(),
        })
    }

    /// Plain text search finds documents by word match and addresses hits
    /// by their collection/key _id, even when the document carries its own
    /// application-level `document_id` field.
    #[tokio::test]
    async fn text_search_matches_and_addresses_by_id() {
        let state = AppState::new(NativeBackend::new());
        state
            .backend
            .ensure_collection("labels", cognigraph_core::CollectionType::Document)
            .await
            .unwrap();
        let id = state
            .backend
            .create_document(
                "labels",
                serde_json::json!({
                    "title": "Hydrocortisone Cream",
                    "document_id": "dailymed:abc",
                }),
            )
            .await
            .unwrap();
        state
            .backend
            .create_document("labels", serde_json::json!({"title": "Insulin Pen"}))
            .await
            .unwrap();

        let response = text_search(
            State(state),
            Json(TextSearchRequest {
                collection: "labels".into(),
                query: "hydrocortisone".into(),
                fields: vec!["title".into()],
                limit: 10,
            }),
        )
        .await
        .unwrap();

        assert_eq!(response.0["count"], 1, "{}", response.0);
        assert_eq!(response.0["results"][0]["document_id"], id.full_id());
    }

    /// The only public query-text branch is parsed CGQL, which must reject
    /// system collections (decision_system_collections.md).
    #[tokio::test]
    async fn system_collections_answer_forbidden() {
        let state = AppState::new(NativeBackend::new());

        forbidden(
            vector_search(
                State(state.clone()),
                Json(VectorSearchRequest {
                    collection: "_users".into(),
                    vector: vec![0.0],
                    threshold: 0.0,
                    limit: 5,
                    model_name: None,
                }),
            )
            .await
            .unwrap_err(),
        );
        forbidden(
            text_search(
                State(state.clone()),
                Json(TextSearchRequest {
                    collection: "_users".into(),
                    query: "secret".into(),
                    fields: default_text_fields(),
                    limit: 5,
                }),
            )
            .await
            .unwrap_err(),
        );
        for language in [Some("cgql".to_string()), None] {
            forbidden(
                raw_query(
                    State(state.clone()),
                    None,
                    Json(RawQueryRequest {
                        query: "FOR u IN _users RETURN u".into(),
                        bind_vars: HashMap::new(),
                        language,
                    }),
                )
                .await
                .unwrap_err(),
            );
        }
    }

    #[tokio::test]
    async fn raw_backend_queries_are_disabled_even_for_admin() {
        let backend = std::sync::Arc::new(NoAccessBackend::default());
        let state = AppState::new_shared(backend.clone());

        for role in [Role::Viewer, Role::ScriptRunner, Role::Editor, Role::Admin] {
            let err = raw_query(
                State(state.clone()),
                Some(user(role)),
                Json(RawQueryRequest {
                    query: "REMOVE 'victim' IN documents".into(),
                    bind_vars: HashMap::new(),
                    language: Some("aql".into()),
                }),
            )
            .await
            .unwrap_err();
            forbidden(err);
        }

        forbidden(
            raw_query(
                State(state.clone()),
                Some(user(Role::Admin)),
                Json(RawQueryRequest {
                    query: r#"FOR d IN `_cognigraph_\u006aobs` RETURN d"#.into(),
                    bind_vars: HashMap::new(),
                    language: Some("aql".into()),
                }),
            )
            .await
            .unwrap_err(),
        );

        // Viewers retain a safe query path: explicit CGQL is parsed and
        // executed in read-only mode even when the backend reports an opaque language.
        let response = raw_query(
            State(state),
            Some(user(Role::Viewer)),
            Json(RawQueryRequest {
                query: "RETURN 1".into(),
                bind_vars: HashMap::new(),
                language: Some("cgql".into()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(response.0["results"], serde_json::json!([1.0]));
        backend.assert_unused();
    }
}
