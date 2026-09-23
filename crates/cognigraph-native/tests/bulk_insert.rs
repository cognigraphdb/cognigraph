//! CG-66: verbatim bulk insert for offline importers. Documents are stored
//! exactly as given (no timestamps, no edge defaults), one store transaction
//! per call, all-or-nothing, with duplicate keys and unique constraints
//! refused before any change. Holds in every storage mode and across reopen.

use cognigraph_core::{CogniGraphError, CollectionType, GraphBackend, IndexDef, IndexType};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::{Value, json};

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cognigraph-native-bulk-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Documents for one refused call and the error it must produce.
type Case = (Vec<Value>, fn(&CogniGraphError) -> bool);

const MODES: [&str; 4] = [
    "memory",
    "resident-embedded",
    "resident-sidecar",
    "paged-sidecar",
];

fn open(dir: &TempDir, mode: &str) -> NativeBackend {
    let path = dir.0.join("store.redb");
    match mode {
        "memory" => NativeBackend::new(),
        "resident-embedded" => NativeBackend::open(&path).unwrap(),
        "resident-sidecar" => {
            NativeBackend::open_with_modes(&path, VectorMode::Sidecar, StorageMode::Resident, 0)
                .unwrap()
        }
        "paged-sidecar" => {
            NativeBackend::open_with_modes(&path, VectorMode::Sidecar, StorageMode::Paged, 4096)
                .unwrap()
        }
        other => panic!("{other}"),
    }
}

fn email_unique() -> IndexDef {
    IndexDef {
        index_type: IndexType::Persistent,
        fields: vec!["email".into()],
        unique: true,
        sparse: false,
        name: None,
    }
}

fn user_fields(mut doc: Value) -> Value {
    doc.as_object_mut().unwrap().remove("_id");
    doc
}

async fn setup(backend: &NativeBackend) {
    backend
        .ensure_collection("people", CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_collection("knows", CollectionType::Edge)
        .await
        .unwrap();
    backend
        .ensure_index("people", &email_unique())
        .await
        .unwrap();
}

#[tokio::test]
async fn documents_and_edges_are_stored_verbatim_in_every_mode() {
    for mode in MODES {
        let dir = TempDir::new();
        let backend = open(&dir, mode);
        setup(&backend).await;
        let person = json!({"_key": "a:1", "email": "a@x", "updated_at": "1999-01-01",
                            "n": 9007199254740993u64, "huge": 1e300, "nested": {"e": [], "o": {}}});
        let other = json!({"_key": "b", "email": "b@x"});
        backend
            .bulk_insert("people", vec![person.clone(), other.clone()])
            .unwrap();
        let edge = json!({"_key": "k1", "_from": "people/a:1", "_to": "people/b"});
        backend.bulk_insert("knows", vec![edge.clone()]).unwrap();

        let stored = backend
            .get_document("people", "a:1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored["_id"], json!("people/a:1"), "{mode}");
        assert_eq!(
            user_fields(stored),
            person,
            "{mode}: no timestamps added or changed"
        );
        let stored = backend.get_document("knows", "k1").await.unwrap().unwrap();
        assert_eq!(
            user_fields(stored),
            edge,
            "{mode}: no relation_type/confidence defaults"
        );
        let listed = backend.list_documents("people", None, None).await.unwrap();
        assert_eq!(listed.len(), 2, "{mode}");
    }
}

#[tokio::test]
async fn persistent_modes_keep_verbatim_documents_and_constraints_after_reopen() {
    for mode in &MODES[1..] {
        let dir = TempDir::new();
        {
            let backend = open(&dir, mode);
            setup(&backend).await;
            backend
                .bulk_insert(
                    "people",
                    vec![json!({"_key": "a", "email": "a@x", "embedding": [0.5, 1.0]})],
                )
                .unwrap();
        }
        // Reopen in embedded mode: the raw stored bytes keep every field.
        let backend = NativeBackend::open(dir.0.join("store.redb")).unwrap();
        let stored = backend.get_document("people", "a").await.unwrap().unwrap();
        assert_eq!(
            user_fields(stored),
            json!({"_key": "a", "email": "a@x", "embedding": [0.5, 1.0]}),
            "{mode}"
        );
        let err = backend
            .create_document("people", json!({"_key": "z", "email": "a@x"}))
            .await
            .unwrap_err();
        assert!(
            matches!(err, CogniGraphError::UniqueViolation { .. }),
            "{mode}: {err:?}"
        );
    }
}

#[tokio::test]
async fn a_rejected_call_changes_nothing() {
    for mode in MODES {
        let dir = TempDir::new();
        let backend = open(&dir, mode);
        setup(&backend).await;
        backend
            .bulk_insert("people", vec![json!({"_key": "a", "email": "a@x"})])
            .unwrap();
        let cases: Vec<Case> = vec![
            // Existing key.
            (
                vec![
                    json!({"_key": "n1", "email": "n1"}),
                    json!({"_key": "a", "email": "fresh"}),
                ],
                |e| matches!(e, CogniGraphError::DocumentConflict(k) if k == "people/a"),
            ),
            // Key repeated inside the call.
            (
                vec![
                    json!({"_key": "n1", "email": "n1"}),
                    json!({"_key": "n1", "email": "n2"}),
                ],
                |e| matches!(e, CogniGraphError::DocumentConflict(k) if k == "people/n1"),
            ),
            // Unique value held by a stored document, then by an earlier doc in the call.
            (
                vec![json!({"_key": "n1", "email": "a@x"})],
                |e| matches!(e, CogniGraphError::UniqueViolation { existing, .. } if existing == "a"),
            ),
            (
                vec![
                    json!({"_key": "n1", "email": "s"}),
                    json!({"_key": "n2", "email": "s"}),
                ],
                |e| matches!(e, CogniGraphError::UniqueViolation { existing, .. } if existing == "n1"),
            ),
            // Shape errors.
            (vec![json!({"email": "no key"})], |e| {
                matches!(e, CogniGraphError::ValidationError(_))
            }),
            (vec![json!({"_key": ""})], |e| {
                matches!(e, CogniGraphError::ValidationError(_))
            }),
            (vec![json!(["not an object"])], |e| {
                matches!(e, CogniGraphError::ValidationError(_))
            }),
            (
                vec![json!({"_key": "n1", "_id": "other/n1", "email": "q"})],
                |e| matches!(e, CogniGraphError::ValidationError(_)),
            ),
        ];
        for (docs, expected) in cases {
            let err = backend.bulk_insert("people", docs.clone()).unwrap_err();
            assert!(expected(&err), "{mode}: {docs:?} -> {err:?}");
            assert_eq!(
                backend
                    .list_documents("people", None, None)
                    .await
                    .unwrap()
                    .len(),
                1,
                "{mode}: {docs:?} left a partial write"
            );
            assert!(
                backend
                    .get_document("people", "n1")
                    .await
                    .unwrap()
                    .is_none(),
                "{mode}"
            );
        }
        // The value refused above is still free for a valid call.
        backend
            .bulk_insert("people", vec![json!({"_key": "n1", "email": "s"})])
            .unwrap();
    }
}

#[tokio::test]
async fn edges_need_both_ends_and_unknown_collections_are_refused() {
    let backend = NativeBackend::new();
    setup(&backend).await;
    for edge in [
        json!({"_key": "e", "_from": "people/a"}),
        json!({"_key": "e", "_from": "people/a", "_to": 7}),
    ] {
        let err = backend.bulk_insert("knows", vec![edge]).unwrap_err();
        assert!(
            matches!(err, CogniGraphError::ValidationError(_)),
            "{err:?}"
        );
    }
    let err = backend
        .bulk_insert("absent", vec![json!({"_key": "a"})])
        .unwrap_err();
    assert!(
        matches!(err, CogniGraphError::CollectionNotFound(_)),
        "{err:?}"
    );
    // An empty call is a no-op even for a known collection.
    backend.bulk_insert("knows", Vec::new()).unwrap();
}

#[tokio::test]
async fn bulk_inserted_edges_are_traversable() {
    let backend = NativeBackend::new();
    setup(&backend).await;
    backend
        .bulk_insert(
            "people",
            vec![
                json!({"_key": "a", "email": "1"}),
                json!({"_key": "b", "email": "2"}),
            ],
        )
        .unwrap();
    backend
        .bulk_insert(
            "knows",
            vec![json!({"_key": "k", "_from": "people/a", "_to": "people/b"})],
        )
        .unwrap();
    let rows = backend
        .query(
            r#"FOR v IN 1..1 OUTBOUND "people/a" knows RETURN v._key"#,
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(rows, vec![json!("b")]);
}
