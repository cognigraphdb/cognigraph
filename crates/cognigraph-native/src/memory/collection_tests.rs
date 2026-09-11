use std::path::PathBuf;
use std::sync::{Arc, atomic::Ordering};

use cognigraph_core::{CollectionType, GraphBackend, VectorSearchOpts};
use serde_json::json;
use uuid::Uuid;

use super::{NativeBackend, StorageMode, VectorMode};

const MODES: [Option<(VectorMode, StorageMode)>; 4] = [
    None,
    Some((VectorMode::Embedded, StorageMode::Resident)),
    Some((VectorMode::Sidecar, StorageMode::Resident)),
    Some((VectorMode::Sidecar, StorageMode::Paged)),
];

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cg-ensure-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn open(&self, mode: Option<(VectorMode, StorageMode)>) -> NativeBackend {
        match mode {
            None => NativeBackend::new(),
            Some((vector, storage)) => {
                NativeBackend::open_with_modes(self.0.join("db.redb"), vector, storage, 1 << 20)
                    .unwrap()
            }
        }
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn versions(backend: &NativeBackend) -> (u64, Option<(u32, Uuid)>) {
    (
        backend.write_version.load(Ordering::Relaxed),
        backend.store.as_ref().map(|store| {
            (
                store.data_generation().unwrap(),
                store.data_revision().unwrap(),
            )
        }),
    )
}

fn assert_one_commit(before: (u64, Option<(u32, Uuid)>), backend: &NativeBackend) {
    let after = versions(backend);
    assert_eq!(after.0, before.0 + 1);
    if let Some((generation, revision)) = before.1 {
        let current = after.1.unwrap();
        assert_eq!(current.0, generation + 1);
        assert_ne!(current.1, revision);
    }
}

#[tokio::test]
async fn collection_ensures_preserve_versions_types_and_data() {
    for mode in MODES {
        let dir = TempDir::new();
        let backend = dir.open(mode);
        for kind in [CollectionType::Document, CollectionType::Edge] {
            let before = versions(&backend);
            backend.ensure_collection("items", kind).await.unwrap();
            assert_one_commit(before, &backend);
            match kind {
                CollectionType::Document => {
                    backend
                        .create_document("items", json!({"_key":"a", "v":1}))
                        .await
                        .unwrap();
                }
                CollectionType::Edge => {
                    backend
                        .create_edge(
                            "items",
                            json!({"_key":"a", "_from":"docs/a", "_to":"docs/b"}),
                        )
                        .await
                        .unwrap();
                }
            }
            let before = versions(&backend);
            let snapshot = backend.export_snapshot().await.unwrap();
            // Ensuring a different type has always preserved the first type.
            for requested in [CollectionType::Document, CollectionType::Edge] {
                backend.ensure_collection("items", requested).await.unwrap();
                assert_eq!(versions(&backend), before, "{mode:?}: {kind:?}");
                assert_eq!(backend.export_snapshot().await.unwrap(), snapshot);
                assert_eq!(
                    backend.list_collections().await.unwrap()[0].collection_type,
                    kind.as_str()
                );
            }
            backend.drop_collection("items").await.unwrap();
            assert_one_commit(before, &backend);
        }
        // Implicit creation through CRUD must satisfy later explicit ensures.
        backend
            .create_document("implicit", json!({"_key":"a"}))
            .await
            .unwrap();
        let before = versions(&backend);
        backend
            .ensure_collection("implicit", CollectionType::Document)
            .await
            .unwrap();
        assert_eq!(versions(&backend), before);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_collection_ensures_commit_only_one_creation() {
    for mode in MODES {
        let dir = TempDir::new();
        let backend = Arc::new(dir.open(mode));
        let before = versions(&backend);
        let barrier = Arc::new(tokio::sync::Barrier::new(16));
        let mut tasks = Vec::new();
        for n in 0..16 {
            let backend = backend.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                let kind = if n % 2 == 0 {
                    CollectionType::Document
                } else {
                    CollectionType::Edge
                };
                backend.ensure_collection("race", kind).await.unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_one_commit(before, &backend);
        let catalog = backend.list_collections().await.unwrap();
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].count, 0);
        assert!(
            backend
                .list_documents("race", None, None)
                .await
                .unwrap()
                .is_empty()
        );
        if mode.is_some() {
            let revision = versions(&backend).1;
            let snapshot = backend.export_snapshot().await.unwrap();
            drop(backend);
            let reopened = dir.open(mode);
            assert_eq!(versions(&reopened).1, revision);
            assert_eq!(reopened.export_snapshot().await.unwrap(), snapshot);
        }
    }
}

async fn search(backend: &NativeBackend) {
    let hits = backend
        .vector_search(
            "docs",
            &[1.0, 0.0],
            &VectorSearchOpts {
                threshold: Some(0.9),
                limit: 1,
                model_name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], "a");
    let hits = backend
        .text_search("docs", "needle", &["text".into()], 10)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], "a");
}

#[tokio::test]
async fn collection_ensures_preserve_live_and_reopened_derivatives() {
    for mode in MODES {
        let dir = TempDir::new();
        let backend = dir.open(mode);
        backend
            .create_document(
                "docs",
                json!({"_key":"a", "text":"needle", "embedding":[1,0]}),
            )
            .await
            .unwrap();
        search(&backend).await;
        let before = versions(&backend);
        let text_index = backend
            .text_indexes
            .read()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .clone();
        backend
            .ensure_collection("docs", CollectionType::Document)
            .await
            .unwrap();
        search(&backend).await;
        assert_eq!(versions(&backend), before);
        assert!(
            backend
                .text_indexes
                .read()
                .unwrap()
                .values()
                .any(|index| Arc::ptr_eq(index, &text_index))
        );
        drop(text_index);
        if let Some((vector, _)) = mode {
            drop(backend);
            let reopened = dir.open(mode);
            reopened
                .ensure_collection("docs", CollectionType::Document)
                .await
                .unwrap();
            assert_eq!(versions(&reopened).1, before.1);
            search(&reopened).await;
            if vector == VectorMode::Sidecar {
                assert_eq!(
                    reopened.sidecar_rebuild_count(),
                    0,
                    "unchanged file must reopen"
                );
                reopened
                    .ensure_collection("new_collection", CollectionType::Document)
                    .await
                    .unwrap();
                search(&reopened).await;
                assert_eq!(
                    reopened.sidecar_rebuild_count(),
                    1,
                    "real creation must invalidate"
                );
            }
        }
    }
}
