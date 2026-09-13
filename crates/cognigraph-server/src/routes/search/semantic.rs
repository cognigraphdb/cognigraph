use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use cognigraph_cache::{CacheKey, QueryCache, SearchMode, cache_weight, normalize_query};
use cognigraph_core::{GraphBackend, VectorSearchOpts};

use crate::error::AppError;
use crate::state::AppState;

use super::{
    default_embeddings_collection, default_limit, default_threshold, embed_query_cached,
    require_embedder,
};

/// Cache-assisted retrieval for semantic search.
///
/// Merges cached document IDs with a fresh vector search using RRF.
/// The `weight` parameter (computed from `cache_weight()`) determines
/// the overall cache influence. Within cached results, each document's
/// contribution decays by its original rank position — top-ranked cached
/// docs influence more, tail docs fade out.
async fn cache_assisted_semantic(
    backend: &dyn GraphBackend,
    cached: &[cognigraph_core::SearchHit],
    weight: f64,
    query_vector: &[f64],
    collection: &str,
    opts: &VectorSearchOpts,
) -> Result<Vec<serde_json::Value>, AppError> {
    let fresh_hits = backend
        .vector_search(collection, query_vector, opts)
        .await?;

    const RRF_K: f64 = 60.0;

    let mut fused: HashMap<String, (f64, Option<serde_json::Value>)> = HashMap::new();
    let mut fresh_ids = std::collections::HashSet::new();

    // Score cached results with per-document rank decay.
    // Each cached doc gets: weight * rank_decay / (K + rank + 1)
    // where rank_decay = 1 / (1 + rank * 0.15) — top docs keep ~full
    // weight, tail docs fade to ~40% influence by rank 10.
    for (rank, hit) in cached.iter().enumerate() {
        let doc_id = super::result_doc_id(&hit.document)
            .unwrap_or_default()
            .to_string();
        if !doc_id.is_empty() {
            let rank_decay = 1.0 / (1.0 + rank as f64 * 0.15);
            let rrf = (weight * rank_decay) / (RRF_K + rank as f64 + 1.0);
            let entry = fused.entry(doc_id).or_insert((0.0, None));
            entry.0 += rrf;
        }
    }

    // Score fresh results at full strength (always the primary signal)
    for (rank, hit) in fresh_hits.iter().enumerate() {
        let doc_id = super::result_doc_id(&hit.document)
            .unwrap_or_default()
            .to_string();
        if !doc_id.is_empty() {
            fresh_ids.insert(doc_id.clone());
            let rrf = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = fused.entry(doc_id).or_insert((0.0, None));
            entry.0 += rrf;
        }
    }

    // Sort by fused score
    let mut ranked: Vec<_> = fused.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1.0
            .partial_cmp(&a.1.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if opts.limit == 0 {
        return Ok(Vec::new());
    }
    // Fetch fresh documents
    let mut results = Vec::with_capacity(opts.limit.min(ranked.len()));
    for (doc_id, (score, _)) in &ranked {
        let document = if let Some((coll, key)) = doc_id.split_once('/') {
            backend.get_document(coll, key).await.ok().flatten()
        } else {
            None
        };
        if !fresh_ids.contains(doc_id) && document.is_none() {
            continue;
        }

        results.push(serde_json::json!({
            "document_id": doc_id,
            "score": score,
            "document": document,
        }));
        if results.len() == opts.limit {
            break;
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Semantic search (text → embedding → vector search → fetch documents)
// ---------------------------------------------------------------------------

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct SemanticSearchRequest {
    // Query text is separately normalized; every other field belongs to the
    // exact parameter identity, including optional values and future fields.
    #[serde(skip_serializing)]
    query: String,
    #[serde(default = "default_embeddings_collection")]
    collection: String,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    model_name: Option<String>,
}

impl SemanticSearchRequest {
    fn cache_key(&self) -> cognigraph_core::Result<CacheKey> {
        Ok(CacheKey {
            collection: self.collection.clone(),
            search_mode: SearchMode::Semantic,
            normalized_query: normalize_query(&self.query),
            params: serde_json::to_string(&("semantic-v2", self))?,
        })
    }
}

pub(super) async fn semantic_search(
    State(state): State<AppState>,
    Json(req): Json<SemanticSearchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let embedder = require_embedder(&state)?;
    let cache = state.cache.as_deref();
    let result_generation = cache.map(QueryCache::result_generation);

    let cache_key = req.cache_key()?;

    let query_vector = embed_query_cached(embedder, cache, &req.query).await?;

    let opts = VectorSearchOpts {
        threshold: Some(req.threshold),
        limit: req.limit,
        model_name: req.model_name,
    };

    // Cache lookup — always attempted, cache is global memory
    let cache_hit = if let Some(c) = cache {
        c.get_results(&cache_key, Some(&query_vector)).await
    } else {
        None
    };

    // Fast path: very high similarity, return cached results directly
    if let Some(ref hit) = cache_hit
        && let Some(c) = cache
    {
        let config = c.config();
        if hit.similarity >= config.strong_threshold {
            let results: Vec<_> = hit
                .results
                .iter()
                .map(|cached| cached.document.clone())
                .collect();
            return Ok(Json(serde_json::json!({
                "count": results.len(),
                "results": results,
                "cached": true,
                "cache_similarity": hit.similarity,
            })));
        }
    }

    // Compute cache weight (0.0 if no hit, continuous curve otherwise)
    let (weight, similarity) = if let Some(ref hit) = cache_hit {
        let floor = cache.map(|c| c.config().similarity_floor).unwrap_or(0.7);
        (cache_weight(hit.similarity, floor), hit.similarity)
    } else {
        (0.0, 0.0)
    };

    // Always run fresh search. If cache has any signal, merge it in.
    if weight > 0.0 {
        let results = cache_assisted_semantic(
            &*state.backend,
            &cache_hit.as_ref().unwrap().results,
            weight,
            &query_vector,
            &req.collection,
            &opts,
        )
        .await?;

        // Store the merged results back into cache for future queries
        if let (Some(c), Some(generation)) = (cache, result_generation) {
            let cache_entries: Vec<cognigraph_core::SearchHit> = results
                .iter()
                .map(|r| cognigraph_core::SearchHit {
                    document: r.clone(),
                    score: r["score"].as_f64().unwrap_or(0.0),
                    source: Some("semantic".into()),
                })
                .collect();
            c.put_results_if_generation(
                generation,
                &cache_key,
                Some(query_vector),
                cache_entries,
                None,
            )
            .await;
        }

        return Ok(Json(serde_json::json!({
            "results": results,
            "count": results.len(),
            "cached": "assisted",
            "cache_similarity": similarity,
            "cache_weight": (weight * 1000.0).round() / 1000.0,
        })));
    }

    // Pure fresh search (no cache signal available)
    let hits = state
        .backend
        .vector_search(&req.collection, &query_vector, &opts)
        .await?;

    let mut results = Vec::with_capacity(hits.len());
    for hit in &hits {
        let doc_id = super::result_doc_id(&hit.document);

        let full_doc = if let Some(doc_id) = doc_id {
            if let Some((coll, key)) = doc_id.split_once('/') {
                state.backend.get_document(coll, key).await.ok().flatten()
            } else {
                None
            }
        } else {
            None
        };

        results.push(serde_json::json!({
            "score": hit.score,
            "document_id": doc_id,
            "document": full_doc,
        }));
    }

    // Store in cache for future queries
    if let (Some(c), Some(generation)) = (cache, result_generation) {
        let cache_hits: Vec<cognigraph_core::SearchHit> = results
            .iter()
            .map(|r| cognigraph_core::SearchHit {
                document: r.clone(),
                score: r["score"].as_f64().unwrap_or(0.0),
                source: Some("semantic".into()),
            })
            .collect();
        c.put_results_if_generation(generation, &cache_key, Some(query_vector), cache_hits, None)
            .await;
    }

    Ok(Json(serde_json::json!({
        "results": results,
        "count": results.len(),
    })))
}

#[cfg(test)]
#[path = "semantic_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use cognigraph_cache::{CacheConfig, InMemoryCache};
    use cognigraph_core::SearchHit;
    use cognigraph_native::NativeBackend;

    struct FixedEmbedder;

    #[async_trait::async_trait]
    impl cognigraph_embeddings::EmbeddingProvider for FixedEmbedder {
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>> {
            Ok(vec![vec![1.0, 0.0]; texts.len()])
        }
    }

    #[tokio::test]
    async fn assisted_semantic_skips_ghosts_without_consuming_the_limit() {
        let backend = NativeBackend::new();
        backend
            .create_document("documents", serde_json::json!({"_key": "fresh"}))
            .await
            .unwrap();
        backend
            .create_document("documents", serde_json::json!({"_key": "cached"}))
            .await
            .unwrap();
        backend
            .create_document(
                "embeddings",
                serde_json::json!({
                    "_key": "fresh",
                    "document_id": "documents/fresh",
                    "embedding": [1.0, 0.0]
                }),
            )
            .await
            .unwrap();
        let cached = vec![
            cognigraph_core::SearchHit {
                document: serde_json::json!({"document_id": "documents/deleted"}),
                score: 1.0,
                source: Some("semantic".into()),
            },
            cognigraph_core::SearchHit {
                document: serde_json::json!({"document_id": "documents/cached"}),
                score: 0.9,
                source: Some("semantic".into()),
            },
        ];
        let results = cache_assisted_semantic(
            &backend,
            &cached,
            2.0,
            &[1.0, 0.0],
            "embeddings",
            &VectorSearchOpts {
                threshold: Some(0.0),
                limit: 2,
                model_name: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(results.len(), 2, "{results:?}");
        assert!(
            results
                .iter()
                .all(|row| row["document_id"] != "documents/deleted"),
            "{results:?}"
        );
        assert!(
            results
                .iter()
                .any(|row| row["document_id"] == "documents/fresh"),
            "a lower-ranked valid candidate must fill the vacated slot: {results:?}"
        );
    }

    #[tokio::test]
    async fn below_floor_match_is_pure_fresh_not_assisted() {
        let backend = NativeBackend::new();
        backend
            .create_document(
                "embeddings",
                serde_json::json!({
                    "_key": "fresh",
                    "document_id": "documents/fresh",
                    "embedding": [1.0, 0.0]
                }),
            )
            .await
            .unwrap();
        backend
            .create_document("documents", serde_json::json!({"_key": "fresh"}))
            .await
            .unwrap();
        let cache = InMemoryCache::new(CacheConfig {
            enabled: true,
            similarity_floor: 0.8,
            ..Default::default()
        });
        let mut request = SemanticSearchRequest {
            query: "old query".into(),
            collection: "embeddings".into(),
            threshold: 0.0,
            limit: 10,
            model_name: None,
        };
        let cached_key = request.cache_key().unwrap();
        cache
            .put_results(
                &cached_key,
                Some(vec![0.7, 0.714]),
                vec![SearchHit {
                    document: serde_json::json!({"document_id": "documents/old"}),
                    score: 1.0,
                    source: Some("semantic".into()),
                }],
                None,
            )
            .await;
        let state = AppState::new(backend)
            .with_embedder(FixedEmbedder)
            .with_cache(cache);

        request.query = "new query".into();
        let response = semantic_search(State(state.clone()), Json(request))
            .await
            .unwrap()
            .0;
        assert!(response.get("cached").is_none(), "{response}");
        assert_eq!(response["results"][0]["document_id"], "documents/fresh");
        let stats = state.cache.as_deref().unwrap().stats_snapshot();
        assert_eq!(stats.hits_assisted, 0);
        assert_eq!(stats.misses, 1);
    }

    /// Regression: threshold/limit were once absent from the cache key, so a
    /// query warmed at a loose threshold answered the same query at a strict
    /// threshold with sub-threshold cached results. Different params must run
    /// fresh (and honor the threshold); identical params still hit the cache.
    #[tokio::test]
    async fn cache_never_crosses_request_params() {
        let backend = NativeBackend::new();
        let state = AppState::new(backend)
            .with_embedder(FixedEmbedder)
            .with_cache(cognigraph_cache::InMemoryCache::new(
                cognigraph_cache::CacheConfig {
                    enabled: true,
                    ..Default::default()
                },
            ));
        state
            .backend
            .ensure_collection("emb", cognigraph_core::CollectionType::Document)
            .await
            .unwrap();
        // FixedEmbedder returns [1.0, 0.0]: doc a scores 1.0, doc b scores 0.6.
        state
            .backend
            .create_document("emb", serde_json::json!({"embedding": [1.0, 0.0]}))
            .await
            .unwrap();
        state
            .backend
            .create_document("emb", serde_json::json!({"embedding": [0.6, 0.8]}))
            .await
            .unwrap();

        let request = |threshold: f64| SemanticSearchRequest {
            query: "same question".into(),
            collection: "emb".into(),
            threshold,
            limit: 10,
            model_name: None,
        };

        // Warm the cache at a loose threshold: both docs return.
        let warm = semantic_search(State(state.clone()), Json(request(0.0)))
            .await
            .unwrap();
        assert_eq!(warm.0["count"], 2);

        // Same query, strict threshold: must not serve the warm entry.
        let strict = semantic_search(State(state.clone()), Json(request(0.9)))
            .await
            .unwrap();
        assert_eq!(strict.0["count"], 1, "{}", strict.0);
        assert!(strict.0.get("cached").is_none(), "{}", strict.0);
        for hit in strict.0["results"].as_array().unwrap() {
            assert!(hit["score"].as_f64().unwrap() >= 0.9, "{}", strict.0);
        }

        // Identical params again: the fast path still serves the cache.
        let repeat = semantic_search(State(state), Json(request(0.0)))
            .await
            .unwrap();
        assert_eq!(repeat.0["count"], 2);
        assert_eq!(repeat.0["cached"], true, "{}", repeat.0);
        assert!(
            repeat.0["results"][0].get("document_id").is_some(),
            "cache fast path must preserve the fresh response row shape: {}",
            repeat.0
        );
    }

    /// Semantic search over a system collection is refused before any
    /// vector scan runs (decision_system_collections.md).
    #[tokio::test]
    async fn system_collections_answer_forbidden() {
        let state = AppState::new(NativeBackend::new()).with_embedder(FixedEmbedder);
        let err = semantic_search(
            State(state),
            Json(SemanticSearchRequest {
                query: "who is the admin".into(),
                collection: "_users".into(),
                threshold: 0.0,
                limit: 5,
                model_name: None,
            }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err.0, cognigraph_core::CogniGraphError::Forbidden(_)),
            "{err:?}"
        );
    }
}
