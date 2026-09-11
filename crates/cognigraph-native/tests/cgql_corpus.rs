//! Dual-engine equivalence: runs the shared CGQL exec corpus through
//! `NativeBackend::query()` and asserts the same expected results as the
//! in-memory executor run in `cognigraph-query/tests/corpus.rs`.
//!
//! The backend is seeded via `import_json` so the fixture documents stay
//! byte-identical (no create-time stamping).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
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

#[tokio::test]
async fn corpus_exec_matches_in_memory_engine() {
    let dataset: Value =
        serde_json::from_str(&fs::read_to_string(corpus_dir().join("dataset.json")).unwrap())
            .unwrap();
    let backend = NativeBackend::new();
    backend.import_json(&dataset).await.unwrap();

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
            "native backend diverged from expected results for {file:?}"
        );
    }
}
