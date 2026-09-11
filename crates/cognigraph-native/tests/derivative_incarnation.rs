use std::path::{Path, PathBuf};

use cognigraph_core::{GraphBackend, VectorSearchOpts};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::json;

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cg-incarnation-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open(path: &Path, mode: StorageMode) -> NativeBackend {
    NativeBackend::open_with_modes(path, VectorMode::Sidecar, mode, 1 << 20).unwrap()
}

async fn seed(backend: &NativeBackend, word: &str, vector: [f64; 2]) {
    backend
        .create_document("docs", json!({"_key":"a", "text":word, "embedding":vector}))
        .await
        .unwrap();
    // More distractors than the sidecar's minimum candidate set: a stale
    // vector must not be rescued accidentally by exact reranking every row.
    for n in 0..40 {
        backend
            .create_document(
                "docs",
                json!({"_key":format!("d{n:02}"), "embedding":[0.6,0.8]}),
            )
            .await
            .unwrap();
    }
}

async fn check(backend: &NativeBackend, present: &str, absent: &str, vector: [f64; 2]) {
    let fields = ["text".to_string()];
    let hits = backend
        .text_search("docs", present, &fields, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1, "current vocabulary missing: {present}");
    assert_eq!(hits[0].document["text"], present);
    assert!(
        backend
            .text_search("docs", absent, &fields, 10)
            .await
            .unwrap()
            .is_empty(),
        "previous vocabulary matched current data: {absent}"
    );
    let hits = backend
        .vector_search(
            "docs",
            &vector,
            &VectorSearchOpts {
                threshold: Some(0.95),
                limit: 1,
                model_name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 1, "current vector missing from candidates");
    assert_eq!(hits[0].document["_key"], "a");
}

#[tokio::test]
async fn recreated_database_does_not_reuse_same_generation_derivatives() {
    for mode in [StorageMode::Resident, StorageMode::Paged] {
        let dir = TempDir::new();
        let path = dir.0.join("tenant.redb");
        {
            let backend = open(&path, mode);
            seed(&backend, "oldvocabulary", [1.0, 0.0]).await;
            check(&backend, "oldvocabulary", "newvocabulary", [1.0, 0.0]).await;
        }
        // Leave every derivative in place, replacing only the synthetic DB.
        std::fs::remove_file(&path).unwrap();
        {
            let backend = open(&path, mode);
            seed(&backend, "newvocabulary", [0.0, 1.0]).await;
            check(&backend, "newvocabulary", "oldvocabulary", [0.0, 1.0]).await;
        }
        check(
            &open(&path, mode),
            "newvocabulary",
            "oldvocabulary",
            [0.0, 1.0],
        )
        .await;
    }
}

#[tokio::test]
async fn divergent_restore_does_not_reuse_same_database_and_generation_derivatives() {
    for mode in [StorageMode::Resident, StorageMode::Paged] {
        let dir = TempDir::new();
        let path = dir.0.join("tenant.redb");
        let backup = dir.0.join("backup.redb");
        {
            let backend = open(&path, mode);
            seed(&backend, "baseline", [1.0, 0.0]).await;
        }
        std::fs::copy(&path, &backup).unwrap();
        {
            let backend = open(&path, mode);
            backend
                .update_document(
                    "docs",
                    "a",
                    json!({"text":"oldbranch", "embedding":[1.0,0.0]}),
                )
                .await
                .unwrap();
            check(&backend, "oldbranch", "newbranch", [1.0, 0.0]).await;
        }
        // Same database UUID, same keys, same number of commits, different
        // data after restoring a closed physical backup and taking a new branch.
        std::fs::copy(&backup, &path).unwrap();
        {
            let backend = open(&path, mode);
            backend
                .update_document(
                    "docs",
                    "a",
                    json!({"text":"newbranch", "embedding":[0.0,1.0]}),
                )
                .await
                .unwrap();
            check(&backend, "newbranch", "oldbranch", [0.0, 1.0]).await;
        }
        check(&open(&path, mode), "newbranch", "oldbranch", [0.0, 1.0]).await;
    }
}
