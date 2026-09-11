use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use cognigraph_cache::{CacheHit, CacheKey, SearchMode, cache_weight, normalize_query};
use cognigraph_core::{QueryLanguage, VectorSearchOpts};

use crate::error::AppError;
use crate::state::AppState;

use super::{
    default_embeddings_collection, default_limit, default_threshold, embed_query_cached,
    require_embedder,
};

// ---------------------------------------------------------------------------
// Hybrid search (BM25 via ArangoSearch + vector search, fused via RRF)
// ---------------------------------------------------------------------------

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct HybridSearchRequest {
    // Query text has its own normalized cache-key component and is the only
    // input eligible for similarity matching. Every other field participates
    // in the structured parameter identity, including future request fields.
    #[serde(skip_serializing)]
    query: String,
    /// Collection whose text fields are searched on backends with native
    /// full-text support (non-AQL). Arango uses `search_view` instead.
    #[serde(default = "default_documents_collection")]
    documents_collection: String,
    #[serde(default = "default_embeddings_collection")]
    embeddings_collection: String,
    #[serde(default = "default_search_view")]
    search_view: String,
    #[serde(default = "default_search_fields")]
    search_fields: Vec<String>,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_rrf_k")]
    rrf_k: f64,
    #[serde(default = "default_weight")]
    bm25_weight: f64,
    #[serde(default = "default_weight")]
    vector_weight: f64,
    /// Fold generated side-view (context-expansion Q&A) vectors in as an extra
    /// retrieval leg. Each side-view carries a `document_id` pointer to its
    /// source document, so a side-view match boosts the SOURCE document via the
    /// same RRF parent-collapse the chunk-embedding convention already uses.
    /// Opt-in: the `side_views` collection only exists once generation has run,
    /// and a missing collection would otherwise error rather than no-op.
    #[serde(default)]
    include_side_views: bool,
    #[serde(default = "default_side_views_collection")]
    side_views_collection: String,
    #[serde(default = "default_weight")]
    side_view_weight: f64,
}

impl HybridSearchRequest {
    fn cache_key(&self) -> cognigraph_core::Result<CacheKey> {
        Ok(CacheKey {
            collection: self.embeddings_collection.clone(),
            search_mode: SearchMode::Hybrid,
            normalized_query: normalize_query(&self.query),
            // Preserve string boundaries and ordered arrays. A versioned
            // representation cannot match the former delimiter-based keys.
            params: serde_json::to_string(&("hybrid-v2", self))?,
        })
    }
}

fn default_documents_collection() -> String {
    "documents".into()
}
fn default_search_view() -> String {
    "documents_view".into()
}
fn default_search_fields() -> Vec<String> {
    vec!["content".into(), "title".into()]
}
fn default_rrf_k() -> f64 {
    60.0
}
fn default_weight() -> f64 {
    0.5
}
fn default_side_views_collection() -> String {
    crate::system_collections::SIDE_VIEWS_COLLECTION.into()
}

pub(super) async fn hybrid_search(
    State(state): State<AppState>,
    Json(req): Json<HybridSearchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    if state.backend.query_language() == QueryLanguage::Cgql {
        cognigraph_native::validate_text_search_fields(&req.search_fields)?;
    }
    if req.search_fields.is_empty() {
        return Err(AppError(cognigraph_core::CogniGraphError::ValidationError(
            "search_fields must contain at least one field".into(),
        )));
    }
    if req.limit == 0 {
        return Ok(Json(serde_json::json!({
            "results": [],
            "count": 0,
        })));
    }
    let embedder = require_embedder(&state)?;
    let cache = state.cache.as_deref();
    let result_generation = cache.map(cognigraph_cache::QueryCache::result_generation);

    let cache_key = req.cache_key()?;

    let query_vector = embed_query_cached(embedder, cache, &req.query).await?;

    // A strong hit is the only cache-only fast path. We retain weaker hits as
    // a weighted third retrieval leg and still execute both fresh legs below.
    let mut assisted_cache: Option<(CacheHit, f64)> = None;
    if let Some(c) = cache
        && let Some(cache_hit) = c.get_results(&cache_key, Some(&query_vector)).await
    {
        let config = c.config();

        if cache_hit.similarity >= config.strong_threshold {
            let results: Vec<_> = cache_hit
                .results
                .iter()
                .map(|cached| cached.document.clone())
                .collect();
            return Ok(Json(serde_json::json!({
                "count": results.len(),
                "results": results,
                "cached": true,
                "cache_similarity": cache_hit.similarity,
            })));
        }
        let weight = cache_weight(cache_hit.similarity, config.similarity_floor);
        assisted_cache = Some((cache_hit, weight));
    }

    let fetch_limit = req.limit * 3;

    // Full-text leg: AQL backends go through ArangoSearch views; other
    // backends use the GraphBackend text_search capability. When neither
    // works the response says so explicitly instead of degrading silently.
    let mut bm25_skipped: Option<String> = None;
    let bm25_results = if state.backend.query_language() == QueryLanguage::Aql {
        let bm25_filter = arango_bm25_filter(req.search_fields.len());

        let bm25_aql = format!(
            "FOR doc IN @@view SEARCH {bm25_filter} \
             SORT BM25(doc) DESC \
             LIMIT @limit \
             RETURN {{ _id: doc._id, _key: doc._key, score: BM25(doc), doc: doc }}"
        );

        let mut bm25_vars = HashMap::new();
        bm25_vars.insert("@view".to_string(), serde_json::json!(req.search_view));
        bm25_vars.insert("query".to_string(), serde_json::json!(req.query));
        bm25_vars.insert("limit".to_string(), serde_json::json!(fetch_limit));
        for (index, field) in req.search_fields.iter().enumerate() {
            bm25_vars.insert(format!("field_{index}"), serde_json::json!(field));
        }

        state.backend.query(&bm25_aql, bm25_vars).await?
    } else {
        match state
            .backend
            .text_search(
                &req.documents_collection,
                &req.query,
                &req.search_fields,
                fetch_limit,
            )
            .await
        {
            Ok(hits) => hits
                .into_iter()
                .map(|hit| {
                    serde_json::json!({
                        "_id": hit.document.get("_id").cloned().unwrap_or_default(),
                        "score": hit.score,
                        "doc": hit.document,
                    })
                })
                .collect(),
            Err(e) => {
                tracing::warn!(
                    backend = state.backend.backend_name(),
                    error = %e,
                    "hybrid search: full-text leg skipped"
                );
                bm25_skipped = Some(format!("skipped: {e}"));
                Vec::new()
            }
        }
    };

    // Vector search
    let vec_opts = VectorSearchOpts {
        threshold: Some(req.threshold),
        limit: fetch_limit,
        model_name: None,
    };
    let vec_results = state
        .backend
        .vector_search(&req.embeddings_collection, &query_vector, &vec_opts)
        .await?;

    // Side-view leg (opt-in): generated Q&A vectors, each pointing at its
    // source document via `document_id`. Best-effort and additive — a missing
    // side_views collection (none generated yet) contributes nothing rather than
    // failing the search, so the leg can be left on for collections that may or
    // may not have side-views.
    let mut side_views_skipped: Option<String> = None;
    let side_view_results = if req.include_side_views {
        match state
            .backend
            .vector_search(&req.side_views_collection, &query_vector, &vec_opts)
            .await
        {
            Ok(hits) => hits,
            Err(e) => {
                tracing::debug!(error = %e, "hybrid search: side-view leg skipped");
                side_views_skipped = Some(format!("skipped: {e}"));
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // RRF fusion
    let mut fused_scores: HashMap<String, (f64, Option<serde_json::Value>)> = HashMap::new();
    let mut fresh_ids = std::collections::HashSet::new();

    for (rank, item) in bm25_results.iter().enumerate() {
        let id = item["_id"].as_str().unwrap_or_default().to_string();
        if !id.is_empty() {
            fresh_ids.insert(id.clone());
            let rrf = req.bm25_weight / (req.rrf_k + rank as f64 + 1.0);
            let entry = fused_scores.entry(id).or_insert((0.0, None));
            entry.0 += rrf;
            entry.1 = item.get("doc").cloned();
        }
    }

    for (rank, hit) in vec_results.iter().enumerate() {
        if let Some(doc_id) = super::result_doc_id(&hit.document) {
            fresh_ids.insert(doc_id.to_string());
            let rrf = req.vector_weight / (req.rrf_k + rank as f64 + 1.0);
            let entry = fused_scores
                .entry(doc_id.to_string())
                .or_insert((0.0, None));
            entry.0 += rrf;
        }
    }

    // Side-view leg fuses into the SAME map keyed by resolved document_id: a
    // side-view row's document_id points at its source, so its RRF contribution
    // lands on the parent document's entry and the parent (never the synthetic
    // Q&A row) is what surfaces — the chunk→source collapse, reused for free.
    for (rank, hit) in side_view_results.iter().enumerate() {
        if let Some(doc_id) = super::result_doc_id(&hit.document) {
            fresh_ids.insert(doc_id.to_string());
            let rrf = req.side_view_weight / (req.rrf_k + rank as f64 + 1.0);
            let entry = fused_scores
                .entry(doc_id.to_string())
                .or_insert((0.0, None));
            entry.0 += rrf;
        }
    }

    // A weak cache hit is advisory, never an answer by itself. Add it as a
    // decayed RRF leg so fresh BM25/vector evidence remains primary.
    if let Some((cache_hit, weight)) = &assisted_cache
        && *weight > 0.0
    {
        for (rank, hit) in cache_hit.results.iter().enumerate() {
            if let Some(doc_id) = super::result_doc_id(&hit.document) {
                let rank_decay = 1.0 / (1.0 + rank as f64 * 0.15);
                let rrf = (weight * rank_decay) / (req.rrf_k + rank as f64 + 1.0);
                let entry = fused_scores
                    .entry(doc_id.to_string())
                    .or_insert((0.0, None));
                entry.0 += rrf;
            }
        }
    }

    let mut ranked: Vec<_> = fused_scores.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1.0
            .partial_cmp(&a.1.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut results = Vec::with_capacity(req.limit.min(ranked.len()));
    for (doc_id, (score, cached_doc)) in &ranked {
        let document = if cached_doc.is_some() {
            cached_doc.clone()
        } else if let Some((coll, key)) = doc_id.split_once('/') {
            state.backend.get_document(coll, key).await.ok().flatten()
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
        if results.len() == req.limit {
            break;
        }
    }

    // Store in cache
    if let (Some(c), Some(generation)) = (cache, result_generation) {
        let cache_hits: Vec<cognigraph_core::SearchHit> = results
            .iter()
            .map(|r| cognigraph_core::SearchHit {
                document: r.clone(),
                score: r["score"].as_f64().unwrap_or(0.0),
                source: Some("hybrid".into()),
            })
            .collect();
        c.put_results_if_generation(generation, &cache_key, Some(query_vector), cache_hits, None)
            .await;
    }

    let mut response = serde_json::json!({
        "results": results,
        "count": results.len(),
    });
    if let Some(reason) = bm25_skipped {
        response["bm25"] = serde_json::json!(reason);
    }
    if let Some(reason) = side_views_skipped {
        response["side_views"] = serde_json::json!(reason);
    }
    if let Some((cache_hit, weight)) = assisted_cache {
        response["cached"] = serde_json::json!("assisted");
        response["cache_similarity"] = serde_json::json!(cache_hit.similarity);
        response["cache_weight"] = serde_json::json!((weight * 1000.0).round() / 1000.0);
    }
    Ok(Json(response))
}

/// Build an AQL predicate using dynamic attribute bind variables. Field names
/// are request data and must never be interpolated into executable AQL.
fn arango_bm25_filter(field_count: usize) -> String {
    (0..field_count)
        .map(|index| format!("ANALYZER(PHRASE(doc[@field_{index}], @query), \"text_en\")"))
        .collect::<Vec<_>>()
        .join(" OR ")
}

#[cfg(test)]
#[path = "hybrid_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use crate::system_collections::SIDE_VIEWS_COLLECTION;
    use cognigraph_cache::{CacheConfig, InMemoryCache, QueryCache};
    use cognigraph_core::{GraphBackend, SearchHit};
    use cognigraph_native::NativeBackend;
    use serde_json::json;

    struct FixedEmbedder;

    #[async_trait::async_trait]
    impl cognigraph_embeddings::EmbeddingProvider for FixedEmbedder {
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>> {
            Ok(vec![vec![1.0, 0.0]; texts.len()])
        }
    }

    /// The hybrid vector leg over a system collection is refused
    /// (decision_system_collections.md). The text leg tolerates errors as
    /// capability gaps, so the vector leg is the one that must hard-fail.
    #[tokio::test]
    async fn system_collections_answer_forbidden() {
        let state = AppState::new(NativeBackend::new()).with_embedder(FixedEmbedder);
        let err = hybrid_search(
            State(state),
            Json(HybridSearchRequest {
                query: "who is the admin".into(),
                documents_collection: default_documents_collection(),
                embeddings_collection: "_users".into(),
                search_view: default_search_view(),
                search_fields: default_search_fields(),
                threshold: 0.0,
                limit: 5,
                rrf_k: default_rrf_k(),
                bm25_weight: 0.5,
                vector_weight: 0.5,
                include_side_views: false,
                side_views_collection: default_side_views_collection(),
                side_view_weight: 0.5,
            }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err.0, cognigraph_core::CogniGraphError::Forbidden(_)),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn weak_cache_hit_is_merged_with_fresh_hybrid_results() {
        let backend = NativeBackend::new();
        backend
            .create_document(
                "documents",
                json!({"_key": "cached", "title": "old", "content": "old result"}),
            )
            .await
            .unwrap();
        backend
            .create_document(
                "documents",
                json!({"_key": "fresh", "title": "new question", "content": "new question"}),
            )
            .await
            .unwrap();
        backend
            .create_document(
                "embeddings",
                json!({"_key": "fresh", "document_id": "documents/fresh", "embedding": [1.0, 0.0]}),
            )
            .await
            .unwrap();

        let cache = InMemoryCache::new(CacheConfig {
            enabled: true,
            ..Default::default()
        });
        let request = HybridSearchRequest {
            query: "new question".into(),
            documents_collection: "documents".into(),
            embeddings_collection: "embeddings".into(),
            search_view: default_search_view(),
            search_fields: default_search_fields(),
            threshold: 0.0,
            limit: 10,
            rrf_k: default_rrf_k(),
            bm25_weight: 0.5,
            vector_weight: 0.5,
            include_side_views: false,
            side_views_collection: default_side_views_collection(),
            side_view_weight: 0.5,
        };
        let mut old_key = request.cache_key().unwrap();
        old_key.normalized_query = normalize_query("old question");
        cache
            .put_results(
                &old_key,
                Some(vec![0.8, 0.6]),
                vec![
                    SearchHit {
                        document: json!({
                            "document_id": "documents/cached",
                            "score": 1.0,
                            "document": {"_id": "documents/cached"}
                        }),
                        score: 1.0,
                        source: Some("hybrid".into()),
                    },
                    SearchHit {
                        document: json!({"document_id": "documents/deleted", "score": 0.5}),
                        score: 0.5,
                        source: Some("hybrid".into()),
                    },
                ],
                None,
            )
            .await;

        let state = AppState::new(backend)
            .with_embedder(FixedEmbedder)
            .with_cache(cache);
        let response = hybrid_search(State(state.clone()), Json(request))
            .await
            .unwrap()
            .0;

        assert_eq!(response["cached"], "assisted", "{response}");
        assert!(
            response["results"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["document_id"] == "documents/fresh"),
            "fresh retrieval must run on a weak cache hit: {response}"
        );
        assert!(
            response["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["document_id"] != "documents/deleted"),
            "deleted cached-only documents must not re-enter results: {response}"
        );

        let repeat = hybrid_search(
            State(state),
            Json(HybridSearchRequest {
                query: "new question".into(),
                documents_collection: "documents".into(),
                embeddings_collection: "embeddings".into(),
                search_view: default_search_view(),
                search_fields: default_search_fields(),
                threshold: 0.0,
                limit: 10,
                rrf_k: default_rrf_k(),
                bm25_weight: 0.5,
                vector_weight: 0.5,
                include_side_views: false,
                side_views_collection: default_side_views_collection(),
                side_view_weight: 0.5,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(repeat["cached"], true, "{repeat}");
        assert!(
            repeat["results"][0].get("document_id").is_some(),
            "{repeat}"
        );
    }

    #[tokio::test]
    async fn below_floor_match_is_pure_fresh_not_assisted() {
        let backend = NativeBackend::new();
        backend
            .create_document(
                "documents",
                json!({"_key": "fresh", "title": "new question", "content": "new question"}),
            )
            .await
            .unwrap();
        backend
            .create_document(
                "embeddings",
                json!({"_key": "fresh", "document_id": "documents/fresh", "embedding": [1.0, 0.0]}),
            )
            .await
            .unwrap();
        let request = HybridSearchRequest {
            query: "new question".into(),
            documents_collection: "documents".into(),
            embeddings_collection: "embeddings".into(),
            search_view: default_search_view(),
            search_fields: default_search_fields(),
            threshold: 0.0,
            limit: 10,
            rrf_k: default_rrf_k(),
            bm25_weight: 0.5,
            vector_weight: 0.5,
            include_side_views: false,
            side_views_collection: default_side_views_collection(),
            side_view_weight: 0.5,
        };
        let mut old_key = request.cache_key().unwrap();
        old_key.normalized_query = normalize_query("old question");
        let cache = InMemoryCache::new(CacheConfig {
            enabled: true,
            similarity_floor: 0.8,
            ..Default::default()
        });
        cache
            .put_results(
                &old_key,
                Some(vec![0.7, 0.714]),
                vec![SearchHit {
                    document: json!({"document_id": "documents/old"}),
                    score: 1.0,
                    source: Some("hybrid".into()),
                }],
                None,
            )
            .await;
        let state = AppState::new(backend)
            .with_embedder(FixedEmbedder)
            .with_cache(cache);

        let response = hybrid_search(State(state.clone()), Json(request))
            .await
            .unwrap()
            .0;
        assert!(response.get("cached").is_none(), "{response}");
        assert!(
            response["results"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["document_id"] == "documents/fresh"),
            "{response}"
        );
        let stats = state.cache.as_deref().unwrap().stats_snapshot();
        assert_eq!(stats.hits_assisted, 0);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn arango_search_fields_are_always_bind_variables() {
        let attack =
            r#"title], @query), "text_en") RETURN DOCUMENT(CONCAT("_us","ers"), "admin") //"#;
        let query_fragment = arango_bm25_filter(1);
        assert!(!query_fragment.contains(attack));
        assert_eq!(
            query_fragment,
            "ANALYZER(PHRASE(doc[@field_0], @query), \"text_en\")"
        );
    }

    #[tokio::test]
    async fn side_view_hits_boost_their_parent_document() {
        // A side-view row carries a `document_id` pointer to its source. When
        // the query matches the generated question, the side-view leg surfaces
        // the PARENT document (not the synthetic Q&A row), and only when opted
        // in. The parent's own text is orthogonal to the query, so nothing but
        // the side-view leg can surface it.
        let backend = NativeBackend::new();
        backend
            .create_document(
                "documents",
                json!({"_key": "p1", "title": "unrelated", "content": "unrelated prose"}),
            )
            .await
            .unwrap();
        // The primary vector leg needs its collection to exist; keep its one
        // row orthogonal to the query so it contributes nothing.
        backend
            .create_document(
                "embeddings",
                json!({"_key": "e0", "document_id": "documents/other", "embedding": [0.0, 1.0]}),
            )
            .await
            .unwrap();
        // The side-view: question embedding (FixedEmbedder → [1,0]) matches the
        // query vector, pointer back to documents/p1. Written through the raw
        // handle, as the generation job would.
        backend
            .create_document(
                SIDE_VIEWS_COLLECTION,
                json!({
                    "_key": "sv1",
                    "document_id": "documents/p1",
                    "kind": "side_view",
                    "question": "what is the special topic",
                    "answer": "the special topic",
                    "embedding": [1.0, 0.0],
                }),
            )
            .await
            .unwrap();
        let state = AppState::new(backend).with_embedder(FixedEmbedder);

        let make_request = |include_side_views: bool| HybridSearchRequest {
            query: "special topic".into(),
            documents_collection: "documents".into(),
            embeddings_collection: "embeddings".into(),
            search_view: default_search_view(),
            search_fields: default_search_fields(),
            threshold: 0.0,
            limit: 10,
            rrf_k: default_rrf_k(),
            bm25_weight: 0.5,
            vector_weight: 0.5,
            include_side_views,
            side_views_collection: default_side_views_collection(),
            side_view_weight: 0.5,
        };

        // Opted out: the parent is never surfaced (its text does not match).
        let without = hybrid_search(State(state.clone()), Json(make_request(false)))
            .await
            .unwrap()
            .0;
        assert!(
            without["results"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["document_id"] != "documents/p1"),
            "without side-views the parent must not surface: {without}"
        );

        // Opted in: the side-view match boosts its PARENT document.
        let with = hybrid_search(State(state), Json(make_request(true)))
            .await
            .unwrap()
            .0;
        let parent = with["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["document_id"] == "documents/p1");
        assert!(
            parent.is_some(),
            "side-view must surface its parent: {with}"
        );
        assert_eq!(
            parent.unwrap()["document"]["_key"],
            json!("p1"),
            "the parent document is returned, not the synthetic Q&A row: {with}"
        );
    }
}
