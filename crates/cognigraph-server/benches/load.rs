//! Concurrent-load benchmark: spawns the REAL server binary (native
//! backend, auth/cache/rate-limit off) and drives it over TCP with
//! concurrent clients. Reports throughput and latency percentiles per
//! workload per concurrency level, for BOTH backend modes: in-memory and
//! persistent (redb write-through) — the persistent pass is H4, measuring
//! commit cost under concurrent writes (seed rate = sequential write
//! throughput; the pure-write and mixed cells expose it under load).
//! Run: cargo bench -p cognigraph-server --bench load

use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const DOCS: usize = 5_000;
const DIM: usize = 64;
const CONCURRENCY: [usize; 4] = [1, 8, 32, 128];
const CELL_SECS: f64 = 2.0;

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Deterministic pseudo-random vector, same generator family as the
/// native perf bench.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn unit(&mut self) -> f64 {
        (self.next() % 10_000) as f64 / 10_000.0 - 0.5
    }
}

fn vector(seed: u64) -> Vec<f64> {
    let mut rng = Lcg(seed);
    (0..DIM).map(|_| rng.unit()).collect()
}

fn main() {
    let scratch = std::env::temp_dir().join(format!("cg-load-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();

    run_mode("in-memory", &scratch, None);
    let redb = scratch.join("bench.redb");
    run_mode("persistent (redb)", &scratch, Some(redb.to_str().unwrap()));
    let _ = std::fs::remove_dir_all(&scratch);
}

fn run_mode(mode: &str, scratch: &std::path::Path, native_path: Option<&str>) {
    let port = free_port();
    let base = format!("http://127.0.0.1:{port}");

    // env_clear + a scratch cwd: the project .env must not leak in (it
    // selects the arango backend and API keys).
    let mut command = Command::new(env!("CARGO_BIN_EXE_cognigraph-server"));
    command
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("COGNIGRAPH_HOST", "127.0.0.1")
        .env("COGNIGRAPH_PORT", port.to_string())
        .env("COGNIGRAPH_BACKEND", "native")
        .env("RUST_LOG", "error")
        .current_dir(scratch)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(path) = native_path {
        command.env("COGNIGRAPH_NATIVE_PATH", path);
    }
    let child = command.spawn().expect("spawn server binary");
    let _guard = KillOnDrop(child);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(256)
            .build()
            .unwrap();

        // Wait for the server.
        let mut up = false;
        for _ in 0..100 {
            if client.get(format!("{base}/health")).send().await.is_ok() {
                up = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(up, "server did not come up on {base}");

        // Seed concurrently (64-way).
        let seed_started = Instant::now();
        let seeded = Arc::new(AtomicU64::new(0));
        let mut seeders = Vec::new();
        for worker in 0..64usize {
            let client = client.clone();
            let base = base.clone();
            let seeded = seeded.clone();
            seeders.push(tokio::spawn(async move {
                let mut i = worker;
                while i < DOCS {
                    let body = serde_json::json!({
                        "collection": "bench",
                        "_key": format!("k{i}"),
                        "title": format!("document {i}"),
                        "category": format!("c{}", i % 8),
                        "score": (i % 100) as f64 / 10.0,
                        "embedding": vector(i as u64),
                    });
                    let response = client
                        .post(format!("{base}/documents"))
                        .json(&body)
                        .send()
                        .await
                        .expect("seed request");
                    assert!(response.status().is_success(), "seed failed: {response:?}");
                    seeded.fetch_add(1, Ordering::Relaxed);
                    i += 64;
                }
            }));
        }
        for seeder in seeders {
            seeder.await.unwrap();
        }
        let seed_secs = seed_started.elapsed().as_secs_f64();
        println!(
            "=== mode: {mode} ===\nseeded {} docs (dim {DIM}) in {seed_secs:.1}s ({:.0} writes/s)\n",
            seeded.load(Ordering::Relaxed),
            seeded.load(Ordering::Relaxed) as f64 / seed_secs
        );
        println!(
            "{:<28} {:>5} {:>10} {:>9} {:>9} {:>9}",
            "workload", "conc", "req/s", "p50 ms", "p90 ms", "p99 ms"
        );

        for &concurrency in &CONCURRENCY {
            run_cell(&client, &base, "GET point read", concurrency, |n| {
                Request::Get(format!("/documents/bench/k{}", n as usize % DOCS))
            })
            .await;
        }
        for &concurrency in &CONCURRENCY {
            run_cell(&client, &base, "POST search/query (CGQL)", concurrency, |_| {
                Request::Post(
                    "/search/query".into(),
                    serde_json::json!({
                        "query": "FOR d IN bench FILTER d.category == \"c3\" LIMIT 10 RETURN d.title",
                    }),
                )
            })
            .await;
        }
        for &concurrency in &CONCURRENCY {
            run_cell(&client, &base, "POST search/vector", concurrency, |n| {
                Request::Post(
                    "/search/vector".into(),
                    serde_json::json!({
                        "collection": "bench",
                        "vector": vector(n),
                        "limit": 10,
                        "threshold": -1.0,
                    }),
                )
            })
            .await;
        }
        for &concurrency in &CONCURRENCY {
            run_cell(&client, &base, "POST insert (pure write)", concurrency, |n| {
                Request::Post(
                    "/documents".into(),
                    serde_json::json!({
                        "collection": "bench_inserts",
                        "value": n,
                    }),
                )
            })
            .await;
        }
        for &concurrency in &CONCURRENCY {
            run_cell(&client, &base, "POST batch (100 inserts)", concurrency, |n| {
                let ops: Vec<serde_json::Value> = (0..100)
                    .map(|i| {
                        serde_json::json!({
                            "op": "insert",
                            "collection": "bench_batch",
                            "doc": { "value": n * 100 + i },
                        })
                    })
                    .collect();
                Request::Post("/batch".into(), serde_json::json!({ "ops": ops }))
            })
            .await;
        }
        for &concurrency in &CONCURRENCY {
            run_cell(&client, &base, "mixed 90% read / 10% write", concurrency, |n| {
                if n % 10 == 0 {
                    Request::Post(
                        "/documents".into(),
                        serde_json::json!({
                            "collection": "bench_writes",
                            "value": n,
                        }),
                    )
                } else {
                    Request::Get(format!("/documents/bench/k{}", n as usize % DOCS))
                }
            })
            .await;
        }
    });
}

enum Request {
    Get(String),
    Post(String, serde_json::Value),
}

async fn run_cell(
    client: &reqwest::Client,
    base: &str,
    name: &str,
    concurrency: usize,
    make: fn(u64) -> Request,
) {
    let deadline = Instant::now() + Duration::from_secs_f64(CELL_SECS);
    let counter = Arc::new(AtomicU64::new(0));
    let mut workers = Vec::new();
    let started = Instant::now();
    for _ in 0..concurrency {
        let client = client.clone();
        let base = base.to_string();
        let counter = counter.clone();
        let name = name.to_string();
        workers.push(tokio::spawn(async move {
            let mut latencies: Vec<u64> = Vec::new();
            while Instant::now() < deadline {
                let n = counter.fetch_add(1, Ordering::Relaxed);
                let begin = Instant::now();
                let response = match make(n) {
                    Request::Get(path) => client.get(format!("{base}{path}")).send().await,
                    Request::Post(path, body) => {
                        client
                            .post(format!("{base}{path}"))
                            .json(&body)
                            .send()
                            .await
                    }
                };
                let status = response.expect("request failed").status();
                assert!(status.is_success(), "{name}: HTTP {status}");
                latencies.push(begin.elapsed().as_micros() as u64);
            }
            latencies
        }));
    }
    let mut latencies: Vec<u64> = Vec::new();
    for worker in workers {
        latencies.extend(worker.await.unwrap());
    }
    let elapsed = started.elapsed().as_secs_f64();
    latencies.sort_unstable();
    let pct = |p: f64| -> f64 {
        if latencies.is_empty() {
            return 0.0;
        }
        let index = ((latencies.len() as f64 - 1.0) * p).round() as usize;
        latencies[index] as f64 / 1000.0
    };
    println!(
        "{:<28} {:>5} {:>10.0} {:>9.3} {:>9.3} {:>9.3}",
        name,
        concurrency,
        latencies.len() as f64 / elapsed,
        pct(0.50),
        pct(0.90),
        pct(0.99),
    );
}
