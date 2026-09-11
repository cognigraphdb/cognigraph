//! Persistent cache backend costs: memory hit vs store read-through vs
//! store put (fsync). Run: cargo bench -p cognigraph-cache

use std::time::Instant;

use cognigraph_cache::{CacheConfig, PersistentCache, QueryCache};

const DIM: usize = 1536; // text-embedding-3-small
const N: usize = 500;

fn main() {
    let dir = std::env::temp_dir().join(format!("cg-cache-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cache.redb");
    let embedding: Vec<f64> = (0..DIM).map(|i| i as f64 / DIM as f64).collect();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let config = CacheConfig {
            enabled: true,
            max_entries: N * 2,
            ..CacheConfig::default()
        };

        // Store put: one Immediate (fsync) commit per new embedding.
        let cache = PersistentCache::open(config.clone(), &path).unwrap();
        let started = Instant::now();
        for i in 0..N {
            cache
                .put_embedding(&format!("query {i}"), "bench-model", embedding.clone())
                .await;
        }
        let put = started.elapsed().as_secs_f64() * 1000.0 / N as f64;

        // Memory hit: the entry was promoted at put time.
        let started = Instant::now();
        for i in 0..N {
            assert!(
                cache
                    .get_embedding(&format!("query {i}"), "bench-model")
                    .await
                    .is_some()
            );
        }
        let memory_hit = started.elapsed().as_secs_f64() * 1000.0 / N as f64;

        // Cold read-through: fresh instance, memory empty, store full.
        drop(cache);
        let cache = PersistentCache::open(config, &path).unwrap();
        let started = Instant::now();
        for i in 0..N {
            assert!(
                cache
                    .get_embedding(&format!("query {i}"), "bench-model")
                    .await
                    .is_some()
            );
        }
        let read_through = started.elapsed().as_secs_f64() * 1000.0 / N as f64;

        println!("persistent cache, dim {DIM}, {N} entries (ms/op):");
        println!("  put_embedding (fsync commit)   {put:.3}");
        println!("  get_embedding (memory hit)     {memory_hit:.4}");
        println!("  get_embedding (store cold)     {read_through:.4}");
    });

    let _ = std::fs::remove_dir_all(&dir);
}
