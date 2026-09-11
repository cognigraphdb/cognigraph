use std::path::PathBuf;

use cognigraph_core::{CogniGraphError, GraphBackend, VectorSearchOpts};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::json;

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cognigraph-search-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(path.join("store")).unwrap();
        Self(path)
    }
    fn open(&self, mode: StorageMode) -> NativeBackend {
        NativeBackend::open_with_modes(
            self.0.join("store/data.redb"),
            VectorMode::Sidecar,
            mode,
            1 << 20,
        )
        .unwrap()
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn derivative_identity_is_safe_and_distinct_across_restarts() {
    for mode in [StorageMode::Resident, StorageMode::Paged] {
        let dir = TempDir::new();
        // Both traversal-shaped names and field delimiters are opaque data.
        let collection = "probe/../../escaped";
        {
            let backend = dir.open(mode);
            backend
                .create_document(
                    collection,
                    json!({
                        "_key": "a", "a,b": "combined", "a": "separate", "b": "split",
                        "embedding": [1.0, 0.0]
                    }),
                )
                .await
                .unwrap();
        }
        for _ in 0..2 {
            let backend = dir.open(mode);
            for (fields, found, absent) in [
                (vec!["a,b".into()], "combined", "separate"),
                (vec!["a".into(), "b".into()], "separate", "combined"),
            ] {
                let hits = backend
                    .text_search(collection, found, &fields, 10)
                    .await
                    .unwrap();
                assert_eq!(hits.len(), 1, "{mode:?}: {fields:?}");
                assert_eq!(hits[0].document["_key"], "a");
                assert!(
                    backend
                        .text_search(collection, absent, &fields, 10)
                        .await
                        .unwrap()
                        .is_empty()
                );
            }
            let hits = backend
                .vector_search(
                    collection,
                    &[1.0, 0.0],
                    &VectorSearchOpts {
                        threshold: None,
                        limit: 10,
                        model_name: None,
                    },
                )
                .await
                .unwrap();
            assert_eq!(hits.len(), 1);
        }
        assert_eq!(
            std::fs::read_dir(&dir.0).unwrap().count(),
            1,
            "nothing escaped store"
        );
        let paths: Vec<_> = std::fs::read_dir(dir.0.join("store"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(
            paths
                .iter()
                .filter(|p| p.extension().is_some_and(|e| e == "tantivy"))
                .count(),
            2
        );
        assert_eq!(
            paths
                .iter()
                .filter(|p| p.extension().is_some_and(|e| e == "vectors"))
                .count(),
            1
        );
        assert!(paths.iter().all(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("data.redb")
        }));
    }
}

#[tokio::test]
async fn reserved_and_duplicate_fields_return_validation_errors_in_every_storage_mode() {
    let dir = TempDir::new();
    for backend in [NativeBackend::new(), dir.open(StorageMode::Resident)] {
        assert_invalid_fields(&backend).await;
    }
    assert_invalid_fields(&dir.open(StorageMode::Paged)).await;
}

async fn assert_invalid_fields(backend: &NativeBackend) {
    backend
        .create_document("docs", json!({"content": "rust"}))
        .await
        .unwrap();
    for fields in [
        vec!["_key".into()],
        vec!["content".into(), "content".into()],
    ] {
        let err = backend
            .text_search("docs", "rust", &fields, 10)
            .await
            .unwrap_err();
        assert!(matches!(err, CogniGraphError::ValidationError(_)), "{err}");
    }
    assert!(
        !backend
            .text_search("docs", "rust", &["content".into()], 10)
            .await
            .unwrap()
            .is_empty()
    );
}
