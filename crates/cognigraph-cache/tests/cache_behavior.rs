//! Behavioral tests for the InMemoryCache.
//!
//! These tests verify the cache's fusion, invalidation, and ranking properties
//! using synthetic data — no database required.

use cognigraph_cache::{
    CacheConfig, CacheKey, InMemoryCache, QueryCache, SearchMode, cache_weight,
};
use cognigraph_core::SearchHit;

/// Helper: create a cache with custom config.
fn test_cache(floor: f64, strong: f64) -> InMemoryCache {
    InMemoryCache::new(CacheConfig {
        enabled: true,
        ttl_secs: 300,
        max_entries: 100,
        similarity_floor: floor,
        strong_threshold: strong,
        backend: "memory".into(),
        path: None,
    })
}

/// Helper: create a SearchHit with a doc ID and score.
fn hit(doc_id: &str, score: f64) -> SearchHit {
    SearchHit {
        document: serde_json::json!({
            "document_id": doc_id,
            "score": score,
        }),
        score,
        source: Some("test".into()),
    }
}

/// Helper: create a cache key for semantic search.
fn semantic_key(collection: &str, query: &str) -> CacheKey {
    semantic_key_with_params(collection, query, "t=0.3;l=10")
}

/// Helper: create a cache key with an explicit params fingerprint.
fn semantic_key_with_params(collection: &str, query: &str, params: &str) -> CacheKey {
    CacheKey {
        collection: collection.into(),
        search_mode: SearchMode::Semantic,
        normalized_query: query.into(),
        params: params.into(),
    }
}

/// Helper: create a simple embedding vector pointing in a direction.
/// Uses unit-ish vectors in 3D for easy cosine similarity reasoning.
fn embedding(x: f64, y: f64, z: f64) -> Vec<f64> {
    let mag = (x * x + y * y + z * z).sqrt();
    if mag == 0.0 {
        vec![0.0, 0.0, 0.0]
    } else {
        vec![x / mag, y / mag, z / mag]
    }
}

// ---------------------------------------------------------------------------
// 1. Exact match returns Strong hit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_exact_match_returns_direct_hit() {
    let cache = test_cache(0.7, 0.97);
    let key = semantic_key("docs", "hello world");
    let emb = embedding(1.0, 0.0, 0.0);
    let results = vec![hit("docs/1", 0.95), hit("docs/2", 0.80)];

    cache
        .put_results(&key, Some(emb.clone()), results.clone(), None)
        .await;

    let cache_hit = cache.get_results(&key, Some(&emb)).await;
    assert!(cache_hit.is_some());
    let cache_hit = cache_hit.unwrap();
    assert_eq!(cache_hit.similarity, 1.0); // exact match
    assert_eq!(cache_hit.results.len(), 2);

    // Stats: should be a direct hit
    let stats = cache.stats().snapshot();
    assert_eq!(stats.hits_direct, 1);
    assert_eq!(stats.hits_assisted, 0);
}

// ---------------------------------------------------------------------------
// 2. Similar query returns hit with correct similarity
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_similar_query_returns_hit() {
    let cache = test_cache(0.7, 0.97);
    let key = semantic_key("docs", "original query");
    let emb = embedding(1.0, 0.0, 0.0);
    let results = vec![hit("docs/1", 0.9)];

    cache.put_results(&key, Some(emb), results, None).await;

    // Query with a slightly different embedding (high similarity)
    let similar_emb = embedding(1.0, 0.1, 0.0); // cos sim ≈ 0.995
    let new_key = semantic_key("docs", "similar query");
    let cache_hit = cache.get_results(&new_key, Some(&similar_emb)).await;

    assert!(cache_hit.is_some());
    let cache_hit = cache_hit.unwrap();
    assert!(cache_hit.similarity > 0.99);
    assert!(cache_hit.similarity >= 0.97); // should be direct
}

// ---------------------------------------------------------------------------
// 3. A low but above-floor similarity returns a weak signal
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_low_similarity_still_returns_hit() {
    let cache = test_cache(0.7, 0.97);
    let key = semantic_key("docs", "original");
    let emb = embedding(1.0, 0.0, 0.0);
    cache
        .put_results(&key, Some(emb), vec![hit("docs/1", 0.9)], None)
        .await;

    // Different direction — lower similarity
    let diff_emb = embedding(1.0, 1.0, 0.0); // cos sim ≈ 0.707
    let new_key = semantic_key("docs", "different query");
    let cache_hit = cache.get_results(&new_key, Some(&diff_emb)).await;

    assert!(cache_hit.is_some());
    let cache_hit = cache_hit.unwrap();
    assert!(cache_hit.similarity > 0.5);
    assert!(cache_hit.similarity < 0.97); // not direct

    // Weight should be very low
    let w = cache_weight(cache_hit.similarity, 0.7);
    assert!(
        w < 0.05,
        "low similarity should produce near-zero weight: {w}"
    );
}

#[tokio::test]
async fn test_below_floor_is_a_miss_not_an_assisted_hit() {
    let cache = test_cache(0.8, 0.97);
    let key = semantic_key("docs", "original");
    cache
        .put_results(
            &key,
            Some(embedding(1.0, 0.0, 0.0)),
            vec![hit("docs/1", 0.9)],
            None,
        )
        .await;

    let below_floor = embedding(1.0, 1.0, 0.0); // cosine ~= 0.707
    assert!(
        cache
            .get_results(&semantic_key("docs", "below-floor"), Some(&below_floor))
            .await
            .is_none()
    );
    let stats = cache.stats().snapshot();
    assert_eq!(stats.hits, 0);
    assert_eq!(stats.hits_assisted, 0);
    assert_eq!(stats.misses, 1);
}

// ---------------------------------------------------------------------------
// 4. No pollution: low-similarity cache should not dominate fresh results
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_no_pollution_from_low_similarity() {
    // With sim ≈ 0.71, the weight should be negligible
    let sim = 0.71;
    let w = cache_weight(sim, 0.7);

    // At this weight, cached RRF contribution is tiny:
    // best case: w * rank_decay(0) / (60 + 1) ≈ w * 1.0 / 61
    let max_cached_contribution = w / 61.0;

    // Fresh result at rank 0: 1.0 / 61 ≈ 0.0164
    let fresh_contribution = 1.0 / 61.0;

    assert!(
        max_cached_contribution < fresh_contribution * 0.01,
        "cached contribution ({max_cached_contribution}) should be <1% of fresh ({fresh_contribution})"
    );
}

// ---------------------------------------------------------------------------
// 5. Orthogonal embedding returns no hit (similarity ≈ 0)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_orthogonal_returns_no_useful_signal() {
    let cache = test_cache(0.7, 0.97);
    let key = semantic_key("docs", "original");
    let emb = embedding(1.0, 0.0, 0.0);
    cache
        .put_results(&key, Some(emb), vec![hit("docs/1", 0.9)], None)
        .await;

    // Orthogonal embedding — cos sim ≈ 0
    let ortho_emb = embedding(0.0, 1.0, 0.0);
    let new_key = semantic_key("docs", "unrelated");
    let cache_hit = cache.get_results(&new_key, Some(&ortho_emb)).await;

    // May or may not return a hit, but if it does, weight should be 0
    if let Some(hit) = cache_hit {
        let w = cache_weight(hit.similarity, 0.7);
        assert_eq!(w, 0.0, "orthogonal query should produce zero weight");
    }
}

// ---------------------------------------------------------------------------
// 6. Invalidation clears collection results
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_invalidate_collection_clears_all_modes() {
    let cache = test_cache(0.7, 0.97);
    let emb = embedding(1.0, 0.0, 0.0);

    // Insert results for different search modes in the same collection
    let sem_key = CacheKey {
        collection: "docs".into(),
        search_mode: SearchMode::Semantic,
        normalized_query: "test".into(),
        params: "t=0.3;l=10".into(),
    };
    let hyb_key = CacheKey {
        collection: "docs".into(),
        search_mode: SearchMode::Hybrid,
        normalized_query: "test".into(),
        params: "t=0.3;l=10".into(),
    };
    let other_key = CacheKey {
        collection: "other_coll".into(),
        search_mode: SearchMode::Semantic,
        normalized_query: "test".into(),
        params: "t=0.3;l=10".into(),
    };

    cache
        .put_results(&sem_key, Some(emb.clone()), vec![hit("docs/1", 0.9)], None)
        .await;
    cache
        .put_results(&hyb_key, Some(emb.clone()), vec![hit("docs/2", 0.8)], None)
        .await;
    cache
        .put_results(
            &other_key,
            Some(emb.clone()),
            vec![hit("other/1", 0.7)],
            None,
        )
        .await;

    assert_eq!(cache.entry_count().await, 3);

    // Invalidate "docs" collection
    cache.invalidate_collection("docs").await;

    assert_eq!(cache.entry_count().await, 1); // only "other_coll" remains

    // "docs" queries should miss
    assert!(cache.get_results(&sem_key, Some(&emb)).await.is_none());
    assert!(cache.get_results(&hyb_key, Some(&emb)).await.is_none());

    // "other_coll" should still hit
    assert!(cache.get_results(&other_key, Some(&emb)).await.is_some());
}

// ---------------------------------------------------------------------------
// 7. Embedding cache works independently
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_embedding_cache() {
    let cache = test_cache(0.7, 0.97);

    cache
        .put_embedding("hello world", "model-a", vec![1.0, 2.0, 3.0])
        .await;

    // Exact match
    let result = cache.get_embedding("hello world", "model-a").await;
    assert!(result.is_some());
    assert_eq!(result.unwrap(), vec![1.0, 2.0, 3.0]);

    // Different model → miss
    let result = cache.get_embedding("hello world", "model-b").await;
    assert!(result.is_none());

    // Different query → miss
    let result = cache.get_embedding("goodbye world", "model-a").await;
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// 8. LRU eviction works correctly
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_lru_eviction() {
    let cache = InMemoryCache::new(CacheConfig {
        enabled: true,
        ttl_secs: 300,
        max_entries: 3,
        similarity_floor: 0.7,
        strong_threshold: 0.97,
        backend: "memory".into(),
        path: None,
    });

    let emb = embedding(1.0, 0.0, 0.0);
    for i in 0..5 {
        let key = semantic_key("docs", &format!("query-{i}"));
        cache
            .put_results(
                &key,
                Some(emb.clone()),
                vec![hit(&format!("docs/{i}"), 0.9)],
                None,
            )
            .await;
    }

    // Only 3 entries should remain (LRU evicted the first 2)
    assert_eq!(cache.entry_count().await, 3);

    let stats = cache.stats().snapshot();
    assert_eq!(stats.evictions, 2);
}

// ---------------------------------------------------------------------------
// 9. Clear removes everything
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_clear() {
    let cache = test_cache(0.7, 0.97);
    let emb = embedding(1.0, 0.0, 0.0);

    cache
        .put_results(
            &semantic_key("docs", "q1"),
            Some(emb.clone()),
            vec![hit("docs/1", 0.9)],
            None,
        )
        .await;
    cache.put_embedding("hello", "model", vec![1.0, 2.0]).await;

    assert_eq!(cache.entry_count().await, 1);

    cache.clear().await;

    assert_eq!(cache.entry_count().await, 0);
    assert!(cache.get_embedding("hello", "model").await.is_none());
}

// ---------------------------------------------------------------------------
// 10. Cross-collection isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cross_collection_isolation() {
    let cache = test_cache(0.7, 0.97);
    let emb = embedding(1.0, 0.0, 0.0);

    // Cache results in two different collections with same query
    let key_a = semantic_key("collection_a", "shared query");
    let key_b = semantic_key("collection_b", "shared query");

    cache
        .put_results(&key_a, Some(emb.clone()), vec![hit("a/1", 0.9)], None)
        .await;
    cache
        .put_results(&key_b, Some(emb.clone()), vec![hit("b/1", 0.8)], None)
        .await;

    // Each collection should return its own results
    let hit_a = cache.get_results(&key_a, Some(&emb)).await.unwrap();
    assert_eq!(hit_a.results[0].document["document_id"], "a/1");

    let hit_b = cache.get_results(&key_b, Some(&emb)).await.unwrap();
    assert_eq!(hit_b.results[0].document["document_id"], "b/1");
}

// ---------------------------------------------------------------------------
// 11. Stats tracking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_stats_tracking() {
    let cache = test_cache(0.7, 0.97);
    let emb = embedding(1.0, 0.0, 0.0);
    let key = semantic_key("docs", "test");

    // Miss
    cache.get_results(&key, Some(&emb)).await;

    // Store and hit
    cache
        .put_results(&key, Some(emb.clone()), vec![hit("docs/1", 0.9)], None)
        .await;
    cache.get_results(&key, Some(&emb)).await;

    let stats = cache.stats().snapshot();
    assert_eq!(stats.misses, 1);
    assert_eq!(stats.hits, 1);
    assert_eq!(stats.hits_direct, 1);
}

// ---------------------------------------------------------------------------
// 12. Similarity match stats tracked as assisted
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_similarity_hit_tracked_as_assisted() {
    let cache = test_cache(0.7, 0.97);
    let emb = embedding(1.0, 0.0, 0.0);
    let key = semantic_key("docs", "original");

    cache
        .put_results(&key, Some(emb), vec![hit("docs/1", 0.9)], None)
        .await;

    // Query with similar but not strong embedding
    // cos(1,0,0 vs 1,0.5,0) ≈ 0.894 — below strong threshold
    let sim_emb = embedding(1.0, 0.5, 0.0);
    let new_key = semantic_key("docs", "similar");
    let result = cache.get_results(&new_key, Some(&sim_emb)).await;

    assert!(result.is_some());
    let sim = result.unwrap().similarity;
    assert!(sim < 0.97);

    let stats = cache.stats().snapshot();
    assert_eq!(stats.hits_assisted, 1);
    assert_eq!(stats.hits_direct, 0);
}

// ---------------------------------------------------------------------------
// 13. Different request params never share a cache entry
// ---------------------------------------------------------------------------

/// Regression: threshold/limit used to be absent from the key, so a query
/// cached at threshold 0.3 answered the same query at threshold 0.9 with
/// sub-threshold results. Params must partition both lookup paths.
#[tokio::test]
async fn test_params_mismatch_misses_exact_path() {
    let cache = test_cache(0.7, 0.97);
    let emb = embedding(1.0, 0.0, 0.0);
    let warm = semantic_key_with_params("docs", "hello", "t=0.3;l=10");

    cache
        .put_results(&warm, Some(emb.clone()), vec![hit("docs/1", 0.6)], None)
        .await;

    // Identical query text and embedding, stricter threshold → must miss.
    let strict = semantic_key_with_params("docs", "hello", "t=0.9;l=10");
    assert!(
        cache.get_results(&strict, None).await.is_none(),
        "exact-path lookup must not cross params"
    );

    // Same params → still hits.
    assert!(cache.get_results(&warm, None).await.is_some());
}

#[tokio::test]
async fn test_params_mismatch_misses_similarity_path() {
    let cache = test_cache(0.7, 0.97);
    let emb = embedding(1.0, 0.0, 0.0);
    let warm = semantic_key_with_params("docs", "hello", "t=0.3;l=10");

    cache
        .put_results(&warm, Some(emb.clone()), vec![hit("docs/1", 0.6)], None)
        .await;

    // A near-identical embedding under different params must not borrow
    // the entry through the similarity index.
    let similar_emb = embedding(1.0, 0.01, 0.0);
    let strict = semantic_key_with_params("docs", "hello there", "t=0.9;l=10");
    assert!(
        cache
            .get_results(&strict, Some(&similar_emb))
            .await
            .is_none(),
        "similarity lookup must not cross params"
    );

    // Same params → the similarity path still works.
    let same_params = semantic_key_with_params("docs", "hello there", "t=0.3;l=10");
    let borrowed = cache.get_results(&same_params, Some(&similar_emb)).await;
    assert!(borrowed.is_some());
    assert!(borrowed.unwrap().similarity > 0.99);
}
