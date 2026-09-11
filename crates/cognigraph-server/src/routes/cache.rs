use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stats", get(cache_stats))
        .route("/clear", post(clear_cache))
}

async fn cache_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let Some(cache) = state.cache.as_deref() else {
        return Json(serde_json::json!({ "enabled": false }));
    };

    let snapshot = cache.stats_snapshot();
    let total = snapshot.hits + snapshot.misses;
    let hit_rate = if total > 0 {
        snapshot.hits as f64 / total as f64
    } else {
        0.0
    };
    let config = cache.config();
    let entries = cache.entry_count().await;

    Json(serde_json::json!({
        "enabled": true,
        "entries": entries,
        "max_entries": config.max_entries,
        "ttl_secs": config.ttl_secs,
        "similarity_floor": config.similarity_floor,
        "strong_threshold": config.strong_threshold,
        "totals": {
            "hits": snapshot.hits,
            "misses": snapshot.misses,
            "evictions": snapshot.evictions,
            "invalidations": snapshot.invalidations,
            "hit_rate": (hit_rate * 1000.0).round() / 1000.0,
        },
        "hits_by_type": {
            "direct": snapshot.hits_direct,
            "assisted": snapshot.hits_assisted,
        },
    }))
}

async fn clear_cache(State(state): State<AppState>) -> Json<serde_json::Value> {
    let Some(cache) = state.cache.as_deref() else {
        return Json(serde_json::json!({
            "cleared": false,
            "reason": "cache not enabled",
        }));
    };

    let entries_before = cache.entry_count().await;
    cache.clear().await;

    Json(serde_json::json!({
        "cleared": true,
        "entries_removed": entries_before,
    }))
}
