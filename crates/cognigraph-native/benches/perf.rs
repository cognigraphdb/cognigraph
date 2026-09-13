//! Dependency-free micro-benchmarks for the native backend hot paths.
//!
//! Run: cargo bench -p cognigraph-native
//! Prints median / p90 of wall-clock time per operation. Deterministic
//! dataset (LCG-generated embeddings) so runs are comparable.

use std::time::Instant;

use cognigraph_core::{CollectionType, Direction, GraphBackend, TraversalOpts, VectorSearchOpts};
use cognigraph_native::NativeBackend;
use serde_json::json;

const DOCS: usize = 10_000;
const DIM: usize = 128;
const ITERS: usize = 30;

/// Deterministic pseudo-random floats.
struct Lcg(u64);
impl Lcg {
    fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

fn bench<F: FnMut()>(name: &str, mut f: F) {
    // Warmup
    f();
    let mut samples: Vec<f64> = (0..ITERS)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{name:<38} median {:>9.3} ms   p90 {:>9.3} ms",
        samples[ITERS / 2],
        samples[(ITERS * 9) / 10]
    );
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let backend = NativeBackend::new();
    rt.block_on(backend.ensure_collection("documents", CollectionType::Document))
        .unwrap();
    rt.block_on(backend.ensure_collection("relationships", CollectionType::Edge))
        .unwrap();

    let mut rng = Lcg(42);
    let words = [
        "graph",
        "vector",
        "rust",
        "search",
        "knowledge",
        "engine",
        "fast",
        "data",
    ];
    for i in 0..DOCS {
        let embedding: Vec<f64> = (0..DIM).map(|_| rng.next_f64()).collect();
        let content = format!(
            "{} {} {} document number {i}",
            words[i % 8],
            words[(i / 3) % 8],
            words[(i / 7) % 8]
        );
        rt.block_on(backend.create_document(
            "documents",
            json!({
                    "_key": format!("d{i}"),
                    "title": format!("Doc {i}"),
                    "category": words[i % 8],
                    "score": (i % 100) as f64 / 10.0,
                    "content": content,
                    "embedding": embedding
            }),
        ))
        .unwrap();
    }
    // Chain plus fan-out edges: d_i -> d_{i+1}, d_i -> d_{i*2 mod N}
    for i in 0..DOCS - 1 {
        rt.block_on(backend.create_edge(
            "relationships",
            json!({
                    "_from": format!("documents/d{i}"),
                    "_to": format!("documents/d{}", i + 1),
                    "relation_type": "next",
                    "confidence": 0.9
            }),
        ))
        .unwrap();
    }
    println!("dataset: {DOCS} docs, dim {DIM}, {} edges\n", DOCS - 1);

    let handle = rt.handle().clone();
    let query_vec: Vec<f64> = {
        let mut r = Lcg(7);
        (0..DIM).map(|_| r.next_f64()).collect()
    };

    bench("vector_search top-10 (10k x 128)", || {
        let hits = handle
            .block_on(backend.vector_search(
                "documents",
                &query_vec,
                &VectorSearchOpts {
                    threshold: None,
                    limit: 10,
                    model_name: None,
                },
            ))
            .unwrap();
        assert_eq!(hits.len(), 10);
    });

    let fields = vec!["title".to_string(), "content".to_string()];
    bench("text_search BM25 (10k docs)", || {
        let hits = handle
            .block_on(backend.text_search("documents", "rust knowledge engine", &fields, 10))
            .unwrap();
        assert!(!hits.is_empty());
    });

    bench("traverse depth 1..3 (chain)", || {
        let paths = handle
            .block_on(backend.traverse(
                "documents/d0",
                &TraversalOpts {
                    max_depth: 3,
                    min_depth: 1,
                    direction: Direction::Outbound,
                    edge_collection: "relationships".into(),
                    min_confidence: None,
                    path_decay: 0.8,
                },
            ))
            .unwrap();
        assert_eq!(paths.len(), 3);
    });

    // upsert_edge must find an existing (from, to, relation_type) triple;
    // updating one edge in a 10k-edge collection exercises that lookup.
    bench("upsert_edge update (10k edges)", || {
        let edge = handle
            .block_on(backend.upsert_edge(
                "relationships",
                "documents/d100",
                "documents/d101",
                "next",
                json!({"confidence": 0.95}),
            ))
            .unwrap();
        assert_eq!(edge["_from"], "documents/d100");
    });

    bench("CGQL filter+sort+limit (10k scan)", || {
        let rows = handle
            .block_on(backend.query(
                "FOR d IN documents FILTER d.category == \"rust\" SORT d.score DESC LIMIT 10 RETURN d.title",
                std::collections::HashMap::new(),
            ))
            .unwrap();
        assert_eq!(rows.len(), 10);
    });

    // IN-predicate pushdown: membership filters run by reference in the
    // backend scan.
    bench("CGQL IN filter pushdown (10k)", || {
        let rows = handle
            .block_on(backend.query(
                "FOR d IN documents FILTER d.category IN [\"rust\", \"graph\"] SORT d._key ASC LIMIT 10 RETURN d.title",
                std::collections::HashMap::new(),
            ))
            .unwrap();
        assert_eq!(rows.len(), 10);
    });

    // Full pushdown: filter AND limit reach the backend (no sort), so the
    // scan stops after 10 matching rows without cloning anything else.
    bench("CGQL filter+limit full pushdown", || {
        let rows = handle
            .block_on(backend.query(
                "FOR d IN documents FILTER d.category == \"rust\" AND d.score >= 5 LIMIT 10 RETURN d.title",
                std::collections::HashMap::new(),
            ))
            .unwrap();
        assert_eq!(rows.len(), 10);
    });

    // Sidecar mode: same dataset persisted, mmap'd int8 scanning.
    {
        let dir = std::env::temp_dir().join(format!("cg-bench-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = dir.join("bench.redb");
        let sidecar_backend = rt
            .block_on(async {
                let b = NativeBackend::open_with_mode(&db, cognigraph_native::VectorMode::Sidecar)?;
                b.ensure_collection("documents", CollectionType::Document)
                    .await?;
                let mut rng = Lcg(42);
                for i in 0..DOCS {
                    let embedding: Vec<f64> = (0..DIM).map(|_| rng.next_f64()).collect();
                    b.create_document(
                        "documents",
                        json!({ "_key": format!("d{i:05}"), "embedding": embedding }),
                    )
                    .await?;
                }
                // Prime (builds + mmaps the sidecar file).
                b.vector_search(
                    "documents",
                    &[0.5; DIM],
                    &VectorSearchOpts {
                        threshold: None,
                        limit: 1,
                        model_name: None,
                    },
                )
                .await?;
                Ok::<_, cognigraph_core::CogniGraphError>(b)
            })
            .unwrap();
        bench("vector_search sidecar mmap (10k x 128)", || {
            let hits = handle
                .block_on(sidecar_backend.vector_search(
                    "documents",
                    &query_vec,
                    &VectorSearchOpts {
                        threshold: None,
                        limit: 10,
                        model_name: None,
                    },
                ))
                .unwrap();
            assert_eq!(hits.len(), 10);
        });
        let sidecar_file = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.file_name().to_string_lossy().ends_with(".vectors"))
            .map(|e| e.metadata().unwrap().len())
            .unwrap_or(0);
        println!(
            "
memory: f64-in-RAM eliminated {:.1} MB -> sidecar file {:.1} MB (paged i8)",
            (DOCS * DIM * 8) as f64 / 1e6,
            sidecar_file as f64 / 1e6
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- Quantization quality + speed matrix ------------------------------
    {
        // Keep the raw vectors for an exact brute-force reference.
        let raw: Vec<(String, Vec<f64>)> = {
            let mut rng = Lcg(42);
            (0..DOCS)
                .map(|i| (format!("d{i}"), (0..DIM).map(|_| rng.next_f64()).collect()))
                .collect()
        };
        let cosine = |a: &[f64], b: &[f64]| -> f64 {
            let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            let na: f64 = a.iter().map(|v| v * v).sum::<f64>().sqrt();
            let nb: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
            dot / (na * nb)
        };
        let exact_top10 = |q: &[f64]| -> Vec<String> {
            let mut scored: Vec<(f64, &String)> =
                raw.iter().map(|(k, v)| (cosine(q, v), k)).collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            scored.iter().take(10).map(|(_, k)| (*k).clone()).collect()
        };

        let mut qrng = Lcg(1234);
        let queries: Vec<Vec<f64>> = (0..20)
            .map(|_| (0..DIM).map(|_| qrng.next_f64()).collect())
            .collect();

        bench("vector exact brute force (reference)", || {
            let _ = exact_top10(&query_vec);
        });

        let mut matched = 0usize;
        let mut total = 0usize;
        for q in &queries {
            let expected = exact_top10(q);
            let hits = handle
                .block_on(backend.vector_search(
                    "documents",
                    q,
                    &VectorSearchOpts {
                        threshold: None,
                        limit: 10,
                        model_name: None,
                    },
                ))
                .unwrap();
            for hit in hits {
                if expected.contains(&hit.document["_key"].as_str().unwrap().to_string()) {
                    matched += 1;
                }
                total += 1;
            }
        }
        println!(
            "
quantized recall@10 vs exact: {:.2}% over {} queries",
            matched as f64 * 100.0 / total as f64,
            queries.len()
        );

        // Hybrid matrix: RRF(text BM25, vector) top-10, per vector engine.
        let fields = vec!["title".to_string(), "content".to_string()];
        let rrf = |text: &[String], vector: &[String]| -> Vec<String> {
            let mut scores: std::collections::HashMap<String, f64> =
                std::collections::HashMap::new();
            for (rank, k) in text.iter().enumerate() {
                *scores.entry(k.clone()).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
            }
            for (rank, k) in vector.iter().enumerate() {
                *scores.entry(k.clone()).or_default() += 1.0 / (60.0 + rank as f64 + 1.0);
            }
            let mut fused: Vec<(f64, String)> = scores.into_iter().map(|(k, s)| (s, k)).collect();
            fused.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap().then_with(|| a.1.cmp(&b.1)));
            fused.into_iter().take(10).map(|(_, k)| k).collect()
        };
        let text_keys: Vec<String> = handle
            .block_on(backend.text_search("documents", "rust knowledge engine", &fields, 30))
            .unwrap()
            .into_iter()
            .map(|h| h.document["_key"].as_str().unwrap().to_string())
            .collect();
        let reference_hybrid = rrf(&text_keys, &exact_top10(&query_vec));
        let quant_keys: Vec<String> = handle
            .block_on(backend.vector_search(
                "documents",
                &query_vec,
                &VectorSearchOpts {
                    threshold: None,
                    limit: 10,
                    model_name: None,
                },
            ))
            .unwrap()
            .into_iter()
            .map(|h| h.document["_key"].as_str().unwrap().to_string())
            .collect();
        let quant_hybrid = rrf(&text_keys, &quant_keys);
        let agreement = quant_hybrid
            .iter()
            .filter(|k| reference_hybrid.contains(k))
            .count();
        println!("hybrid matrix (RRF text+vector, top-10):");
        println!("  exact vector + tantivy BM25   -> reference");
        println!(
            "  quantized vector + tantivy    -> agreement {}/10",
            agreement
        );
        bench("hybrid quantized+tantivy end-to-end", || {
            let t: Vec<String> = handle
                .block_on(backend.text_search("documents", "rust knowledge engine", &fields, 30))
                .unwrap()
                .into_iter()
                .map(|h| h.document["_key"].as_str().unwrap().to_string())
                .collect();
            let v: Vec<String> = handle
                .block_on(backend.vector_search(
                    "documents",
                    &query_vec,
                    &VectorSearchOpts {
                        threshold: None,
                        limit: 10,
                        model_name: None,
                    },
                ))
                .unwrap()
                .into_iter()
                .map(|h| h.document["_key"].as_str().unwrap().to_string())
                .collect();
            let _ = rrf(&t, &v);
        });
    }

    bench("CGQL COLLECT aggregate (10k rows)", || {
        let rows = handle
            .block_on(backend.query(
                "FOR d IN documents COLLECT c = d.category AGGREGATE avg = AVG(d.score) WITH COUNT INTO n RETURN [c, avg, n]",
                std::collections::HashMap::new(),
            ))
            .unwrap();
        assert_eq!(rows.len(), 8);
    });

    // High-cardinality grouping: every title is distinct, so linear group
    // probing is O(rows x groups) — the case hash grouping exists for.
    bench("CGQL COLLECT high cardinality (10k)", || {
        let rows = handle
            .block_on(backend.query(
                "FOR d IN documents COLLECT t = d.title WITH COUNT INTO n RETURN n",
                std::collections::HashMap::new(),
            ))
            .unwrap();
        assert_eq!(rows.len(), 10_000);
    });

    bench("CGQL RETURN DISTINCT (10k)", || {
        let rows = handle
            .block_on(backend.query(
                "FOR d IN documents RETURN DISTINCT d.title",
                std::collections::HashMap::new(),
            ))
            .unwrap();
        assert_eq!(rows.len(), 10_000);
    });
}
