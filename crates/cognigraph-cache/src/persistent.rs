//! Persistent query cache: in-memory semantics with a redb-backed
//! embedding store underneath (decision_cache_backends.md).
//!
//! Only EMBEDDINGS persist. They are deterministic per (query, model), so
//! surviving a restart saves real provider calls; search RESULTS are
//! TTL-bounded and go stale with the data, so they stay memory-only and
//! die with the process. Store commits are the default `Immediate`
//! (~4 ms fsync, benchmarks.md H4) — affordable because a store write
//! only happens right after a fresh provider embedding call that already
//! cost hundreds of ms; reads and result puts never touch the store.

use std::path::Path;

use async_trait::async_trait;
use redb::{Database, ReadableDatabase, TableDefinition};

use cognigraph_core::SearchHit;

use crate::memory::InMemoryCache;
use crate::normalize::normalize_query;
use crate::traits::QueryCache;
use crate::types::{CacheConfig, CacheHit, CacheKey, CacheStats};

/// (model, normalized_query) → embedding as little-endian f64 bytes.
const EMBEDDINGS: TableDefinition<'_, (&str, &str), &[u8]> = TableDefinition::new("embeddings");

pub struct PersistentCache {
    memory: InMemoryCache,
    store: Database,
}

fn encode(embedding: &[f64]) -> Vec<u8> {
    embedding.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn decode(bytes: &[u8]) -> Vec<f64> {
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| f64::from_le_bytes(*chunk))
        .collect()
}

impl PersistentCache {
    /// Open (or create) the embedding store at `path`. Fails loudly on an
    /// unopenable file — a configured-but-broken cache path should not
    /// silently degrade to memory-only.
    pub fn open(config: CacheConfig, path: impl AsRef<Path>) -> Result<Self, String> {
        let store = Database::create(path.as_ref())
            .map_err(|e| format!("cannot open cache store {}: {e}", path.as_ref().display()))?;
        // Ensure the table exists so first reads don't error.
        let txn = store.begin_write().map_err(|e| e.to_string())?;
        txn.open_table(EMBEDDINGS).map_err(|e| e.to_string())?;
        txn.commit().map_err(|e| e.to_string())?;
        Ok(Self {
            memory: InMemoryCache::new(config),
            store,
        })
    }

    fn store_get(&self, model: &str, query: &str) -> Option<Vec<f64>> {
        let txn = self.store.begin_read().ok()?;
        let table = txn.open_table(EMBEDDINGS).ok()?;
        let value = table.get((model, query)).ok()??;
        Some(decode(value.value()))
    }

    fn store_put(&self, model: &str, query: &str, embedding: &[f64]) {
        let result: Result<(), redb::Error> = (|| {
            let txn = self.store.begin_write()?;
            txn.open_table(EMBEDDINGS)?
                .insert((model, query), encode(embedding).as_slice())?;
            txn.commit()?;
            Ok(())
        })();
        if let Err(e) = result {
            tracing::warn!("cache store write failed: {e}");
        }
    }

    fn store_clear(&self) {
        let result: Result<(), redb::Error> = (|| {
            let txn = self.store.begin_write()?;
            txn.delete_table(EMBEDDINGS)?;
            txn.open_table(EMBEDDINGS)?;
            txn.commit()?;
            Ok(())
        })();
        if let Err(e) = result {
            tracing::warn!("cache store clear failed: {e}");
        }
    }
}

#[async_trait]
impl QueryCache for PersistentCache {
    async fn get_embedding(&self, query: &str, model: &str) -> Option<Vec<f64>> {
        if let Some(hit) = self.memory.get_embedding(query, model).await {
            return Some(hit);
        }
        // Read-through: promote a stored embedding into memory. Stored
        // embeddings never expire (they are deterministic); promotion
        // gives the hot ones memory-speed on subsequent lookups.
        let normalized = normalize_query(query);
        let embedding = self.store_get(model, &normalized)?;
        self.memory
            .put_embedding(query, model, embedding.clone())
            .await;
        Some(embedding)
    }

    async fn put_embedding(&self, query: &str, model: &str, embedding: Vec<f64>) {
        self.store_put(model, &normalize_query(query), &embedding);
        self.memory.put_embedding(query, model, embedding).await;
    }

    async fn get_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<&[f64]>,
    ) -> Option<CacheHit> {
        self.memory.get_results(key, query_embedding).await
    }

    async fn put_results(
        &self,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<serde_json::Value>,
    ) {
        self.memory
            .put_results(key, query_embedding, results, meta)
            .await
    }

    fn result_generation(&self) -> u64 {
        self.memory.result_generation()
    }

    async fn put_results_if_generation(
        &self,
        expected_generation: u64,
        key: &CacheKey,
        query_embedding: Option<Vec<f64>>,
        results: Vec<SearchHit>,
        meta: Option<serde_json::Value>,
    ) -> bool {
        self.memory
            .put_results_if_generation(expected_generation, key, query_embedding, results, meta)
            .await
    }

    async fn invalidate(&self, key: &CacheKey) {
        self.memory.invalidate(key).await
    }

    async fn invalidate_collection(&self, collection: &str) {
        self.memory.invalidate_collection(collection).await
    }

    async fn invalidate_results(&self) {
        self.memory.invalidate_results().await
    }

    async fn clear(&self) {
        self.store_clear();
        self.memory.clear().await
    }

    fn stats(&self) -> &CacheStats {
        self.memory.stats()
    }

    async fn entry_count(&self) -> usize {
        self.memory.entry_count().await
    }

    fn config(&self) -> &CacheConfig {
        self.memory.config()
    }
}
