use async_trait::async_trait;
use cognigraph_core::SearchHit;

use crate::types::{CacheConfig, CacheHit, CacheKey, CacheStats, CacheStatsSnapshot};

/// Trait for pluggable query cache backends (in-memory, redb, remote, etc.).
///
/// Implementations must be thread-safe (`Send + Sync`) and use interior
/// mutability for state management. All methods take `&self` so the cache
/// can be shared via `Arc<dyn QueryCache>`.
#[async_trait]
pub trait QueryCache: Send + Sync {
    // --- Embedding cache ---

    /// Look up a cached embedding for the given (query, model) pair.
    async fn get_embedding(&self, query: &str, model: &str) -> Option<Vec<f64>>;

    /// Store an embedding for the given (query, model) pair.
    async fn put_embedding(&self, query: &str, model: &str, embedding: Vec<f64>);

    // --- Search result cache ---

    /// Look up cached search results.
    ///
    /// Returns the best matching cached results above the configured
    /// similarity floor. The caller uses `cache_weight(similarity, floor)` to compute
    /// the continuous influence weight:
    /// - `similarity >= strong_threshold` → return directly (fast path)
    /// - a lower, above-floor similarity → merge with fresh results at computed weight
    /// - no match at all → `None`, run pure fresh search
    ///
    /// When `query_embedding` is `None`, only exact normalized-query matching
    /// is used (similarity = 1.0).
    async fn get_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<&[f64]>,
    ) -> Option<CacheHit>;

    /// Store search results in the cache, optionally with a route-specific
    /// meta payload returned verbatim on hits.
    async fn put_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<serde_json::Value>,
    );

    /// Current result-cache generation. Search pipelines capture this before
    /// reading backend data, then use [`Self::put_results_if_generation`] so
    /// a write-side invalidation cannot be followed by an in-flight stale put.
    fn result_generation(&self) -> u64;

    /// Store results only if no invalidation has advanced the generation
    /// since the caller began retrieval. The generation check and insertion
    /// must be atomic with respect to invalidation.
    async fn put_results_if_generation(
        &self,
        expected_generation: u64,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<serde_json::Value>,
    ) -> bool;

    // --- Management ---

    /// Remove a specific entry from the cache.
    async fn invalidate(&self, key: &CacheKey);

    /// Invalidate all cached results for a given collection across all
    /// search modes and queries. Called when documents in a collection
    /// are created, updated, or deleted.
    async fn invalidate_collection(&self, collection: &str);

    /// Invalidate every cached search result while retaining deterministic
    /// query embeddings. Use this after writes whose result dependencies are
    /// broader than one collection (hybrid/graph search, opaque scripts, raw
    /// backend queries).
    async fn invalidate_results(&self);

    /// Clear all cached entries.
    async fn clear(&self);

    /// Access the live statistics counters.
    fn stats(&self) -> &CacheStats;

    /// Return a point-in-time snapshot of cache statistics.
    fn stats_snapshot(&self) -> CacheStatsSnapshot {
        self.stats().snapshot()
    }

    /// Return the number of entries currently in the result cache.
    async fn entry_count(&self) -> usize;

    /// Access the cache configuration.
    fn config(&self) -> &CacheConfig;
}
