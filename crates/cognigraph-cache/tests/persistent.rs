//! Persistent cache backend: embeddings survive a restart; results and
//! semantics otherwise match the in-memory backend.

use cognigraph_cache::{CacheConfig, CacheKey, PersistentCache, QueryCache, SearchMode};

fn config() -> CacheConfig {
    CacheConfig {
        enabled: true,
        ..CacheConfig::default()
    }
}

fn key(query: &str) -> CacheKey {
    CacheKey {
        collection: "docs".into(),
        search_mode: SearchMode::Semantic,
        normalized_query: query.into(),
        params: "t=0.3;l=10".into(),
    }
}

#[tokio::test]
async fn embeddings_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.redb");

    {
        let cache = PersistentCache::open(config(), &path).unwrap();
        cache
            .put_embedding(
                "What is CogniGraph?",
                "text-embedding-3-small",
                vec![0.1, 0.2, 0.3],
            )
            .await;
        // Normalization applies on the persistent path too.
        assert_eq!(
            cache
                .get_embedding("  what is cognigraph?  ", "text-embedding-3-small")
                .await,
            Some(vec![0.1, 0.2, 0.3])
        );
        // Different model = different entry.
        assert_eq!(
            cache
                .get_embedding("What is CogniGraph?", "other-model")
                .await,
            None
        );
    }

    // "Restart": a fresh instance over the same file still has it.
    let cache = PersistentCache::open(config(), &path).unwrap();
    assert_eq!(
        cache
            .get_embedding("What is CogniGraph?", "text-embedding-3-small")
            .await,
        Some(vec![0.1, 0.2, 0.3])
    );
    // Second lookup is served from the promoted in-memory entry.
    assert_eq!(
        cache
            .get_embedding("What is CogniGraph?", "text-embedding-3-small")
            .await,
        Some(vec![0.1, 0.2, 0.3])
    );
}

#[tokio::test]
async fn results_do_not_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.redb");

    {
        let cache = PersistentCache::open(config(), &path).unwrap();
        cache
            .put_results(
                &key("q"),
                None,
                Vec::new(),
                Some(serde_json::json!({"m":1})),
            )
            .await;
        let hit = cache.get_results(&key("q"), None).await.unwrap();
        assert_eq!(hit.similarity, 1.0);
        assert_eq!(hit.meta, Some(serde_json::json!({"m":1})));
        assert_eq!(cache.entry_count().await, 1);
    }

    let cache = PersistentCache::open(config(), &path).unwrap();
    assert!(cache.get_results(&key("q"), None).await.is_none());
    assert_eq!(cache.entry_count().await, 0);
}

#[tokio::test]
async fn clear_wipes_the_store_too() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache.redb");

    {
        let cache = PersistentCache::open(config(), &path).unwrap();
        cache.put_embedding("q", "m", vec![1.0]).await;
        cache.clear().await;
        assert_eq!(cache.get_embedding("q", "m").await, None);
    }

    let cache = PersistentCache::open(config(), &path).unwrap();
    assert_eq!(cache.get_embedding("q", "m").await, None);
}

#[tokio::test]
async fn open_fails_loudly_on_bad_path() {
    assert!(PersistentCache::open(config(), "/nonexistent-dir/cache.redb").is_err());
}
