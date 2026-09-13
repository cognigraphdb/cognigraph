//! End-to-end test over the classic "State of the Union" corpus
//! (fixtures/sotu.txt, 2022 address — US government work, public domain).
//!
//! Ingests paragraphs into a `sotu` collection with deterministic synthetic
//! embeddings, then exercises BM25, vector search, and complex CGQL over
//! real prose.

use cognigraph_core::{CollectionType, GraphBackend, VectorSearchOpts};
use cognigraph_native::NativeBackend;
use serde_json::json;
use std::collections::HashMap;

const DIM: usize = 16;

/// Deterministic bag-of-bytes embedding — synthetic, but self-similar and
/// stable, which is all vector-path testing needs.
fn synthetic_embedding(text: &str) -> Vec<f64> {
    let mut buckets = [0.0f64; DIM];
    for (i, b) in text.bytes().enumerate() {
        buckets[(b as usize + i) % DIM] += f64::from(b) / 255.0;
    }
    let norm = buckets.iter().map(|v| v * v).sum::<f64>().sqrt().max(1e-9);
    buckets.iter().map(|v| v / norm).collect()
}

async fn ingest() -> (NativeBackend, usize) {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sotu.txt"
    ))
    .expect("fixtures/sotu.txt missing");
    let backend = NativeBackend::new();
    backend
        .ensure_collection("sotu", CollectionType::Document)
        .await
        .unwrap();

    let mut count = 0usize;
    for (i, para) in text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .enumerate()
    {
        backend
            .create_document(
                "sotu",
                json!({
                    "_key": format!("p{i:04}"),
                    "seq": i,
                    "text": para,
                    "word_count": para.split_whitespace().count(),
                    "embedding": synthetic_embedding(para),
                }),
            )
            .await
            .unwrap();
        count += 1;
    }
    (backend, count)
}

#[tokio::test]
async fn sotu_ingest_and_query() {
    let (backend, count) = ingest().await;
    assert!(count > 50, "expected a real corpus, got {count} paragraphs");

    // BM25 over real prose: the 2022 address features Ukraine heavily.
    let fields = vec!["text".to_string()];
    let hits = backend
        .text_search("sotu", "ukraine putin", &fields, 5)
        .await
        .unwrap();
    assert!(!hits.is_empty());
    assert!(
        hits[0].document["text"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("ukrain")
            || hits[0].document["text"]
                .as_str()
                .unwrap()
                .to_lowercase()
                .contains("putin")
    );

    // Vector search: a paragraph's own embedding must return itself first.
    let probe = backend
        .get_document("sotu", "p0010")
        .await
        .unwrap()
        .unwrap();
    let probe_vec: Vec<f64> = probe["embedding"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let hits = backend
        .vector_search(
            "sotu",
            &probe_vec,
            &VectorSearchOpts {
                threshold: None,
                limit: 3,
                model_name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("p0010"));
    assert!((hits[0].score - 1.0).abs() < 1e-9);

    // Complex CGQL: LET + CONTAINS filter + COLLECT AGGREGATE over prose.
    let rows = backend
        .query(
            r#"
            FOR p IN sotu
            LET mentions_ukraine = CONTAINS(LOWER(p.text), "ukrain")
            FILTER mentions_ukraine
            COLLECT hit = mentions_ukraine
            AGGREGATE words = SUM(p.word_count)
            WITH COUNT INTO paragraphs
            RETURN { paragraphs: paragraphs, words: words }
            "#,
            HashMap::new(),
        )
        .await
        .unwrap();
    let stats = &rows[0];
    assert!(stats["paragraphs"].as_u64().unwrap() >= 3);
    assert!(stats["words"].as_f64().unwrap() > 100.0);

    // Longest paragraphs via multi-key sort; whole-corpus word count.
    let rows = backend
        .query(
            "FOR p IN sotu SORT p.word_count DESC, p.seq ASC LIMIT 3 RETURN p.word_count",
            HashMap::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows[0].as_u64().unwrap() >= rows[2].as_u64().unwrap());
}
