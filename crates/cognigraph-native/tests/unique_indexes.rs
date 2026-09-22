//! CG-86: unique constraints persist with the store, hold across reopen in
//! every persistent mode, survive snapshot round trips, and admit exactly
//! one writer under concurrency.

use std::sync::Arc;

use cognigraph_core::{
    BatchOp, CogniGraphError, CollectionType, GraphBackend, IndexDef, IndexType,
};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::json;

/// Unique temp directory, removed on drop. Dependency-free like the other
/// persistence tests.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cognigraph-native-unique-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn email_index() -> IndexDef {
    IndexDef {
        index_type: IndexType::Persistent,
        fields: vec!["email".into()],
        unique: true,
        sparse: false,
        name: None,
    }
}

fn open(path: &std::path::Path, mode: &str) -> NativeBackend {
    match mode {
        "resident-embedded" => NativeBackend::open(path).unwrap(),
        "resident-sidecar" => {
            NativeBackend::open_with_modes(path, VectorMode::Sidecar, StorageMode::Resident, 0)
                .unwrap()
        }
        "paged-sidecar" => {
            NativeBackend::open_with_modes(path, VectorMode::Sidecar, StorageMode::Paged, 1024)
                .unwrap()
        }
        other => panic!("unknown mode {other}"),
    }
}

const PERSISTENT_MODES: [&str; 3] = ["resident-embedded", "resident-sidecar", "paged-sidecar"];

fn is_violation(result: &cognigraph_core::Result<impl std::fmt::Debug>) -> bool {
    matches!(result, Err(CogniGraphError::UniqueViolation { .. }))
}

#[tokio::test]
async fn unique_index_survives_reopen_in_every_persistent_mode() {
    for mode in PERSISTENT_MODES {
        let dir = TempDir::new();
        let path = dir.path().join("store.redb");
        {
            let backend = open(&path, mode);
            backend
                .ensure_collection("users", CollectionType::Document)
                .await
                .unwrap();
            backend.ensure_index("users", &email_index()).await.unwrap();
            backend
                .create_document(
                    "users",
                    json!({ "_key": "a", "email": "x@y", "embedding": [1.0, 0.0] }),
                )
                .await
                .unwrap();
        }
        let backend = open(&path, mode);
        let listed = backend.list_indexes("users").await.unwrap();
        assert_eq!(listed.len(), 1, "{mode}: {listed:?}");
        let dup = backend
            .create_document("users", json!({ "_key": "b", "email": "x@y" }))
            .await;
        assert!(is_violation(&dup), "{mode}: {dup:?}");
        backend
            .create_document("users", json!({ "_key": "b", "email": "b@y" }))
            .await
            .unwrap();
        // The entry written before reopen and the one written after both hold.
        let dup = backend
            .create_document("users", json!({ "_key": "c", "email": "b@y" }))
            .await;
        assert!(is_violation(&dup), "{mode}: {dup:?}");
        backend.delete_document("users", "a").await.unwrap();
        backend
            .create_document("users", json!({ "_key": "c", "email": "x@y" }))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn paged_mode_enforces_over_existing_rows_without_resident_bodies() {
    let dir = TempDir::new();
    let path = dir.path().join("store.redb");
    let backend = open(&path, "paged-sidecar");
    for (key, email) in [("a", "1"), ("b", "2"), ("c", "3")] {
        backend
            .create_document("users", json!({ "_key": key, "email": email }))
            .await
            .unwrap();
    }
    backend.ensure_index("users", &email_index()).await.unwrap();
    assert!(is_violation(
        &backend
            .create_document("users", json!({ "_key": "d", "email": "2" }))
            .await
    ));
    drop(backend);
    let backend = open(&path, "paged-sidecar");
    assert!(is_violation(
        &backend
            .create_document("users", json!({ "_key": "d", "email": "3" }))
            .await
    ));
    // Declaring over duplicates scans the stored rows and refuses.
    backend
        .create_document("users", json!({ "_key": "d", "email": "4", "tag": "t" }))
        .await
        .unwrap();
    backend
        .create_document("users", json!({ "_key": "e", "email": "5", "tag": "t" }))
        .await
        .unwrap();
    let tag = IndexDef {
        index_type: IndexType::Persistent,
        fields: vec!["tag".into()],
        unique: true,
        sparse: true,
        name: Some("tag".into()),
    };
    let err = backend.ensure_index("users", &tag).await.unwrap_err();
    assert!(
        matches!(err, CogniGraphError::UniqueViolation { ref existing, .. } if existing == "d"),
        "{err:?}"
    );
    assert_eq!(backend.list_indexes("users").await.unwrap().len(), 1);
}

#[tokio::test]
async fn snapshot_round_trip_keeps_constraints_and_rejects_duplicate_snapshots() {
    let source = NativeBackend::new();
    source
        .ensure_collection("users", CollectionType::Document)
        .await
        .unwrap();
    source.ensure_index("users", &email_index()).await.unwrap();
    source
        .create_document("users", json!({ "_key": "a", "email": "x@y" }))
        .await
        .unwrap();
    let snapshot = source.export_snapshot().await.unwrap();
    assert_eq!(
        snapshot["collections"]["users"]["indexes"][0]["fields"],
        json!(["email"]),
        "{snapshot}"
    );

    let target = NativeBackend::new();
    target.import_snapshot(&snapshot).await.unwrap();
    assert_eq!(target.list_indexes("users").await.unwrap().len(), 1);
    assert!(is_violation(
        &target
            .create_document("users", json!({ "_key": "b", "email": "x@y" }))
            .await
    ));

    // A hand-made snapshot that violates its own constraint is refused.
    let mut bad = snapshot.clone();
    bad["collections"]["users"]["documents"]["b"] = json!({ "_key": "b", "email": "x@y" });
    let fresh = NativeBackend::new();
    let err = fresh.import_snapshot(&bad).await.unwrap_err();
    assert!(
        matches!(err, CogniGraphError::UniqueViolation { .. }),
        "{err:?}"
    );

    // Re-importing the same snapshot over itself overwrites keys, no conflict.
    target.import_snapshot(&snapshot).await.unwrap();
}

#[tokio::test]
async fn concurrent_inserts_admit_exactly_one_holder() {
    let backend = Arc::new(NativeBackend::new());
    backend
        .ensure_collection("users", CollectionType::Document)
        .await
        .unwrap();
    backend.ensure_index("users", &email_index()).await.unwrap();
    let mut handles = Vec::new();
    for key in ["p", "q", "r", "s", "t", "u"] {
        let backend = Arc::clone(&backend);
        handles.push(tokio::spawn(async move {
            backend
                .create_document("users", json!({ "_key": key, "email": "same" }))
                .await
        }));
    }
    let mut ok = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => ok += 1,
            Err(CogniGraphError::UniqueViolation { .. }) => {}
            Err(other) => panic!("{other:?}"),
        }
    }
    assert_eq!(ok, 1);
    let rows = backend.list_documents("users", None, None).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn dropping_the_collection_drops_its_indexes_durably() {
    let dir = TempDir::new();
    let path = dir.path().join("store.redb");
    {
        let backend = open(&path, "resident-embedded");
        backend.ensure_index("users", &email_index()).await.unwrap();
        backend
            .create_document("users", json!({ "_key": "a", "email": "x@y" }))
            .await
            .unwrap();
        backend.drop_collection("users").await.unwrap();
        assert!(backend.list_indexes("users").await.unwrap().is_empty());
    }
    let backend = open(&path, "resident-embedded");
    assert!(backend.list_indexes("users").await.unwrap().is_empty());
    for key in ["a", "b"] {
        backend
            .create_document("users", json!({ "_key": key, "email": "x@y" }))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn batch_violation_rolls_back_the_whole_batch_in_persistent_modes() {
    for mode in PERSISTENT_MODES {
        let dir = TempDir::new();
        let path = dir.path().join("store.redb");
        let backend = open(&path, mode);
        backend.ensure_index("users", &email_index()).await.unwrap();
        backend
            .create_document("users", json!({ "_key": "a", "email": "x@y" }))
            .await
            .unwrap();
        let err = backend
            .execute_batch(vec![
                BatchOp::Insert {
                    collection: "users".into(),
                    doc: json!({ "_key": "b", "email": "fresh" }),
                },
                BatchOp::Update {
                    collection: "users".into(),
                    key: "a".into(),
                    merge: json!({ "email": "fresh" }),
                },
            ])
            .await
            .unwrap_err();
        assert!(
            matches!(err, CogniGraphError::UniqueViolation { .. }),
            "{mode}: {err:?}"
        );
        drop(backend);
        let backend = open(&path, mode);
        assert!(
            backend.get_document("users", "b").await.unwrap().is_none(),
            "{mode}"
        );
        assert_eq!(
            backend.get_document("users", "a").await.unwrap().unwrap()["email"],
            json!("x@y"),
            "{mode}"
        );
        // The value freed inside a batch is usable later in the same batch.
        backend
            .execute_batch(vec![
                BatchOp::Delete {
                    collection: "users".into(),
                    key: "a".into(),
                },
                BatchOp::Insert {
                    collection: "users".into(),
                    doc: json!({ "_key": "b", "email": "x@y" }),
                },
            ])
            .await
            .unwrap();
    }
}
