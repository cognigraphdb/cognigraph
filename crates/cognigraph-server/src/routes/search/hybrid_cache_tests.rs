use super::*;
use cognigraph_cache::{CacheConfig, InMemoryCache, QueryCache};
use cognigraph_core::{GraphBackend, SearchHit};
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

fn request(overrides: Value) -> HybridSearchRequest {
    let mut wire = json!({
        "query": "needle", "documents_collection": "docs",
        "search_fields": ["a,b"], "limit": 10,
    });
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
async fn every_result_parameter_partitions_exact_and_similarity_lookups() {
    let original = request(json!({}));
    let original_key = original.cache_key().unwrap();
    let cache = cache();
    cache
        .put_results(
            &original_key,
            Some(vec![1.0, 0.0]),
            vec![SearchHit {
                document: json!({"document_id": "docs/original"}),
                score: 1.0,
                source: None,
            }],
            None,
        )
        .await;

    for (field, value) in [
        ("documents_collection", json!("other;view=docs")),
        ("embeddings_collection", json!("other;embeddings")),
        ("search_fields", json!(["a", "b"])),
        ("threshold", json!(0.8)),
        ("limit", json!(3)),
        ("rrf_k", json!(30.0)),
        ("bm25_weight", json!(0.2)),
        ("vector_weight", json!(0.8)),
        ("include_side_views", json!(true)),
        ("side_views_collection", json!("other;svw=0.5")),
        ("side_view_weight", json!(0.9)),
    ] {
        let mut wire = json!({});
        wire[field] = value;
        let mut different = request(wire);
        let exact = different.cache_key().unwrap();
        assert_ne!(original_key, exact, "{field} must partition exact hits");
        assert!(cache.get_results(&exact, None).await.is_none(), "{field}");
        different.query = "related".into();
        let similar = different.cache_key().unwrap();
        assert!(
            cache
                .get_results(&similar, Some(&[0.8, 0.6]))
                .await
                .is_none(),
            "{field} must partition assisted hits"
        );
    }

    assert!(cache.get_results(&original_key, None).await.is_some());
    let mut same_parameters = original;
    same_parameters.query = "related".into();
    let hit = cache
        .get_results(&same_parameters.cache_key().unwrap(), Some(&[0.8, 0.6]))
        .await
        .unwrap();
    assert!((hit.similarity - 0.8).abs() < 1e-12);
    assert_eq!(hit.results[0].document["document_id"], "docs/original");
}

#[test]
fn defaults_query_normalization_and_ordered_fields_keep_their_contract() {
    let implicit: HybridSearchRequest = serde_json::from_value(json!({"query": "needle"})).unwrap();
    let mut explicit = serde_json::to_value(&implicit).unwrap();
    explicit["query"] = json!("  NEEDLE  ");
    let explicit: HybridSearchRequest = serde_json::from_value(explicit).unwrap();
    assert_eq!(implicit.cache_key().unwrap(), explicit.cache_key().unwrap());

    let mut reordered = implicit.clone();
    reordered.search_fields.reverse();
    assert_ne!(
        implicit.cache_key().unwrap(),
        reordered.cache_key().unwrap()
    );
    let mut related = implicit.clone();
    related.query = "related".into();
    assert_eq!(
        implicit.cache_key().unwrap().params,
        related.cache_key().unwrap().params
    );
    assert_ne!(
        implicit.cache_key().unwrap().normalized_query,
        related.cache_key().unwrap().normalized_query
    );
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

fn ids(response: &Value) -> Vec<&str> {
    response["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["document_id"].as_str().unwrap())
        .collect()
}

#[tokio::test]
async fn delimiter_collisions_cannot_reuse_direct_or_assisted_hybrid_results() {
    // Each pair flattened to identical legacy params, but searches different
    // actual text. Exercise the handler, its production builder, and real BM25.
    for (left, right) in [
        (
            json!({"search_fields": ["a,b"]}),
            json!({"search_fields": ["a", "b"]}),
        ),
        (
            json!({"search_fields": [r#"a,"b\c"#]}),
            json!({"search_fields": ["a", r#""b\c"#]}),
        ),
        (
            json!({"documents_collection": "docs;fields=title", "search_fields": ["text"]}),
            json!({"documents_collection": "docs", "search_fields": ["title;fields=text"]}),
        ),
    ] {
        for query in ["needle", "alias", "related"] {
            let left = request(left.clone());
            let mut right = request(right.clone());
            right.query = query.into();
            let backend = NativeBackend::new();
            backend
                .ensure_collection("embeddings", cognigraph_core::CollectionType::Document)
                .await
                .unwrap();
            for (req, key) in [(&left, "left"), (&right, "right")] {
                let mut doc = json!({"_key": key});
                doc[&req.search_fields[0]] = json!("needle alias related");
                backend
                    .create_document(&req.documents_collection, doc)
                    .await
                    .unwrap();
            }
            let left_id = format!("{}/left", left.documents_collection);
            let right_id = format!("{}/right", right.documents_collection);
            let state = AppState::new(backend)
                .with_embedder(SimilarEmbedder)
                .with_cache(cache());
            let warm = hybrid_search(State(state.clone()), Json(left.clone()))
                .await
                .unwrap()
                .0;
            assert_eq!(ids(&warm), vec![left_id.as_str()]);
            let fresh = hybrid_search(State(state.clone()), Json(right.clone()))
                .await
                .unwrap()
                .0;
            assert!(fresh.get("cached").is_none(), "{fresh}");
            assert_eq!(ids(&fresh), vec![right_id.as_str()], "{fresh}");
            let repeat = hybrid_search(State(state.clone()), Json(right))
                .await
                .unwrap()
                .0;
            assert_eq!(repeat["cached"], true);
            assert_eq!(repeat["results"], fresh["results"]);

            // Query similarity still assists when the parameter identity is the
            // same, even if names and field lists contain delimiters.
            let mut similar = left;
            similar.query = "related".into();
            let assisted = hybrid_search(State(state.clone()), Json(similar.clone()))
                .await
                .unwrap()
                .0;
            assert_eq!(assisted["cached"], "assisted", "{assisted}");
            assert_eq!(ids(&assisted), vec![left_id.as_str()]);
            let repeat = hybrid_search(State(state), Json(similar)).await.unwrap().0;
            assert_eq!(repeat["cached"], true);
            assert_eq!(repeat["results"], assisted["results"]);
        }
    }
}
