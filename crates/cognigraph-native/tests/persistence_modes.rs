//! Sidecar vector mode and paged storage mode tests for the persistent
//! native backend.

use cognigraph_core::{
    CollectionType, Direction, FieldPredicate, GraphBackend, PredicateOp, TraversalOpts,
};
use cognigraph_native::NativeBackend;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;

/// Unique temp directory, removed on drop. Deliberately dependency-free.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cognigraph-native-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn db_path(&self) -> PathBuf {
        self.0.join("data.redb")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn sidecar_mode_full_lifecycle() {
    use cognigraph_core::VectorSearchOpts;
    use cognigraph_native::VectorMode;
    let dir = TempDir::new();

    // Deterministic vectors; keep a copy for the brute-force reference.
    let mut vectors = Vec::new();
    {
        let backend = NativeBackend::open_with_mode(dir.db_path(), VectorMode::Sidecar).unwrap();
        backend
            .ensure_collection("vecs", CollectionType::Document)
            .await
            .unwrap();
        let mut seed = 7u64;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        };
        for i in 0..200 {
            let v: Vec<f64> = (0..32).map(|_| next()).collect();
            backend
                .create_document(
                    "vecs",
                    json!({ "_key": format!("v{i:03}"), "label": i, "embedding": v }),
                )
                .await
                .unwrap();
            vectors.push(v);
        }

        // Search-only embeddings: documents come back stripped.
        let doc = backend.get_document("vecs", "v000").await.unwrap().unwrap();
        assert!(doc.get("embedding").is_none());
        assert_eq!(doc["label"], json!(0));

        // A partial update must NOT destroy the vector (truth merge).
        backend
            .update_document("vecs", "v000", json!({ "reviewed": true }))
            .await
            .unwrap();
    }

    // Reopen: warm-start from the mmap'd sidecar (same generation).
    let backend = NativeBackend::open_with_mode(dir.db_path(), VectorMode::Sidecar).unwrap();
    let query: Vec<f64> = (0..32).map(|i| ((i as f64) / 32.0) - 0.5).collect();

    fn cosine(a: &[f64], b: &[f64]) -> f64 {
        let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f64 = a.iter().map(|v| v * v).sum::<f64>().sqrt();
        let nb: f64 = b.iter().map(|v| v * v).sum::<f64>().sqrt();
        dot / (na * nb)
    }
    let mut expected: Vec<(usize, f64)> = vectors
        .iter()
        .enumerate()
        .map(|(i, v)| (i, cosine(&query, v)))
        .collect();
    expected.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let opts = VectorSearchOpts {
        threshold: None,
        limit: 5,
        model_name: None,
    };
    let hits = backend.vector_search("vecs", &query, &opts).await.unwrap();
    assert_eq!(hits.len(), 5);
    for (hit, (index, score)) in hits.iter().zip(expected.iter().take(5)) {
        assert_eq!(hit.document["_key"], json!(format!("v{index:03}")));
        assert!((hit.score - score).abs() < 1e-9, "exact scores from truth");
        assert!(hit.document.get("embedding").is_none());
    }

    // v000's vector survived the partial update.
    assert!(hits.iter().any(|_| true));
    let snapshot = backend.export_json().await.unwrap();
    let v000 = &snapshot["collections"]["vecs"]["documents"]["v000"];
    assert_eq!(v000["reviewed"], json!(true));
    assert_eq!(
        v000["embedding"].as_array().unwrap().len(),
        32,
        "truth export keeps embeddings"
    );

    // Writes invalidate: a new aligned vector becomes the top hit.
    backend
        .create_document(
            "vecs",
            json!({ "_key": "fresh", "embedding": query.clone() }),
        )
        .await
        .unwrap();
    let hits = backend.vector_search("vecs", &query, &opts).await.unwrap();
    assert_eq!(hits[0].document["_key"], json!("fresh"));
    assert!((hits[0].score - 1.0).abs() < 1e-9);
}

#[tokio::test]
async fn sidecar_delta_avoids_rebuilds() {
    use cognigraph_core::VectorSearchOpts;
    use cognigraph_native::VectorMode;
    let dir = TempDir::new();
    let backend = NativeBackend::open_with_mode(dir.db_path(), VectorMode::Sidecar).unwrap();
    backend
        .ensure_collection("vecs", CollectionType::Document)
        .await
        .unwrap();
    for i in 0..100 {
        let angle = i as f64 / 100.0;
        backend
            .create_document(
                "vecs",
                json!({ "_key": format!("v{i:03}"), "embedding": [angle.cos(), angle.sin()] }),
            )
            .await
            .unwrap();
    }
    let opts = VectorSearchOpts {
        threshold: None,
        limit: 3,
        model_name: None,
    };
    backend
        .vector_search("vecs", &[1.0, 0.0], &opts)
        .await
        .unwrap();
    assert_eq!(backend.sidecar_rebuild_count(), 1, "one initial build");

    // Steady writes: create, update embedding, delete — searches stay
    // correct with NO further rebuilds (delta path).
    backend
        .create_document("vecs", json!({ "_key": "new", "embedding": [1.0, 0.0] }))
        .await
        .unwrap();
    let hits = backend
        .vector_search("vecs", &[1.0, 0.0], &opts)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("new"));

    backend
        .update_document("vecs", "new", json!({ "embedding": [0.0, 1.0] }))
        .await
        .unwrap();
    let hits = backend
        .vector_search("vecs", &[0.0, 1.0], &opts)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("new"));

    backend.delete_document("vecs", "new").await.unwrap();
    let hits = backend
        .vector_search("vecs", &[0.0, 1.0], &opts)
        .await
        .unwrap();
    assert_ne!(hits[0].document["_key"], json!("new"));

    assert_eq!(
        backend.sidecar_rebuild_count(),
        1,
        "delta absorbed all writes without a rebuild"
    );

    // Past the 10% threshold (>= 65 delta entries on a 101-key base...
    // base is 101 keys -> threshold max(64, 10) = 64), a rebuild triggers.
    for i in 0..70 {
        backend
            .create_document(
                "vecs",
                json!({ "_key": format!("extra{i:03}"), "embedding": [0.5, 0.5] }),
            )
            .await
            .unwrap();
    }
    backend
        .vector_search("vecs", &[1.0, 0.0], &opts)
        .await
        .unwrap();
    assert_eq!(backend.sidecar_rebuild_count(), 2, "threshold rebuild");
}

#[tokio::test]
async fn paged_mode_conformance_and_lifecycle() {
    use cognigraph_core::VectorSearchOpts;
    use cognigraph_native::{StorageMode, VectorMode};
    let dir = TempDir::new();

    // Paged requires sidecar vectors.
    assert!(
        NativeBackend::open_with_modes(
            dir.db_path(),
            VectorMode::Embedded,
            StorageMode::Paged,
            1 << 20
        )
        .is_err()
    );

    let open = || {
        NativeBackend::open_with_modes(
            dir.db_path(),
            VectorMode::Sidecar,
            StorageMode::Paged,
            1 << 20,
        )
        .unwrap()
    };
    {
        let backend = open();
        // The full shared conformance suite must hold in paged mode.
        cognigraph_core::contract::run_all(&backend, "paged_contract").await;

        backend
            .ensure_collection("docs", CollectionType::Document)
            .await
            .unwrap();
        backend
            .ensure_collection("rels", CollectionType::Edge)
            .await
            .unwrap();
        for i in 0..50 {
            let angle = i as f64 / 50.0;
            backend
                .create_document(
                    "docs",
                    json!({
                        "_key": format!("d{i:02}"),
                        "title": format!("doc {i} rust"),
                        "n": i,
                        "embedding": [angle.cos(), angle.sin()]
                    }),
                )
                .await
                .unwrap();
        }
        backend
            .create_edge(
                "rels",
                json!({ "_from": "docs/d00", "_to": "docs/d01", "relation_type": "r", "confidence": 0.9 }),
            )
            .await
            .unwrap();
    }

    // Reopen paged: only keys resident; everything works from redb.
    let backend = open();
    let doc = backend.get_document("docs", "d07").await.unwrap().unwrap();
    assert_eq!(doc["n"], json!(7));
    assert!(doc.get("embedding").is_none(), "sidecar strip in paged");

    // Filter offset/limit are applied while redb is scanned, and only the
    // bounded matching page is retained and projected.
    let filtered = backend
        .list_documents_filtered(
            "docs",
            &[FieldPredicate {
                path: vec!["n".into()],
                op: PredicateOp::Ge,
                value: json!(10),
            }],
            Some(&["n".into()]),
            Some(3),
            Some(2),
        )
        .await
        .unwrap();
    assert_eq!(
        filtered,
        vec![
            json!({"_id": "docs/d12", "_key": "d12", "n": 12}),
            json!({"_id": "docs/d13", "_key": "d13", "n": 13}),
            json!({"_id": "docs/d14", "_key": "d14", "n": 14}),
        ]
    );

    // CGQL over paged scans (limit pushdown mid-scan) + COLLECT.
    let rows = backend
        .query(
            "FOR d IN docs FILTER d.n < 5 SORT d.n ASC RETURN d.n",
            HashMap::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 5);

    // BM25 (persistent tantivy) + vector (sidecar) + traversal.
    let hits = backend
        .text_search("docs", "rust", &["title".to_string()], 5)
        .await
        .unwrap();
    assert_eq!(hits.len(), 5);
    let hits = backend
        .vector_search(
            "docs",
            &[1.0, 0.0],
            &VectorSearchOpts {
                threshold: None,
                limit: 3,
                model_name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("d00"));
    let paths = backend
        .traverse(
            "docs/d00",
            &TraversalOpts {
                max_depth: 1,
                min_depth: 1,
                direction: Direction::Outbound,
                edge_collection: "rels".into(),
                min_confidence: None,
                path_decay: 0.8,
            },
        )
        .await
        .unwrap();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].vertices[1]["_key"], json!("d01"));

    // Partial update via redb truth; delete via keys; conflict via keys.
    let updated = backend
        .update_document("docs", "d07", json!({ "seen": true }))
        .await
        .unwrap();
    assert_eq!(updated["title"], json!("doc 7 rust"));
    assert!(
        backend
            .create_document("docs", json!({ "_key": "d07" }))
            .await
            .is_err()
    );
    assert!(backend.delete_document("docs", "d07").await.unwrap());
    assert!(backend.get_document("docs", "d07").await.unwrap().is_none());
}
