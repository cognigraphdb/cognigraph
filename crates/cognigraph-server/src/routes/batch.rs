//! Atomic batch writes (POST /api/batch): all operations apply or none do.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use cognigraph_core::BatchOp;

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(execute_batch))
}

#[derive(Deserialize)]
struct BatchRequest {
    ops: Vec<BatchOp>,
    /// Legacy per-collection cache hint. Accepted for wire compatibility;
    /// dependency-safe invalidation now clears every result entry.
    #[serde(default)]
    invalidate: Vec<String>,
}

async fn execute_batch(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let execution = state.backend.execute_batch(req.ops).await;
    // Batch operations name their target collections, but cached search rows
    // may be keyed by a different embeddings collection. Always flush results;
    // keep the legacy hint field accepted for wire compatibility.
    let _ = req.invalidate;
    state.invalidate_search_results().await;
    let results = execution?;
    Ok(Json(serde_json::json!({
        "results": results,
        "count": results.len(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_core::CogniGraphError;
    use cognigraph_native::NativeBackend;
    use serde_json::json;

    /// A batch naming a system collection is rejected whole — nothing
    /// applies, not even the ops on ordinary collections
    /// (decision_system_collections.md).
    #[tokio::test]
    async fn system_collections_answer_forbidden() {
        let state = AppState::new(NativeBackend::new());
        let err = execute_batch(
            State(state.clone()),
            Json(BatchRequest {
                ops: vec![
                    BatchOp::Insert {
                        collection: "notes".into(),
                        doc: json!({"_key": "ok"}),
                    },
                    BatchOp::Update {
                        collection: "_users".into(),
                        key: "admin".into(),
                        merge: json!({"role": "Admin"}),
                    },
                ],
                invalidate: vec![],
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(err.0, CogniGraphError::Forbidden(_)), "{err:?}");
        assert!(
            state
                .backend
                .get_document("notes", "ok")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn omitted_invalidation_hint_cannot_leave_results_stale() {
        use cognigraph_cache::{CacheConfig, CacheKey, InMemoryCache, QueryCache, SearchMode};
        use cognigraph_core::SearchHit;

        let cache = InMemoryCache::new(CacheConfig {
            enabled: true,
            ..Default::default()
        });
        let key = CacheKey {
            collection: "embeddings".into(),
            search_mode: SearchMode::Hybrid,
            normalized_query: "cached".into(),
            params: "test".into(),
        };
        cache
            .put_results(
                &key,
                Some(vec![1.0]),
                vec![SearchHit {
                    document: json!({"document_id": "documents/old"}),
                    score: 1.0,
                    source: Some("test".into()),
                }],
                None,
            )
            .await;
        let state = AppState::new(NativeBackend::new()).with_cache(cache);

        let _ = execute_batch(
            State(state.clone()),
            Json(BatchRequest {
                ops: vec![BatchOp::Insert {
                    collection: "documents".into(),
                    doc: json!({"_key": "new"}),
                }],
                invalidate: vec![],
            }),
        )
        .await
        .unwrap();

        assert!(
            state
                .cache
                .as_deref()
                .unwrap()
                .get_results(&key, None)
                .await
                .is_none()
        );
    }
}
