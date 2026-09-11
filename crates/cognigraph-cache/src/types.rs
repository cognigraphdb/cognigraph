use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cognigraph_core::SearchHit;
use serde::{Deserialize, Serialize};

/// Cache backend configuration, loaded from environment variables.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Time-to-live for cache entries in seconds
    pub ttl_secs: u64,
    /// Maximum number of entries in the result cache
    pub max_entries: usize,
    /// Minimum cosine similarity for cache to return a match (0.0–1.0).
    /// Lower values cast a wider net; higher values require closer matches.
    pub similarity_floor: f64,
    /// Similarity at or above which cached results are returned directly
    /// without running a fresh search (the "fast path").
    pub strong_threshold: f64,
    /// Cache backend type: "memory" (default) or "persistent" (redb-backed
    /// embedding store; requires `path`).
    pub backend: String,
    /// Store file for the persistent backend.
    pub path: Option<String>,
}

impl CacheConfig {
    /// Load cache configuration from environment variables with defaults.
    /// `COGNIGRAPH_QUERY_CACHE_*` — "query cache" distinguishes this
    /// (semantic result cache) from the native backend's paged document
    /// cache (`COGNIGRAPH_CACHE_BYTES`). See decision_env_naming.md.
    pub fn from_env() -> Self {
        Self {
            enabled: env("COGNIGRAPH_QUERY_CACHE_ENABLED", "false") == "true",
            ttl_secs: env("COGNIGRAPH_QUERY_CACHE_TTL_SECS", "900")
                .parse()
                .unwrap_or(900),
            max_entries: env("COGNIGRAPH_QUERY_CACHE_MAX_ENTRIES", "1000")
                .parse()
                .unwrap_or(1000),
            similarity_floor: env("COGNIGRAPH_QUERY_CACHE_SIMILARITY_FLOOR", "0.7")
                .parse()
                .unwrap_or(0.7),
            strong_threshold: env("COGNIGRAPH_QUERY_CACHE_STRONG_THRESHOLD", "0.97")
                .parse()
                .unwrap_or(0.97),
            backend: env("COGNIGRAPH_QUERY_CACHE_BACKEND", "memory"),
            path: std::env::var("COGNIGRAPH_QUERY_CACHE_PATH")
                .ok()
                .filter(|v| !v.is_empty()),
        }
    }

    pub fn ttl(&self) -> Duration {
        Duration::from_secs(self.ttl_secs)
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl_secs: 900,
            max_entries: 1000,
            similarity_floor: 0.7,
            strong_threshold: 0.97,
            backend: "memory".into(),
            path: None,
        }
    }
}

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Identifies which search mode produced the cached results.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum SearchMode {
    Vector,
    Semantic,
    Hybrid,
    GraphAugmented,
}

/// Composite key for looking up cached search results.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct CacheKey {
    pub collection: String,
    pub search_mode: SearchMode,
    pub normalized_query: String,
    /// Fingerprint of every request parameter that shapes the result set
    /// (threshold, limit, fusion weights, traversal knobs, …). Two requests
    /// may only share a cache entry when their fingerprints match — the
    /// similarity lookup is fuzzy on the query text, never on parameters.
    pub params: String,
}

/// A cached value together with its metadata.
pub struct CacheEntry<T> {
    pub data: T,
    /// The query embedding used to produce this result (for similarity matching).
    pub query_embedding: Option<Vec<f64>>,
    /// Route-specific extra payload cached alongside the results (e.g. the
    /// graph-augmented route's ranked `graph_facts`).
    pub meta: Option<serde_json::Value>,
    pub inserted_at: Instant,
    pub ttl: Duration,
}

impl<T> CacheEntry<T> {
    pub fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

/// Atomic counters for cache observability.
pub struct CacheStats {
    /// Total cache lookups that returned a result
    pub hits: AtomicU64,
    /// Total cache lookups that returned nothing
    pub misses: AtomicU64,
    /// Total LRU evictions
    pub evictions: AtomicU64,
    /// Hits returned directly without fresh search (exact match or sim >= strong_threshold)
    pub hits_direct: AtomicU64,
    /// Hits used as a signal merged with fresh search (sim < strong_threshold)
    pub hits_assisted: AtomicU64,
    /// Collection-wide invalidations
    pub invalidations: AtomicU64,
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            hits_direct: AtomicU64::new(0),
            hits_assisted: AtomicU64::new(0),
            invalidations: AtomicU64::new(0),
        }
    }

    pub fn record_hit_direct(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.hits_direct.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_hit_assisted(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        self.hits_assisted.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_invalidation(&self) {
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> CacheStatsSnapshot {
        CacheStatsSnapshot {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            hits_direct: self.hits_direct.load(Ordering::Relaxed),
            hits_assisted: self.hits_assisted.load(Ordering::Relaxed),
            invalidations: self.invalidations.load(Ordering::Relaxed),
        }
    }
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new()
    }
}

/// A point-in-time snapshot of cache statistics (for JSON serialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    /// Direct hits (returned without fresh search)
    pub hits_direct: u64,
    /// Assisted hits (merged with fresh search)
    pub hits_assisted: u64,
    /// Collection-wide invalidation count
    pub invalidations: u64,
}

/// A cache hit carrying the matched results and similarity score.
///
/// The caller uses `similarity` to decide behavior:
/// - `similarity >= config.strong_threshold` → return directly (fast path)
/// - otherwise → use as signal, merge with fresh results (weight from `cache_weight()`)
#[derive(Debug, Clone)]
pub struct CacheHit {
    /// The cached search results.
    pub results: Vec<SearchHit>,
    /// Cosine similarity score (1.0 for exact matches).
    pub similarity: f64,
    /// Extra payload stored with the entry (see `CacheEntry::meta`).
    pub meta: Option<serde_json::Value>,
}

/// Compute the cache influence weight from a similarity score.
///
/// Uses a cubic curve so that high-similarity matches contribute strongly
/// while low-similarity matches contribute only lightly:
///
/// - `similarity = 1.0`  → weight ≈ 1.0
/// - `similarity = 0.95` → weight ≈ 0.58
/// - `similarity = 0.85` → weight ≈ 0.13
/// - `similarity ≤ floor` → weight = 0.0
///
/// The curve is: `((sim - floor) / (1 - floor))³`
pub fn cache_weight(similarity: f64, floor: f64) -> f64 {
    if similarity <= floor || floor >= 1.0 {
        return 0.0;
    }
    let normalized = ((similarity - floor) / (1.0 - floor)).clamp(0.0, 1.0);
    normalized.powi(3)
}

/// Compute rank decay for a cached document at a given position.
///
/// Higher-ranked (lower index) cached docs get more influence.
/// decay(0) = 1.0, decay(5) ≈ 0.57, decay(10) ≈ 0.40
pub fn rank_decay(rank: usize) -> f64 {
    1.0 / (1.0 + rank as f64 * 0.15)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // cache_weight tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cache_weight_at_floor() {
        assert_eq!(cache_weight(0.7, 0.7), 0.0);
    }

    #[test]
    fn test_cache_weight_below_floor() {
        assert_eq!(cache_weight(0.5, 0.7), 0.0);
        assert_eq!(cache_weight(0.0, 0.7), 0.0);
        assert_eq!(cache_weight(-0.5, 0.7), 0.0);
    }

    #[test]
    fn test_cache_weight_at_max() {
        let w = cache_weight(1.0, 0.7);
        assert!((w - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cache_weight_cubic_curve() {
        // sim=0.85, floor=0.7 → normalized=0.5 → 0.5^3=0.125
        let w = cache_weight(0.85, 0.7);
        assert!((w - 0.125).abs() < 1e-10);

        // sim=0.95, floor=0.7 → normalized=0.833 → ~0.579
        let w = cache_weight(0.95, 0.7);
        assert!((w - 0.5787).abs() < 0.001);
    }

    #[test]
    fn test_cache_weight_monotonicity() {
        let floor = 0.7;
        let mut prev = 0.0;
        // Weight must increase monotonically with similarity
        for i in 71..=100 {
            let sim = i as f64 / 100.0;
            let w = cache_weight(sim, floor);
            assert!(
                w >= prev,
                "weight must increase: w({sim})={w} < prev={prev}"
            );
            prev = w;
        }
    }

    #[test]
    fn test_cache_weight_whisper_range() {
        // Low similarities should produce near-zero weights
        assert!(cache_weight(0.75, 0.7) < 0.01);
        assert!(cache_weight(0.72, 0.7) < 0.001);
    }

    #[test]
    fn test_cache_weight_strong_range() {
        // High similarities should produce substantial weights
        assert!(cache_weight(0.97, 0.7) > 0.7);
        assert!(cache_weight(0.98, 0.7) > 0.8);
    }

    #[test]
    fn test_cache_weight_floor_at_one() {
        // Degenerate case: floor >= 1.0 should always return 0
        assert_eq!(cache_weight(1.0, 1.0), 0.0);
        assert_eq!(cache_weight(0.99, 1.0), 0.0);
    }

    // -----------------------------------------------------------------------
    // rank_decay tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_rank_decay_top() {
        assert_eq!(rank_decay(0), 1.0);
    }

    #[test]
    fn test_rank_decay_monotonicity() {
        for rank in 1..20 {
            assert!(
                rank_decay(rank) < rank_decay(rank - 1),
                "decay must decrease: rank={rank}"
            );
        }
    }

    #[test]
    fn test_rank_decay_always_positive() {
        for rank in 0..1000 {
            assert!(
                rank_decay(rank) > 0.0,
                "decay must be positive: rank={rank}"
            );
        }
    }

    #[test]
    fn test_rank_decay_expected_values() {
        // rank 5: 1/(1+0.75) ≈ 0.571
        let d = rank_decay(5);
        assert!((d - 0.5714).abs() < 0.001);

        // rank 10: 1/(1+1.5) ≈ 0.4
        let d = rank_decay(10);
        assert!((d - 0.4).abs() < 0.001);
    }

    // -----------------------------------------------------------------------
    // CacheEntry expiry tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cache_entry_not_expired() {
        let entry = CacheEntry {
            data: "test",
            query_embedding: None,
            meta: None,
            inserted_at: Instant::now(),
            ttl: Duration::from_secs(60),
        };
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_cache_entry_expired() {
        let entry = CacheEntry {
            data: "test",
            query_embedding: None,
            meta: None,
            inserted_at: Instant::now() - Duration::from_secs(120),
            ttl: Duration::from_secs(60),
        };
        assert!(entry.is_expired());
    }
}
