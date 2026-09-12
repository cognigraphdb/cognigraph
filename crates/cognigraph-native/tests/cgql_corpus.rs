//! Runs the shared CGQL exec corpus through
//! `NativeBackend::query()` and asserts the same expected results as the
//! in-memory executor run in `cognigraph-query/tests/corpus.rs`. Persistent
//! modes run before and after reopen; these paths share the CGQL implementation.
//!
//! The backend is seeded via `import_json` so the fixture documents stay
//! byte-identical (no create-time stamping).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use cognigraph_core::GraphBackend;
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::Value;

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../cognigraph-query/tests/corpus")
}

fn binds_from_header(source: &str) -> HashMap<String, Value> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("// binds:"))
        .flat_map(|json| {
            serde_json::from_str::<serde_json::Map<String, Value>>(json)
                .expect("invalid `// binds:` header")
        })
        .collect()
}

async fn assert_corpus(backend: &NativeBackend, context: &str) {
    let dir = corpus_dir().join("exec");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("missing corpus dir {dir:?}: {e}"))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "cgql"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .cgql files in {dir:?}");

    for file in files {
        let source = fs::read_to_string(&file).unwrap();
        let binds = binds_from_header(&source);
        let expected_path = file.with_extension("json");
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(&expected_path)
                .unwrap_or_else(|e| panic!("missing expected output {expected_path:?}: {e}")),
        )
        .unwrap();
        let rows = backend
            .query(&source, binds)
            .await
            .unwrap_or_else(|e| panic!("{file:?} failed on the native backend: {e}"));
        assert_eq!(
            Value::Array(rows),
            expected,
            "{context}: native backend diverged from expected results for {file:?}"
        );
    }
}

async fn seed(backend: &NativeBackend) {
    let dataset: Value =
        serde_json::from_str(&fs::read_to_string(corpus_dir().join("dataset.json")).unwrap())
            .unwrap();
    backend.import_json(&dataset).await.unwrap();
}

#[tokio::test]
async fn corpus_exec_matches_in_memory_engine() {
    let backend = NativeBackend::new();
    seed(&backend).await;
    assert_corpus(&backend, "memory").await;
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

async fn persistent_corpus(vector: VectorMode, storage: StorageMode) {
    let dir =
        TempDir(std::env::temp_dir().join(format!("cognigraph-corpus-{}", uuid::Uuid::new_v4())));
    fs::create_dir(&dir.0).unwrap();
    let open =
        || NativeBackend::open_with_modes(dir.0.join("data.redb"), vector, storage, 1024).unwrap();
    let context = format!("{vector:?}/{storage:?}");
    {
        let backend = open();
        seed(&backend).await;
        assert_corpus(&backend, &format!("{context} before reopen")).await;
    }
    let reopened = open();
    assert_corpus(&reopened, &format!("{context} after reopen")).await;
}

#[tokio::test]
async fn corpus_persistent_embedded_survives_reopen() {
    persistent_corpus(VectorMode::Embedded, StorageMode::Resident).await;
}

#[tokio::test]
async fn corpus_persistent_sidecar_survives_reopen() {
    persistent_corpus(VectorMode::Sidecar, StorageMode::Resident).await;
}

#[tokio::test]
async fn corpus_paged_sidecar_survives_reopen() {
    persistent_corpus(VectorMode::Sidecar, StorageMode::Paged).await;
}
