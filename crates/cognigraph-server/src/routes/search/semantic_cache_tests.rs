use super::*;
use cognigraph_cache::{CacheConfig, InMemoryCache};
use cognigraph_core::SearchHit;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

fn request(overrides: Value) -> SemanticSearchRequest {
    let mut wire = json!({"query": "needle", "collection": "models"});
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
async fn semantic_parameters_and_optional_values_partition_all_cache_lookups() {
    let base = request(json!({}));
    let key = base.cache_key().unwrap();
    let cache = cache();
    cache
        .put_results(
            &key,
            Some(vec![1.0, 0.0]),
            vec![SearchHit {
                document: json!({"document_id": "models/original"}),
                score: 1.0,
                source: None,
            }],
            None,
        )
        .await;
    for (field, value) in [
        ("collection", json!("models;m=other")),
        ("threshold", json!(0.8)),
        ("limit", json!(3)),
        ("model_name", json!("")),
        ("model_name", json!("named;model=\"x\\y")),
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
        request(json!({"query": "  NEEDLE  ", "model_name": null}))
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
async fn omitted_and_empty_model_filters_never_share_semantic_results() {
    for (left, right) in [(Value::Null, json!("")), (json!(""), Value::Null)] {
        for query in ["needle", "alias", "related"] {
            let backend = NativeBackend::new();
            for doc in [
                json!({"_key": "named", "model_name": "alpha", "embedding": [1.0, 0.0]}),
                json!({"_key": "empty", "model_name": "", "embedding": [1.0, 0.0]}),
                json!({"_key": "missing", "embedding": [1.0, 0.0]}),
            ] {
                backend.create_document("models", doc).await.unwrap();
            }
            let state = AppState::new(backend)
                .with_embedder(SimilarEmbedder)
                .with_cache(cache());
            let left = request(json!({"model_name": left}));
            let right = request(json!({"model_name": right, "query": query}));
            let expected = |req: &SemanticSearchRequest| {
                if req.model_name.is_some() {
                    vec!["models/empty".to_string()]
                } else {
                    vec![
                        "models/empty".to_string(),
                        "models/missing".to_string(),
                        "models/named".to_string(),
                    ]
                }
            };
            let warm = semantic_search(State(state.clone()), Json(left.clone()))
                .await
                .unwrap()
                .0;
            assert_eq!(ids(&warm), expected(&left));
            let mut alias = left.clone();
            alias.query = "alias".into();
            let strong = semantic_search(State(state.clone()), Json(alias))
                .await
                .unwrap()
                .0;
            assert_eq!(strong["cached"], true);
            assert_eq!(ids(&strong), expected(&left));
            let fresh = semantic_search(State(state.clone()), Json(right.clone()))
                .await
                .unwrap()
                .0;
            assert!(fresh.get("cached").is_none(), "{fresh}");
            assert_eq!(ids(&fresh), expected(&right));
            let repeat = semantic_search(State(state.clone()), Json(right))
                .await
                .unwrap()
                .0;
            assert_eq!(repeat["cached"], true);
            assert_eq!(repeat["results"], fresh["results"]);
            let mut similar = left.clone();
            similar.query = "related".into();
            let assisted = semantic_search(State(state.clone()), Json(similar.clone()))
                .await
                .unwrap()
                .0;
            assert_eq!(assisted["cached"], "assisted");
            assert_eq!(ids(&assisted), expected(&left));
            let repeat = semantic_search(State(state), Json(similar))
                .await
                .unwrap()
                .0;
            assert_eq!(repeat["cached"], true);
            assert_eq!(repeat["results"], assisted["results"]);
        }
    }
}
