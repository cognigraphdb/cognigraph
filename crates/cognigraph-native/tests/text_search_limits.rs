//! CG-109: text search must return an ordinary result or a validation error
//! for any limit and any field name, never a panic inside the index library.

use std::path::PathBuf;

use cognigraph_core::{CogniGraphError, GraphBackend};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::json;

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cognigraph-text-limits-{}", uuid::Uuid::new_v4()));
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

async fn seed(backend: &NativeBackend) {
    for (key, content) in [("a", "alpha beta"), ("b", "alpha gamma"), ("c", "delta")] {
        backend
            .create_document(
                "notes",
                json!({"_key": key, "content": content, "title": key}),
            )
            .await
            .unwrap();
    }
}

async fn assert_limits(backend: &NativeBackend, context: &str) {
    let fields = vec!["content".to_string()];
    for limit in [
        usize::MAX,
        usize::MAX / 2 + 1,
        1_000_000_000_000,
        u32::MAX as usize,
        1_000,
        3,
        2,
    ] {
        let hits = backend
            .text_search("notes", "alpha", &fields, limit)
            .await
            .unwrap_or_else(|error| panic!("{context}: limit {limit}: {error}"));
        assert_eq!(hits.len(), 2, "{context}: limit {limit}");
    }
    // A limit below the match count still truncates.
    let one = backend
        .text_search("notes", "alpha", &fields, 1)
        .await
        .unwrap();
    assert_eq!(one.len(), 1, "{context}");
    // Zero has always meant "at least one candidate"; it must not panic either.
    assert!(
        backend
            .text_search("notes", "alpha", &fields, 0)
            .await
            .is_ok(),
        "{context}"
    );
    // No match, and a collection with nothing indexed.
    let none = backend
        .text_search("notes", "absent", &fields, usize::MAX)
        .await
        .unwrap();
    assert!(none.is_empty(), "{context}");
}

async fn assert_field_names(backend: &NativeBackend, context: &str) {
    for fields in [
        vec![String::new()],
        vec!["-title".to_string()],
        vec!["-".to_string()],
        vec!["content".to_string(), String::new()],
        vec!["content".to_string(), "-content".to_string()],
        vec!["_key".to_string()],
        vec!["content".to_string(), "content".to_string()],
    ] {
        let error = backend
            .text_search("notes", "alpha", &fields, 10)
            .await
            .expect_err(&format!("{context}: {fields:?} must be refused"));
        assert!(
            matches!(error, CogniGraphError::ValidationError(_)),
            "{context}: {fields:?}: {error}"
        );
    }
    // Names that only look unusual are fine.
    for fields in [
        vec!["title-".to_string()],
        vec![" content".to_string()],
        vec!["content".to_string(), "missing_field".to_string()],
        vec!["заголовок".to_string()],
    ] {
        backend
            .text_search("notes", "alpha", &fields, 10)
            .await
            .unwrap_or_else(|error| panic!("{context}: {fields:?}: {error}"));
    }
}

#[tokio::test]
async fn extreme_limits_and_invalid_field_names_never_reach_the_index_library() {
    let memory = NativeBackend::new();
    seed(&memory).await;
    assert_limits(&memory, "memory").await;
    assert_field_names(&memory, "memory").await;

    for mode in [StorageMode::Resident, StorageMode::Paged] {
        let dir = TempDir::new();
        let backend = dir.open(mode);
        seed(&backend).await;
        let context = format!("{mode:?}");
        assert_limits(&backend, &context).await;
        assert_field_names(&backend, &context).await;
        drop(backend);
        let reopened = dir.open(mode);
        let context = format!("{mode:?} after restart");
        assert_field_names(&reopened, &context).await;
        assert_limits(&reopened, &context).await;
    }
}

#[tokio::test]
async fn an_empty_collection_accepts_any_limit() {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("empty", cognigraph_core::CollectionType::Document)
        .await
        .unwrap();
    let hits = backend
        .text_search("empty", "alpha", &["content".to_string()], usize::MAX)
        .await
        .unwrap();
    assert!(hits.is_empty());
}
