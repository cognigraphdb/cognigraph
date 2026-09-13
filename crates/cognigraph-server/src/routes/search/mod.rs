use axum::Router;
use axum::routing::post;

use cognigraph_cache::QueryCache;
use cognigraph_core::CogniGraphError;

use crate::error::AppError;
use crate::state::AppState;

mod basic;
mod graph_augmented;
mod hybrid;
mod semantic;

use basic::{raw_query, text_search, vector_search};
use graph_augmented::graph_augmented_search;
use hybrid::hybrid_search;
use semantic::semantic_search;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/vector", post(vector_search))
        .route("/text", post(text_search))
        .route("/query", post(raw_query))
        .route("/semantic", post(semantic_search))
        .route("/hybrid", post(hybrid_search))
        .route("/graph-augmented", post(graph_augmented_search))
}

// ---------------------------------------------------------------------------
// Shared request defaults
// ---------------------------------------------------------------------------

fn default_embeddings_collection() -> String {
    "embeddings".into()
}

/// The id a search hit's payload is fetched by: the `document_id` pointer
/// when the hit is an embeddings-collection chunk (the chunk→source
/// convention), else the hit's own `_id` — self-contained documents that
/// carry their embedding inline (e.g. stored via POST /api/documents/embed)
/// are their own payload. Also handles cached response rows, where the id
/// sits under `document._id`.
fn result_doc_id(value: &serde_json::Value) -> Option<&str> {
    value
        .get("document_id")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("_id").and_then(|v| v.as_str()))
        .or_else(|| value.pointer("/document/_id").and_then(|v| v.as_str()))
}
fn default_threshold() -> f64 {
    0.7
}
fn default_limit() -> usize {
    10
}

// ---------------------------------------------------------------------------
// Helper: require embedder or return error
// ---------------------------------------------------------------------------

fn require_embedder(
    state: &AppState,
) -> Result<&dyn cognigraph_embeddings::EmbeddingProvider, AppError> {
    state.embedder.as_deref().ok_or_else(|| {
        AppError(CogniGraphError::BackendError(
            "No embedding provider configured. Set COGNIGRAPH_EMBEDDING_PROVIDER env var.".into(),
        ))
    })
}

async fn embed_query(
    embedder: &dyn cognigraph_embeddings::EmbeddingProvider,
    query: &str,
) -> Result<Vec<f64>, AppError> {
    let vectors = embedder
        .embed(&[query])
        .await
        .map_err(|e| AppError(CogniGraphError::EmbeddingError(e.to_string())))?;

    vectors.into_iter().next().ok_or_else(|| {
        AppError(CogniGraphError::EmbeddingError(
            "Embedding provider returned no vectors".into(),
        ))
    })
}

/// Embed a query with cache support. Checks the cache first; on miss,
/// embeds via the provider and stores the result.
async fn embed_query_cached(
    embedder: &dyn cognigraph_embeddings::EmbeddingProvider,
    cache: Option<&dyn QueryCache>,
    query: &str,
) -> Result<Vec<f64>, AppError> {
    let model = embedder.model_name();

    if let Some(c) = cache
        && let Some(cached) = c.get_embedding(query, model).await
    {
        return Ok(cached);
    }

    let embedding = embed_query(embedder, query).await?;

    if let Some(c) = cache {
        c.put_embedding(query, model, embedding.clone()).await;
    }

    Ok(embedding)
}
