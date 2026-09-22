use axum::Json;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use cognigraph_cache::{CacheHit, CacheKey, SearchMode, cache_weight, normalize_query};
#[cfg(feature = "enterprise")]
use cognigraph_construct::{Neuron, rank_boosts};
use cognigraph_core::graph_ranking::{EdgeSelectOpts, GraphEdge, select_graph_edges};
use cognigraph_core::{Direction, TraversalOpts, VectorSearchOpts};

use crate::error::AppError;
use crate::state::AppState;

use super::{
    default_embeddings_collection, default_limit, default_threshold, embed_query_cached,
    require_embedder,
};

// ---------------------------------------------------------------------------
// Graph-augmented search (semantic + multi-hop graph expansion)
// ---------------------------------------------------------------------------

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct GraphAugmentedRequest {
    // Query text is separately normalized; every other field belongs to the
    // exact parameter identity, including optional values and future fields.
    #[serde(skip_serializing)]
    query: String,
    #[serde(default = "default_embeddings_collection")]
    collection: String,
    #[serde(default = "default_edge_collection")]
    edge_collection: String,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_seed_limit")]
    seed_limit: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default = "default_max_depth")]
    max_depth: u32,
    #[serde(default)]
    direction: Direction,
    #[serde(default)]
    min_confidence: Option<f64>,
    #[serde(default = "default_path_decay")]
    path_decay: f64,
    /// Collection holding neuron documents (accepted `relation_rank_hint`
    /// neurons reweight the graph-facts ranking; everything else is inert).
    #[cfg(feature = "enterprise")]
    #[serde(default = "default_neurons_collection")]
    neurons_collection: String,
    #[serde(default = "default_graph_facts_limit")]
    graph_facts_limit: usize,
}

impl GraphAugmentedRequest {
    fn cache_key(&self) -> cognigraph_core::Result<CacheKey> {
        Ok(CacheKey {
            collection: self.collection.clone(),
            search_mode: SearchMode::GraphAugmented,
            normalized_query: normalize_query(&self.query),
            params: serde_json::to_string(&("graph-augmented-v2", self))?,
        })
    }
}

fn default_edge_collection() -> String {
    "document_relations".into()
}
fn default_seed_limit() -> usize {
    5
}
fn default_max_depth() -> u32 {
    2
}
fn default_path_decay() -> f64 {
    0.8
}
#[cfg(feature = "enterprise")]
fn default_neurons_collection() -> String {
    "neurons".into()
}
fn default_graph_facts_limit() -> usize {
    EdgeSelectOpts::default().limit
}

pub(super) async fn graph_augmented_search(
    State(state): State<AppState>,
    Json(req): Json<GraphAugmentedRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let embedder = require_embedder(&state)?;
    let cache = state.cache.as_deref();
    let result_generation = cache.map(cognigraph_cache::QueryCache::result_generation);

    let cache_key = req.cache_key()?;

    let query_vector = embed_query_cached(embedder, cache, &req.query).await?;

    // Strongly similar queries may use the cache-only fast path. Weaker hits
    // are retained as a weighted signal, but the graph is traversed afresh.
    let mut assisted_cache: Option<(CacheHit, f64)> = None;
    if let Some(c) = cache
        && let Some(cache_hit) = c.get_results(&cache_key, Some(&query_vector)).await
    {
        let config = c.config();

        let graph_facts = cache_hit
            .meta
            .clone()
            .unwrap_or_else(|| serde_json::json!([]));
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
                "graph_facts": graph_facts,
            })));
        }
        let weight = cache_weight(cache_hit.similarity, config.similarity_floor);
        assisted_cache = Some((cache_hit, weight));
    }

    // Full pipeline
    let opts = VectorSearchOpts {
        threshold: Some(req.threshold),
        limit: req.seed_limit,
        model_name: None,
    };

    let seeds = state
        .backend
        .vector_search(&req.collection, &query_vector, &opts)
        .await?;

    let traversal_opts = TraversalOpts {
        max_depth: req.max_depth,
        min_depth: 1,
        direction: req.direction,
        edge_collection: req.edge_collection.clone(),
        min_confidence: req.min_confidence,
        path_decay: req.path_decay,
    };

    let mut all_results: HashMap<String, serde_json::Value> = HashMap::new();
    let mut fact_edges: Vec<GraphEdge> = Vec::new();
    let mut seed_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for hit in &seeds {
        let doc_id = super::result_doc_id(&hit.document)
            .unwrap_or_default()
            .to_string();

        if doc_id.is_empty() {
            continue;
        }

        let full_doc = if let Some((coll, key)) = doc_id.split_once('/') {
            state.backend.get_document(coll, key).await.ok().flatten()
        } else {
            None
        };

        all_results.entry(doc_id.clone()).or_insert_with(|| {
            serde_json::json!({
                "document_id": doc_id,
                "score": hit.score,
                "source": "semantic",
                "document": full_doc,
            })
        });

        let paths = match state.backend.traverse(&doc_id, &traversal_opts).await {
            Ok(paths) => paths,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    seed = %doc_id,
                    "graph augmentation traversal failed; seed kept without expansion"
                );
                Vec::new()
            }
        };
        seed_ids.insert(doc_id.clone());
        for path in paths {
            fact_edges.extend(path.edges.iter().filter_map(GraphEdge::from_value));
            for vertex in &path.vertices {
                let vertex_id = vertex["_id"].as_str().unwrap_or_default().to_string();
                if vertex_id.is_empty() {
                    continue;
                }
                all_results.entry(vertex_id.clone()).or_insert_with(|| {
                    serde_json::json!({
                        "document_id": vertex_id,
                        "score": path.score * hit.score,
                        "source": "graph",
                        "depth": path.vertices.len() - 1,
                        "document": vertex,
                    })
                });
            }
        }
    }

    let mut fresh_results: Vec<_> = all_results.into_values().collect();
    fresh_results.sort_by(|a, b| {
        let sa = a["score"].as_f64().unwrap_or(0.0);
        let sb = b["score"].as_f64().unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });
    let results = merge_assisted_results(
        &*state.backend,
        fresh_results,
        assisted_cache.as_ref(),
        req.limit,
    )
    .await;

    // Graph facts: the traversed edges ranked for a retrieval trace —
    // configured relations beat MENTIONS beat other low-signal, reweighted
    // by ACCEPTED relation_rank_hint neurons (see the /neurons routes;
    // proposed/rejected hints are inert).
    let (graph_facts, warnings) = ranked_graph_facts(&state, &req, &fact_edges, &seed_ids).await;

    // Store in cache — graph_facts ride along as entry meta so cached
    // responses keep the trace.
    if let (Some(c), Some(generation)) = (cache, result_generation) {
        let cache_hits: Vec<cognigraph_core::SearchHit> = results
            .iter()
            .map(|r| cognigraph_core::SearchHit {
                document: r.clone(),
                score: r["score"].as_f64().unwrap_or(0.0),
                source: Some("graph-augmented".into()),
            })
            .collect();
        c.put_results_if_generation(
            generation,
            &cache_key,
            Some(query_vector),
            cache_hits,
            Some(serde_json::json!(graph_facts.clone())),
        )
        .await;
    }

    let mut response = serde_json::json!({
        "results": results,
        "count": results.len(),
        "seeds": seeds.len(),
        "graph_facts": graph_facts,
    });
    if let Some((cache_hit, weight)) = assisted_cache {
        response["cached"] = serde_json::json!("assisted");
        response["cache_similarity"] = serde_json::json!(cache_hit.similarity);
        response["cache_weight"] = serde_json::json!((weight * 1000.0).round() / 1000.0);
    }
    // Warnings describe the live neuron state and are never cached: a
    // strong cache hit returns without them (documented in the HTTP guide).
    if !warnings.is_empty() {
        response["warnings"] = serde_json::json!(warnings);
    }
    Ok(Json(response))
}

/// Fuse a weak cached ranking with the newly computed semantic+graph ranking.
/// Cached-only documents are re-fetched, and stale cached graph-fact metadata
/// is deliberately ignored: the caller has just traversed the live graph.
async fn merge_assisted_results(
    backend: &dyn cognigraph_core::GraphBackend,
    fresh: Vec<serde_json::Value>,
    assisted: Option<&(CacheHit, f64)>,
    limit: usize,
) -> Vec<serde_json::Value> {
    if limit == 0 {
        return Vec::new();
    }
    let Some((cache_hit, weight)) = assisted.filter(|(_, weight)| *weight > 0.0) else {
        return fresh.into_iter().take(limit).collect();
    };

    const RRF_K: f64 = 60.0;
    let mut fused: HashMap<String, (f64, Option<serde_json::Value>)> = HashMap::new();
    for (rank, row) in fresh.into_iter().enumerate() {
        if let Some(id) = super::result_doc_id(&row).map(str::to_string) {
            let score = 1.0 / (RRF_K + rank as f64 + 1.0);
            fused.insert(id, (score, Some(row)));
        }
    }
    for (rank, hit) in cache_hit.results.iter().enumerate() {
        if let Some(id) = super::result_doc_id(&hit.document) {
            let rank_decay = 1.0 / (1.0 + rank as f64 * 0.15);
            let score = (weight * rank_decay) / (RRF_K + rank as f64 + 1.0);
            fused.entry(id.to_string()).or_insert((0.0, None)).0 += score;
        }
    }

    let mut ranked: Vec<_> = fused.into_iter().collect();
    ranked.sort_by(|a, b| {
        b.1.0
            .partial_cmp(&a.1.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut results = Vec::with_capacity(limit.min(ranked.len()));
    for (id, (score, fresh_row)) in ranked {
        let mut row = if let Some(row) = fresh_row {
            row
        } else {
            let document = if let Some((collection, key)) = id.split_once('/') {
                backend.get_document(collection, key).await.ok().flatten()
            } else {
                None
            };
            let Some(document) = document else {
                continue;
            };
            serde_json::json!({
                "document_id": id,
                "source": "cache-assisted",
                "document": document,
            })
        };
        row["score"] = serde_json::json!(score);
        results.push(row);
        if results.len() == limit {
            break;
        }
    }
    results
}

/// The ranked retrieval trace plus response warnings (CG-89). Absent
/// collection or malformed neuron documents mean "no boosts, no warning",
/// never an error: ranking must not break search.
async fn ranked_graph_facts(
    state: &AppState,
    req: &GraphAugmentedRequest,
    fact_edges: &[GraphEdge],
    seed_ids: &std::collections::HashSet<String>,
) -> (Vec<serde_json::Value>, Vec<serde_json::Value>) {
    #[cfg(not(feature = "enterprise"))]
    let (boosts, warnings) = {
        let _ = state;
        (HashMap::new(), Vec::new())
    };
    #[cfg(feature = "enterprise")]
    let (boosts, warnings) = {
        let neurons: Vec<Neuron> = match state
            .backend
            .list_documents(&req.neurons_collection, None, None)
            .await
        {
            Ok(docs) => docs
                .into_iter()
                .filter_map(|doc| serde_json::from_value(doc).ok())
                .collect(),
            Err(_) => Vec::new(),
        };
        let warnings =
            super::graph_warnings::inert_rank_hint_warning(&neurons, &req.edge_collection)
                .into_iter()
                .collect();
        (rank_boosts(&neurons), warnings)
    };
    let opts = EdgeSelectOpts {
        limit: req.graph_facts_limit,
        ..EdgeSelectOpts::default()
    };
    let facts = select_graph_edges(fact_edges, seed_ids, &boosts, &opts)
        .into_iter()
        .map(|edge| {
            serde_json::json!({
                "source": edge.source,
                "relation": edge.relation,
                "target": edge.target,
                "evidence_chunk_id": edge.evidence_chunk_id,
                "fact": format!("{} --{}--> {}", edge.source, edge.relation, edge.target),
            })
        })
        .collect();
    (facts, warnings)
}

#[cfg(test)]
#[path = "graph_cache_tests.rs"]
mod cache_tests;

#[cfg(test)]
#[path = "graph_augmented_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "graph_augmented_warning_tests.rs"]
mod warning_tests;
