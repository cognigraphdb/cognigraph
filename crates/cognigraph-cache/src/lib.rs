pub mod memory;
pub mod normalize;
pub mod persistent;
pub mod similarity;
pub mod traits;
pub mod types;

pub use memory::InMemoryCache;
pub use normalize::normalize_query;
pub use persistent::PersistentCache;
pub use similarity::cosine_similarity;
pub use traits::QueryCache;
pub use types::{
    CacheConfig, CacheHit, CacheKey, CacheStats, CacheStatsSnapshot, SearchMode, cache_weight,
    rank_decay,
};
