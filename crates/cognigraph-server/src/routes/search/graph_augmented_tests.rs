use super::*;
use cognigraph_cache::{CacheConfig, InMemoryCache, QueryCache};
use cognigraph_core::GraphBackend;
use cognigraph_core::SearchHit;
use cognigraph_native::NativeBackend;
use serde_json::json;

struct FixedEmbedder;

#[async_trait::async_trait]
impl cognigraph_embeddings::EmbeddingProvider for FixedEmbedder {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>> {
        Ok(vec![vec![1.0, 0.0]; texts.len()])
    }
}

pub(super) async fn seeded_state() -> AppState {
    let backend = NativeBackend::new();
    for (key, name) in [("a", "Alpha Corp"), ("b", "Beta Labs"), ("c", "Gamma")] {
        backend
            .create_document("entities", json!({"_key": key, "name": name}))
            .await
            .unwrap();
    }
    backend
        .create_document(
            "embeddings",
            json!({"_key": "s1", "document_id": "entities/a", "embedding": [1.0, 0.0]}),
        )
        .await
        .unwrap();
    for (from, to, relation, chunk) in [
        ("entities/a", "entities/b", "SUPPLIES", "c1"),
        ("entities/a", "entities/c", "MENTIONS", "c2"),
    ] {
        backend
            .upsert_edge(
                "document_relations",
                from,
                to,
                relation,
                json!({"evidence_chunk_id": chunk}),
            )
            .await
            .unwrap();
    }
    AppState::new(backend).with_embedder(FixedEmbedder)
}

pub(super) fn request() -> GraphAugmentedRequest {
    serde_json::from_value(json!({
        "query": "who supplies what",
        "collection": "embeddings",
        "threshold": 0.0,
    }))
    .unwrap()
}

async fn facts(state: &AppState) -> Vec<serde_json::Value> {
    let response = graph_augmented_search(State(state.clone()), Json(request()))
        .await
        .unwrap_or_else(|_| panic!("graph-augmented search failed"));
    response.0["graph_facts"].as_array().unwrap().clone()
}

#[tokio::test]
async fn graph_facts_use_base_ranking_and_enterprise_only_accepted_hints() {
    let state = seeded_state().await;

    // Base ranking: the configured relation beats MENTIONS.
    let base = facts(&state).await;
    assert_eq!(base.len(), 2);
    assert_eq!(base[0]["relation"], "SUPPLIES");
    assert_eq!(base[0]["fact"], "entities/a --SUPPLIES--> entities/b");

    // A PROPOSED rank hint is inert — governance boundary holds live.
    state
        .managed_backend
        .create_document(
            "neurons",
            json!({
                "_key": "boost-mentions",
                "id": "boost-mentions",
                "type": "relation_rank_hint",
                "status": "proposed",
                "confidence": 0.9,
                "evidence": ["trace review"],
                "relation": "MENTIONS",
                "boost": 200.0,
            }),
        )
        .await
        .unwrap();
    assert_eq!(facts(&state).await[0]["relation"], "SUPPLIES");

    // The test fixture models a completed governed transition through the
    // internal handle; the public backend rejects this direct update.
    state
        .managed_backend
        .update_document("neurons", "boost-mentions", json!({"status": "accepted"}))
        .await
        .unwrap();
    #[cfg(feature = "enterprise")]
    assert_eq!(facts(&state).await[0]["relation"], "MENTIONS");
    #[cfg(not(feature = "enterprise"))]
    assert_eq!(facts(&state).await[0]["relation"], "SUPPLIES");
}

/// Graph-augmented search over a system collection is refused before
/// the seed scan (decision_system_collections.md).
#[tokio::test]
async fn system_collections_answer_forbidden() {
    let state = seeded_state().await;
    let err = graph_augmented_search(
        State(state),
        Json(GraphAugmentedRequest {
            query: "who is the admin".into(),
            collection: "_users".into(),
            edge_collection: default_edge_collection(),
            threshold: 0.0,
            seed_limit: 5,
            limit: 5,
            max_depth: 2,
            direction: Direction::default(),
            min_confidence: None,
            path_decay: 0.8,
            #[cfg(feature = "enterprise")]
            neurons_collection: default_neurons_collection(),
            graph_facts_limit: 10,
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
async fn weak_cache_hit_runs_fresh_graph_pipeline() {
    let mut state = seeded_state().await;
    let req = request();
    let cache = InMemoryCache::new(CacheConfig {
        enabled: true,
        ..Default::default()
    });
    let mut old_key = req.cache_key().unwrap();
    old_key.normalized_query = normalize_query("old supplies question");
    cache
        .put_results(
            &old_key,
            Some(vec![0.8, 0.6]),
            vec![
                SearchHit {
                    document: json!({"document_id": "entities/c", "score": 1.0}),
                    score: 1.0,
                    source: Some("graph-augmented".into()),
                },
                SearchHit {
                    document: json!({"document_id": "entities/deleted", "score": 0.5}),
                    score: 0.5,
                    source: Some("graph-augmented".into()),
                },
            ],
            Some(json!([{"fact": "stale cached fact"}])),
        )
        .await;
    state.cache = Some(std::sync::Arc::new(cache));

    let response = graph_augmented_search(State(state.clone()), Json(req))
        .await
        .unwrap()
        .0;
    assert_eq!(response["cached"], "assisted", "{response}");
    assert!(
        response["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["document_id"] == "entities/a"),
        "fresh semantic seeds must still be present: {response}"
    );
    assert!(
        response["graph_facts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fact| fact["relation"] == "SUPPLIES"),
        "graph facts must come from the live traversal: {response}"
    );
    assert!(
        response["graph_facts"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fact| fact["fact"] != "stale cached fact"),
        "weak cache metadata must not replace live graph facts: {response}"
    );
    assert!(
        response["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["document_id"] != "entities/deleted"),
        "deleted cached-only documents must not re-enter results: {response}"
    );

    let repeat = graph_augmented_search(State(state), Json(request()))
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
    let req = request();
    let mut old_key = req.cache_key().unwrap();
    old_key.normalized_query = normalize_query("old unrelated question");
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
                document: json!({"document_id": "entities/old"}),
                score: 1.0,
                source: Some("graph-augmented".into()),
            }],
            None,
        )
        .await;
    let state = seeded_state().await.with_cache(cache);

    let response = graph_augmented_search(State(state.clone()), Json(req))
        .await
        .unwrap()
        .0;
    assert!(response.get("cached").is_none(), "{response}");
    assert!(
        response["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["document_id"] == "entities/a"),
        "{response}"
    );
    let stats = state.cache.as_deref().unwrap().stats_snapshot();
    assert_eq!(stats.hits_assisted, 0);
    assert_eq!(stats.misses, 1);
}
