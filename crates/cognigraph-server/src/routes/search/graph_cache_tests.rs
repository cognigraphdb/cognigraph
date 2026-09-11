use super::*;
use cognigraph_cache::{CacheConfig, InMemoryCache, QueryCache};
use cognigraph_core::{GraphBackend, SearchHit};
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

fn request(overrides: Value) -> GraphAugmentedRequest {
    let mut wire = json!({"query": "needle", "graph_facts_limit": 10});
    wire.as_object_mut()
        .unwrap()
        .extend(overrides.as_object().unwrap().clone());
    serde_json::from_value(wire).unwrap()
}

fn cache() -> InMemoryCache {
    InMemoryCache::new(CacheConfig {
        enabled: true,
        ..Default::default()
    })
}

#[tokio::test]
async fn every_graph_parameter_partitions_exact_and_similarity_lookups() {
    let base = request(json!({}));
    let key = base.cache_key().unwrap();
    let cache = cache();
    cache
        .put_results(
            &key,
            Some(vec![1.0, 0.0]),
            vec![SearchHit {
                document: json!({"document_id": "entities/original"}),
                score: 1.0,
                source: None,
            }],
            Some(json!([{"fact": "original"}])),
        )
        .await;
    for (field, value) in [
        ("collection", json!("vectors;t=0.7")),
        ("edge_collection", json!("edges;neurons=x")),
        ("threshold", json!(0.8)),
        ("seed_limit", json!(2)),
        ("limit", json!(3)),
        ("max_depth", json!(3)),
        ("direction", json!("inbound")),
        ("min_confidence", json!(0.0)),
        ("path_decay", json!(0.5)),
        #[cfg(feature = "enterprise")]
        ("neurons_collection", json!("neurons;facts=\"x\\y")),
        ("graph_facts_limit", json!(1)),
    ] {
        let mut wire = json!({});
        wire[field] = value;
        let mut changed = request(wire);
        assert!(
            cache
                .get_results(&changed.cache_key().unwrap(), None)
                .await
                .is_none(),
            "{field}"
        );
        changed.query = "related".into();
        for vector in [[1.0, 0.0], [0.8, 0.6]] {
            assert!(
                cache
                    .get_results(&changed.cache_key().unwrap(), Some(&vector))
                    .await
                    .is_none(),
                "{field}"
            );
        }
    }
    assert_eq!(
        key,
        request(json!({"query": "  NEEDLE  ", "min_confidence": null}))
            .cache_key()
            .unwrap()
    );
    let mut explicit = serde_json::to_value(&base).unwrap();
    explicit["query"] = json!("needle");
    assert_eq!(key, request(explicit).cache_key().unwrap());
    let similar = request(json!({"query": "related"})).cache_key().unwrap();
    let hit = cache
        .get_results(&similar, Some(&[0.8, 0.6]))
        .await
        .unwrap();
    assert!((hit.similarity - 0.8).abs() < 1e-12);
    assert_eq!(hit.meta, Some(json!([{"fact": "original"}])));
}

struct SimilarEmbedder;

#[async_trait::async_trait]
impl cognigraph_embeddings::EmbeddingProvider for SimilarEmbedder {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>> {
        Ok(texts
            .iter()
            .map(|text| {
                if *text == "related" {
                    vec![0.8, 0.6]
                } else {
                    vec![1.0, 0.0]
                }
            })
            .collect())
    }
}

fn ids(response: &Value) -> Vec<String> {
    let mut ids: Vec<_> = response["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["document_id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

#[tokio::test]
async fn colliding_collection_names_never_share_graph_results_or_facts() {
    let middle = ";t=0.7;seeds=5;l=10;depth=2;dir=outbound;minc=;decay=0.8;neurons=";
    for query in ["needle", "alias", "related"] {
        let left = request(
            json!({"edge_collection": format!("edges{middle}first"), "neurons_collection": "second"}),
        );
        let right = request(
            json!({"edge_collection": "edges", "neurons_collection": format!("first{middle}second"), "query": query}),
        );
        let backend = NativeBackend::new();
        for key in ["root", "left", "right"] {
            backend
                .create_document("entities", json!({"_key": key}))
                .await
                .unwrap();
        }
        backend
            .create_document(
                "embeddings",
                json!({"_key": "seed", "document_id": "entities/root", "embedding": [1.0, 0.0]}),
            )
            .await
            .unwrap();
        for (req, target, relation) in [
            (&left, "entities/left", "LEFT"),
            (&right, "entities/right", "RIGHT"),
        ] {
            backend
                .upsert_edge(
                    &req.edge_collection,
                    "entities/root",
                    target,
                    relation,
                    json!({"evidence_chunk_id": relation}),
                )
                .await
                .unwrap();
        }
        let state = AppState::new(backend)
            .with_embedder(SimilarEmbedder)
            .with_cache(cache());
        let warm = graph_augmented_search(State(state.clone()), Json(left.clone()))
            .await
            .unwrap()
            .0;
        assert_eq!(ids(&warm), ["entities/left", "entities/root"]);
        assert_eq!(warm["graph_facts"][0]["relation"], "LEFT");
        let mut alias = left.clone();
        alias.query = "alias".into();
        let strong = graph_augmented_search(State(state.clone()), Json(alias))
            .await
            .unwrap()
            .0;
        assert_eq!(strong["cached"], true);
        assert_eq!(strong["graph_facts"], warm["graph_facts"]);
        let fresh = graph_augmented_search(State(state.clone()), Json(right.clone()))
            .await
            .unwrap()
            .0;
        assert!(fresh.get("cached").is_none(), "{fresh}");
        assert_eq!(ids(&fresh), ["entities/right", "entities/root"]);
        assert_eq!(fresh["graph_facts"].as_array().unwrap().len(), 1);
        assert_eq!(fresh["graph_facts"][0]["relation"], "RIGHT");
        let repeat = graph_augmented_search(State(state.clone()), Json(right))
            .await
            .unwrap()
            .0;
        assert_eq!(repeat["cached"], true);
        assert_eq!(repeat["results"], fresh["results"]);
        assert_eq!(repeat["graph_facts"], fresh["graph_facts"]);

        // Existing stale-meta tests also use the production builder; this
        // verifies valid assisted reuse on an actual delimiter-bearing route.
        let mut similar = left;
        similar.query = "related".into();
        let assisted = graph_augmented_search(State(state.clone()), Json(similar.clone()))
            .await
            .unwrap()
            .0;
        assert_eq!(assisted["cached"], "assisted");
        assert_eq!(ids(&assisted), ["entities/left", "entities/root"]);
        assert_eq!(assisted["graph_facts"], warm["graph_facts"]);
        let repeat = graph_augmented_search(State(state), Json(similar))
            .await
            .unwrap()
            .0;
        assert_eq!(repeat["cached"], true);
        assert_eq!(repeat["results"], assisted["results"]);
        assert_eq!(repeat["graph_facts"], assisted["graph_facts"]);
    }
}
