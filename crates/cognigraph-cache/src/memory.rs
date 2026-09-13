use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use lru::LruCache;
use tokio::sync::RwLock;

use cognigraph_core::SearchHit;

use crate::normalize::normalize_query;
use crate::similarity::cosine_similarity;
use crate::traits::QueryCache;
use crate::types::{CacheConfig, CacheEntry, CacheHit, CacheKey, CacheStats, SearchMode};

type EmbeddingCache = LruCache<(String, String), CacheEntry<Vec<f64>>>;
/// (collection, search_mode, params) → [(embedding, cache_key)]. Params
/// partition the buckets so a similar query can never borrow results that
/// were computed under different request parameters.
type SimilarityIndex = HashMap<(String, SearchMode, String), Vec<(Vec<f64>, CacheKey)>>;

/// In-memory query cache with LRU eviction and similarity-aware lookup.
pub struct InMemoryCache {
    config: CacheConfig,
    stats: CacheStats,
    /// Advanced by every result invalidation. A search captures this before
    /// reading backend data and may insert only if it is unchanged.
    result_generation: AtomicU64,
    /// Embedding cache: (normalized_query, model) → embedding vector
    embeddings: RwLock<EmbeddingCache>,
    /// Search result cache: CacheKey → results
    results: RwLock<LruCache<CacheKey, CacheEntry<Vec<SearchHit>>>>,
    /// Similarity index: (collection, search_mode) → [(embedding, cache_key)]
    /// Used for scanning cached embeddings to find similar queries.
    sim_index: RwLock<SimilarityIndex>,
}

impl InMemoryCache {
    pub fn new(config: CacheConfig) -> Self {
        let cap = NonZeroUsize::new(config.max_entries.max(1)).unwrap();
        Self {
            stats: CacheStats::new(),
            result_generation: AtomicU64::new(0),
            embeddings: RwLock::new(LruCache::new(cap)),
            results: RwLock::new(LruCache::new(cap)),
            sim_index: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Remove a key from the similarity index.
    fn remove_from_sim_index(index: &mut SimilarityIndex, key: &CacheKey) {
        let bucket_key = (
            key.collection.clone(),
            key.search_mode.clone(),
            key.params.clone(),
        );
        if let Some(bucket) = index.get_mut(&bucket_key) {
            bucket.retain(|(_, k)| k != key);
            if bucket.is_empty() {
                index.remove(&bucket_key);
            }
        }
    }

    /// Remove similarity-index entries only when their result entry is still
    /// absent/expired. Re-checking under the canonical sim-index -> results
    /// lock order avoids deleting a concurrent replacement for the same key.
    async fn remove_stale_from_sim_index(&self, keys: &[CacheKey]) {
        if keys.is_empty() {
            return;
        }
        let mut sim = self.sim_index.write().await;
        let mut cache = self.results.write().await;
        for key in keys {
            let stale = match cache.peek(key) {
                Some(entry) if !entry.is_expired() => false,
                Some(_) => {
                    cache.pop(key);
                    true
                }
                None => true,
            };
            if stale {
                Self::remove_from_sim_index(&mut sim, key);
            }
        }
    }

    async fn put_results_inner(
        &self,
        expected_generation: Option<u64>,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<serde_json::Value>,
    ) -> bool {
        let entry = CacheEntry {
            data: results,
            query_embedding: query_embedding.clone(),
            meta,
            inserted_at: std::time::Instant::now(),
            ttl: self.config.ttl(),
        };

        // Invalidation takes the same locks in the same order and advances
        // the generation while holding them, making check + insert atomic.
        let mut sim = self.sim_index.write().await;
        let mut cache = self.results.write().await;
        if expected_generation
            .is_some_and(|expected| self.result_generation.load(Ordering::Acquire) != expected)
        {
            return false;
        }
        // A key has exactly one similarity-index row. Remove it even when the
        // result LRU no longer contains the old entry (for example, an exact
        // expiry raced with this replacement).
        Self::remove_from_sim_index(&mut sim, key);
        if let Some((evicted_key, _)) = cache.push(key.clone(), entry)
            && evicted_key != *key
        {
            self.stats.record_eviction();
            Self::remove_from_sim_index(&mut sim, &evicted_key);
        }
        if let Some(embedding) = query_embedding {
            let bucket_key = (
                key.collection.clone(),
                key.search_mode.clone(),
                key.params.clone(),
            );
            sim.entry(bucket_key)
                .or_default()
                .push((embedding, key.clone()));
        }
        true
    }
}

#[async_trait]
impl QueryCache for InMemoryCache {
    async fn get_embedding(&self, query: &str, model: &str) -> Option<Vec<f64>> {
        let normalized = normalize_query(query);
        let key = (normalized, model.to_string());
        let mut cache = self.embeddings.write().await;

        if let Some(entry) = cache.get(&key) {
            if entry.is_expired() {
                cache.pop(&key);
                return None;
            }
            return Some(entry.data.clone());
        }
        None
    }

    async fn put_embedding(&self, query: &str, model: &str, embedding: Vec<f64>) {
        let normalized = normalize_query(query);
        let key = (normalized, model.to_string());
        let entry = CacheEntry {
            data: embedding,
            query_embedding: None,
            meta: None,
            inserted_at: std::time::Instant::now(),
            ttl: self.config.ttl(),
        };
        let mut cache = self.embeddings.write().await;
        cache.put(key, entry);
    }

    async fn get_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<&[f64]>,
    ) -> Option<CacheHit> {
        // 1. Try exact key match
        let mut expired_exact = None;
        {
            let mut cache = self.results.write().await;
            if let Some(entry) = cache.get(key) {
                if entry.is_expired() {
                    cache.pop(key);
                    expired_exact = Some(key.clone());
                } else {
                    self.stats.record_hit_direct();
                    tracing::debug!(
                        collection = %key.collection,
                        query = %key.normalized_query,
                        "Cache hit (exact match)"
                    );
                    return Some(CacheHit {
                        results: entry.data.clone(),
                        similarity: 1.0,
                        meta: entry.meta.clone(),
                    });
                }
            }
        }
        if let Some(expired) = expired_exact {
            self.remove_stale_from_sim_index(&[expired]).await;
        }

        // 2. Try similarity-aware lookup if embedding provided
        if let Some(query_emb) = query_embedding {
            let bucket_key = (
                key.collection.clone(),
                key.search_mode.clone(),
                key.params.clone(),
            );
            // Snapshot candidates, then release the index lock before touching
            // results. `put_results` uses sim-index -> results, so nesting the
            // reverse order here would deadlock at capacity.
            let mut candidates: Vec<(f64, CacheKey)> = {
                let sim_index = self.sim_index.read().await;
                sim_index
                    .get(&bucket_key)
                    .into_iter()
                    .flatten()
                    .filter_map(|(cached_emb, cached_key)| {
                        let similarity = cosine_similarity(query_emb, cached_emb);
                        (similarity > 0.0).then(|| (similarity, cached_key.clone()))
                    })
                    .collect()
            };
            candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            let mut stale_keys = Vec::new();
            for (similarity, similar_key) in candidates {
                let hit = {
                    let mut cache = self.results.write().await;
                    match cache.get(&similar_key) {
                        Some(entry) if !entry.is_expired() => Some(CacheHit {
                            results: entry.data.clone(),
                            similarity,
                            meta: entry.meta.clone(),
                        }),
                        Some(_) => {
                            cache.pop(&similar_key);
                            None
                        }
                        None => None,
                    }
                };
                let Some(hit) = hit else {
                    stale_keys.push(similar_key);
                    continue;
                };

                self.remove_stale_from_sim_index(&stale_keys).await;
                if similarity >= self.config.strong_threshold {
                    self.stats.record_hit_direct();
                } else if crate::cache_weight(similarity, self.config.similarity_floor) > 0.0 {
                    self.stats.record_hit_assisted();
                } else {
                    self.stats.record_miss();
                    return None;
                }
                tracing::debug!(
                    collection = %key.collection,
                    similarity,
                    original_query = %similar_key.normalized_query,
                    "Cache hit (similarity={similarity:.3})"
                );
                return Some(hit);
            }
            self.remove_stale_from_sim_index(&stale_keys).await;
        }

        self.stats.record_miss();
        None
    }

    async fn put_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<serde_json::Value>,
    ) {
        self.put_results_inner(None, key, query_embedding, results, meta)
            .await;
    }

    fn result_generation(&self) -> u64 {
        self.result_generation.load(Ordering::Acquire)
    }

    async fn put_results_if_generation(
        &self,
        expected_generation: u64,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<serde_json::Value>,
    ) -> bool {
        self.put_results_inner(
            Some(expected_generation),
            key,
            query_embedding,
            results,
            meta,
        )
        .await
    }

    async fn invalidate(&self, key: &CacheKey) {
        let mut sim = self.sim_index.write().await;
        let mut cache = self.results.write().await;
        self.result_generation.fetch_add(1, Ordering::AcqRel);
        cache.pop(key);
        Self::remove_from_sim_index(&mut sim, key);
    }

    async fn invalidate_collection(&self, collection: &str) {
        let mut sim = self.sim_index.write().await;
        let mut cache = self.results.write().await;
        self.result_generation.fetch_add(1, Ordering::AcqRel);
        let keys_to_remove: Vec<CacheKey> = cache
            .iter()
            .filter(|(k, _)| k.collection == collection)
            .map(|(k, _)| k.clone())
            .collect();

        if keys_to_remove.is_empty() {
            return;
        }

        let count = keys_to_remove.len();
        for key in &keys_to_remove {
            cache.pop(key);
        }
        for key in &keys_to_remove {
            Self::remove_from_sim_index(&mut sim, key);
        }

        self.stats.record_invalidation();
        tracing::debug!(
            collection = %collection,
            invalidated = count,
            "Cache invalidated for collection"
        );
    }

    async fn invalidate_results(&self) {
        let mut sim = self.sim_index.write().await;
        let mut cache = self.results.write().await;
        self.result_generation.fetch_add(1, Ordering::AcqRel);
        if cache.is_empty() && sim.is_empty() {
            return;
        }
        cache.clear();
        sim.clear();
        self.stats.record_invalidation();
    }

    async fn clear(&self) {
        self.embeddings.write().await.clear();
        let mut sim = self.sim_index.write().await;
        let mut results = self.results.write().await;
        self.result_generation.fetch_add(1, Ordering::AcqRel);
        results.clear();
        sim.clear();
    }

    fn stats(&self) -> &CacheStats {
        &self.stats
    }

    async fn entry_count(&self) -> usize {
        self.results.read().await.len()
    }

    fn config(&self) -> &CacheConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn config(max_entries: usize) -> CacheConfig {
        CacheConfig {
            enabled: true,
            ttl_secs: 300,
            max_entries,
            similarity_floor: 0.7,
            strong_threshold: 0.97,
            backend: "memory".into(),
            path: None,
        }
    }

    fn key(query: &str) -> CacheKey {
        CacheKey {
            collection: "embeddings".into(),
            search_mode: SearchMode::Semantic,
            normalized_query: query.into(),
            params: "t=0;l=10".into(),
        }
    }

    fn hit(id: &str) -> SearchHit {
        SearchHit {
            document: serde_json::json!({"document_id": id}),
            score: 1.0,
            source: Some("test".into()),
        }
    }

    #[tokio::test]
    async fn capacity_put_does_not_hold_results_while_waiting_for_similarity_index() {
        let cache = Arc::new(InMemoryCache::new(config(1)));
        let first = key("first");
        cache
            .put_results(&first, Some(vec![1.0, 0.0]), vec![hit("docs/first")], None)
            .await;

        // Hold the index against the capacity-evicting put. The put must wait
        // before taking `results`, otherwise this exact lookup would form the
        // old sim-read -> results / results -> sim-write deadlock.
        let index_guard = cache.sim_index.read().await;
        let writer_cache = cache.clone();
        let started = Arc::new(tokio::sync::Barrier::new(2));
        let writer_started = started.clone();
        let writer = tokio::spawn(async move {
            writer_started.wait().await;
            writer_cache
                .put_results(
                    &key("second"),
                    Some(vec![0.9, 0.1]),
                    vec![hit("docs/second")],
                    None,
                )
                .await;
        });
        started.wait().await;
        // Give the already-scheduled writer a chance to contend on the held
        // similarity lock. Generous operation timeouts avoid loaded-CI flakes.
        tokio::time::sleep(Duration::from_millis(10)).await;

        let exact = tokio::time::timeout(Duration::from_secs(2), cache.get_results(&first, None))
            .await
            .expect("exact lookup blocked behind an index-waiting writer");
        assert!(exact.is_some());

        drop(index_guard);
        tokio::time::timeout(Duration::from_secs(2), writer)
            .await
            .expect("capacity writer deadlocked")
            .expect("capacity writer panicked");
    }

    #[tokio::test]
    async fn expired_closest_candidate_does_not_shadow_next_live_match() {
        let cache = InMemoryCache::new(config(3));
        let stale = key("stale-nearest");
        let live = key("live-second");
        cache
            .put_results(&stale, Some(vec![1.0, 0.0]), vec![hit("docs/stale")], None)
            .await;
        cache
            .put_results(&live, Some(vec![0.95, 0.05]), vec![hit("docs/live")], None)
            .await;
        cache
            .results
            .write()
            .await
            .peek_mut(&stale)
            .expect("stale entry exists")
            .inserted_at = Instant::now() - Duration::from_secs(301);

        let found = cache
            .get_results(&key("probe"), Some(&[1.0, 0.0]))
            .await
            .expect("the next live candidate should win");
        assert_eq!(found.results[0].document["document_id"], "docs/live");

        let index = cache.sim_index.read().await;
        assert!(
            index
                .values()
                .flatten()
                .all(|(_, indexed_key)| indexed_key != &stale),
            "expired key remained in the similarity index"
        );
    }

    #[tokio::test]
    async fn replacing_a_key_is_not_counted_as_capacity_eviction() {
        let cache = InMemoryCache::new(config(1));
        let same = key("same");
        cache
            .put_results(&same, Some(vec![1.0, 0.0]), vec![hit("docs/old")], None)
            .await;
        cache
            .put_results(&same, Some(vec![0.0, 1.0]), vec![hit("docs/new")], None)
            .await;
        assert_eq!(cache.stats.snapshot().evictions, 0);
        let index = cache.sim_index.read().await;
        let rows: Vec<_> = index
            .values()
            .flatten()
            .filter(|(_, indexed_key)| indexed_key == &same)
            .collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, vec![0.0, 1.0]);
    }

    #[tokio::test]
    async fn invalidation_fences_an_inflight_stale_result_put_even_when_empty() {
        let cache = InMemoryCache::new(config(2));
        cache.put_embedding("query", "model", vec![1.0, 0.0]).await;
        let captured_before_backend_read = cache.result_generation();

        // A write completes while the retrieval that captured generation 0 is
        // still in flight. The cache is empty, but the generation must advance.
        cache.invalidate_results().await;
        assert!(cache.result_generation() > captured_before_backend_read);

        let inserted = cache
            .put_results_if_generation(
                captured_before_backend_read,
                &key("stale-inflight"),
                Some(vec![1.0, 0.0]),
                vec![hit("docs/pre-write")],
                None,
            )
            .await;
        assert!(
            !inserted,
            "pre-write results repopulated after invalidation"
        );
        assert_eq!(cache.entry_count().await, 0);
        assert_eq!(
            cache.get_embedding("query", "model").await,
            Some(vec![1.0, 0.0]),
            "result invalidation should not discard query embeddings"
        );
    }
}
