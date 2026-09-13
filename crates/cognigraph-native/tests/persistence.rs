//! Durability and import/export tests for the persistent native backend.

use cognigraph_core::{CogniGraphError, CollectionType, Direction, GraphBackend, TraversalOpts};
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

async fn seed(backend: &NativeBackend) {
    backend
        .ensure_collection("documents", CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_collection("relationships", CollectionType::Edge)
        .await
        .unwrap();
    backend
        .create_document(
            "documents",
            json!({ "_key": "a", "title": "Alpha", "category": "research" }),
        )
        .await
        .unwrap();
    backend
        .create_document(
            "documents",
            json!({ "_key": "b", "title": "Beta", "category": "research" }),
        )
        .await
        .unwrap();
    backend
        .create_edge(
            "relationships",
            json!({
                "_from": "documents/a",
                "_to": "documents/b",
                "relation_type": "links",
                "confidence": 0.9
            }),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn persistent_backend_ping_reads_the_live_store() {
    let dir = TempDir::new();
    let backend = NativeBackend::open(dir.db_path()).unwrap();

    backend.ping().await.unwrap();
}

#[tokio::test]
async fn data_survives_reopen() {
    let dir = TempDir::new();

    {
        let backend = NativeBackend::open(dir.db_path()).unwrap();
        seed(&backend).await;
        backend
            .update_document("documents", "a", json!({ "reviewed": true }))
            .await
            .unwrap();
    }

    let backend = NativeBackend::open(dir.db_path()).unwrap();

    let doc = backend
        .get_document("documents", "a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(doc["title"], json!("Alpha"));
    assert_eq!(doc["reviewed"], json!(true));

    // Collection types survive: edge writes into the document collection
    // must still be rejected.
    let err = backend
        .create_edge(
            "documents",
            json!({ "_from": "documents/a", "_to": "documents/b" }),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::ValidationError(_)));

    // Edges and traversal survive.
    let paths = backend
        .traverse(
            "documents/a",
            &TraversalOpts {
                max_depth: 1,
                min_depth: 1,
                direction: Direction::Outbound,
                edge_collection: "relationships".into(),
                min_confidence: None,
                path_decay: 0.8,
            },
        )
        .await
        .unwrap();
    assert_eq!(paths.len(), 1);

    // CGQL runs against the reloaded state.
    let rows = backend
        .query(
            "FOR d IN documents SORT d.title ASC RETURN d.title",
            HashMap::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows, vec![json!("Alpha"), json!("Beta")]);
}

#[tokio::test]
async fn deletions_survive_reopen() {
    let dir = TempDir::new();

    {
        let backend = NativeBackend::open(dir.db_path()).unwrap();
        seed(&backend).await;
        assert!(backend.delete_document("documents", "b").await.unwrap());
        backend.drop_collection("relationships").await.unwrap();
    }

    let backend = NativeBackend::open(dir.db_path()).unwrap();
    assert!(
        backend
            .get_document("documents", "b")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        backend
            .list_documents("relationships", None, None)
            .await
            .is_err(),
        "dropped collection must stay dropped after reopen"
    );
    // The surviving document is still there.
    assert!(
        backend
            .get_document("documents", "a")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn export_import_roundtrip() {
    let source = NativeBackend::new();
    seed(&source).await;
    let snapshot = source.export_json().await.unwrap();

    let target = NativeBackend::new();
    target.import_json(&snapshot).await.unwrap();

    assert_eq!(target.export_json().await.unwrap(), snapshot);

    // Imported types are live: edge writes into the document collection fail.
    let err = target
        .create_document("relationships", json!({ "title": "not an edge" }))
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::ValidationError(_)));
}

#[tokio::test]
async fn import_into_persistent_backend_survives_reopen() {
    let source = NativeBackend::new();
    seed(&source).await;
    let snapshot = source.export_json().await.unwrap();

    let dir = TempDir::new();
    {
        let backend = NativeBackend::open(dir.db_path()).unwrap();
        backend.import_json(&snapshot).await.unwrap();
    }

    let backend = NativeBackend::open(dir.db_path()).unwrap();
    assert_eq!(backend.export_json().await.unwrap(), snapshot);
}

#[tokio::test]
async fn shared_backend_contract_on_persistent_backend() {
    let dir = TempDir::new();
    let backend = NativeBackend::open(dir.db_path()).unwrap();
    cognigraph_core::contract::run_all(&backend, "contract").await;
}

#[tokio::test]
async fn multilingual_content_survives_reopen() {
    let dir = TempDir::new();

    {
        let backend = NativeBackend::open(dir.db_path()).unwrap();
        backend
            .ensure_collection("i18n", CollectionType::Document)
            .await
            .unwrap();
        backend
            .create_document(
                "i18n",
                json!({
                    "_key": "multi",
                    "de": "Straße über München",
                    "ru": "Москва",
                    "he": "ירושלים של זהב",
                    "es": "jalapeños mañana",
                    "mixed": "Rust und Русский junto עם עברית"
                }),
            )
            .await
            .unwrap();
    }

    let backend = NativeBackend::open(dir.db_path()).unwrap();
    let doc = backend
        .get_document("i18n", "multi")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(doc["de"], json!("Straße über München"));
    assert_eq!(doc["ru"], json!("Москва"));
    assert_eq!(doc["he"], json!("ירושלים של זהב"));
    assert_eq!(doc["mixed"], json!("Rust und Русский junto עם עברית"));

    // CGQL over reloaded multilingual state, filtering in Cyrillic.
    let rows = backend
        .query(
            r#"FOR d IN i18n FILTER d.ru == "Москва" RETURN LENGTH(d.he)"#,
            HashMap::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows, vec![json!(14)]);
}

#[tokio::test]
async fn tantivy_index_persists_across_reopen() {
    let dir = TempDir::new();
    let fields = vec!["title".to_string()];
    {
        let backend = NativeBackend::open(dir.db_path()).unwrap();
        backend
            .ensure_collection("articles", CollectionType::Document)
            .await
            .unwrap();
        for (key, title) in [("a", "rust engine"), ("b", "cooking pasta")] {
            backend
                .create_document("articles", json!({ "_key": key, "title": title }))
                .await
                .unwrap();
        }
        let hits = backend
            .text_search("articles", "rust", &fields, 5)
            .await
            .unwrap();
        assert_eq!(hits[0].document["_key"], json!("a"));
    }
    // A commit-revision-stamped tantivy directory exists on disk.
    let tantivy_dir = std::fs::read_dir(&dir.0)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().contains(".tantivy"))
        .expect("persistent tantivy directory");
    let revision_path = tantivy_dir.path().join("REVISION");
    let revision = std::fs::read_to_string(&revision_path).unwrap();
    assert!(uuid::Uuid::parse_str(&revision).is_ok());
    let modified = std::fs::metadata(&revision_path)
        .unwrap()
        .modified()
        .unwrap();

    // Reopen: warm start (stamp matches), correct results; a write after
    // reopen invalidates and the new doc is findable.
    let backend = NativeBackend::open(dir.db_path()).unwrap();
    let hits = backend
        .text_search("articles", "rust", &fields, 5)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("a"));
    assert_eq!(
        std::fs::metadata(&revision_path)
            .unwrap()
            .modified()
            .unwrap(),
        modified,
        "unchanged restart rebuilt the index"
    );
    backend
        .create_document(
            "articles",
            json!({ "_key": "c", "title": "rust rust rust" }),
        )
        .await
        .unwrap();
    let hits = backend
        .text_search("articles", "rust", &fields, 5)
        .await
        .unwrap();
    assert_eq!(hits[0].document["_key"], json!("c"));
    assert_ne!(std::fs::read_to_string(revision_path).unwrap(), revision);
}
