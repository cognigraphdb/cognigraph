//! CG-107: a document that could not be read back is refused on every write
//! path, in every storage mode, and the store keeps opening.

use std::collections::HashMap;
use std::path::PathBuf;

use cognigraph_core::{BatchOp, CogniGraphError, GraphBackend};
use cognigraph_native::{MAX_DOCUMENT_DEPTH, NativeBackend, StorageMode, VectorMode};
use cognigraph_query::{ExecutionError, QueryMode, parse_and_execute_backend_with_mode};
use serde_json::{Value, json};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cognigraph-depth-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    Memory,
    Resident,
    Paged,
}

fn open(mode: Mode, dir: &TempDir) -> NativeBackend {
    let path = dir.0.join("data.redb");
    match mode {
        Mode::Memory => NativeBackend::new(),
        Mode::Resident => NativeBackend::open(path).unwrap(),
        Mode::Paged => NativeBackend::open_with_modes(
            path,
            VectorMode::Sidecar,
            StorageMode::Paged,
            8 * 1024 * 1024,
        )
        .unwrap(),
    }
}

/// `levels` nested objects below the document itself.
fn nested(levels: usize) -> Value {
    let mut value = json!(1);
    for _ in 0..levels {
        value = json!({ "n": value });
    }
    value
}

/// A document whose total nesting is exactly `depth`.
fn document(key: &str, depth: usize) -> Value {
    json!({ "_key": key, "n": nested(depth - 1) })
}

fn refused(result: Result<impl std::fmt::Debug, CogniGraphError>, context: &str) {
    match result {
        Err(CogniGraphError::ValidationError(message)) => {
            assert!(
                message.contains("deeper than the limit"),
                "{context}: {message}"
            )
        }
        other => panic!("{context}: expected a validation error, got {other:?}"),
    }
}

#[tokio::test]
async fn every_write_path_accepts_the_limit_and_refuses_one_level_more() {
    let limit = MAX_DOCUMENT_DEPTH;
    for mode in [Mode::Memory, Mode::Resident, Mode::Paged] {
        let dir = TempDir::new();
        let backend = open(mode, &dir);
        let context = format!("{mode:?}");

        // create
        backend
            .create_document("docs", document("ok", limit))
            .await
            .unwrap();
        refused(
            backend
                .create_document("docs", document("deep", limit + 1))
                .await,
            &context,
        );
        assert!(
            backend
                .get_document("docs", "deep")
                .await
                .unwrap()
                .is_none()
        );

        // update: the merged result is what counts
        backend
            .create_document("docs", json!({"_key": "u", "v": 1}))
            .await
            .unwrap();
        refused(
            backend
                .update_document("docs", "u", json!({"n": nested(limit)}))
                .await,
            &context,
        );
        backend
            .update_document("docs", "u", json!({"n": nested(limit - 1)}))
            .await
            .unwrap();

        // replace
        refused(
            backend
                .replace_document("docs", "u", document("u", limit + 1))
                .await,
            &context,
        );
        let kept = backend.get_document("docs", "u").await.unwrap().unwrap();
        assert_eq!(
            kept["v"],
            json!(1),
            "{context}: a refused write changes nothing"
        );

        // atomic batch: one deep document refuses the whole batch
        refused(
            backend
                .execute_batch(vec![
                    BatchOp::Insert {
                        collection: "docs".into(),
                        doc: json!({"_key": "batch-ok"}),
                    },
                    BatchOp::Insert {
                        collection: "docs".into(),
                        doc: document("batch-deep", limit + 1),
                    },
                ])
                .await,
            &context,
        );
        assert!(
            backend
                .get_document("docs", "batch-ok")
                .await
                .unwrap()
                .is_none()
        );

        // bulk insert
        refused(
            backend.bulk_insert("docs", vec![document("bulk-deep", limit + 1)]),
            &context,
        );

        // edges
        backend
            .create_document("docs", json!({"_key": "a"}))
            .await
            .unwrap();
        backend
            .create_document("docs", json!({"_key": "b"}))
            .await
            .unwrap();
        refused(
            backend
                .create_edge(
                    "rels",
                    json!({"_from": "docs/a", "_to": "docs/b", "n": nested(limit)}),
                )
                .await,
            &context,
        );
        refused(
            backend
                .upsert_edge(
                    "rels",
                    "docs/a",
                    "docs/b",
                    "knows",
                    json!({"n": nested(limit)}),
                )
                .await,
            &context,
        );

        // snapshot import
        let snapshot = json!({"collections": {"imported": {
            "type": "document",
            "documents": {"import-deep": document("import-deep", limit + 1)}
        }}});
        refused(backend.import_snapshot(&snapshot).await, &context);

        // Arrays count as nesting too.
        let mut arrays = json!([]);
        for _ in 0..limit - 1 {
            arrays = json!([arrays]);
        }
        refused(
            backend
                .create_document("docs", json!({"_key": "arrays", "a": arrays}))
                .await,
            &context,
        );

        // A persistent store reopens and still serves the documents at the limit.
        if !matches!(mode, Mode::Memory) {
            drop(backend);
            let reopened = open(mode, &dir);
            let ok = reopened.get_document("docs", "ok").await.unwrap().unwrap();
            assert_eq!(ok, {
                let mut expected = document("ok", limit);
                for field in ["_id", "created_at", "updated_at"] {
                    expected[field] = ok[field].clone();
                }
                expected
            });
            assert!(
                reopened
                    .list_documents("docs", None, None)
                    .await
                    .unwrap()
                    .len()
                    >= 4
            );
        }
    }
}

/// The reproduction from the ticket: each run wraps the stored value once more.
#[tokio::test]
async fn repeated_cgql_updates_stop_at_the_limit_and_the_store_reopens() {
    for mode in [Mode::Resident, Mode::Paged] {
        let dir = TempDir::new();
        let backend = open(mode, &dir);
        backend
            .create_document("deep", json!({"_key": "d", "n": 1}))
            .await
            .unwrap();
        let mut accepted = 0;
        let mut refusal = None;
        for _ in 0..135 {
            match parse_and_execute_backend_with_mode(
                "FOR d IN deep UPDATE d._key WITH { n: { n: d.n } } IN deep",
                &backend,
                &HashMap::new(),
                QueryMode::ReadWrite,
            )
            .await
            {
                Ok(_) => accepted += 1,
                Err(error) => {
                    refusal = Some(error);
                    break;
                }
            }
        }
        // The document starts at depth 1 and every run adds one level.
        assert_eq!(accepted, MAX_DOCUMENT_DEPTH - 1, "{mode:?}");
        match refusal {
            Some(ExecutionError::InvalidInput(message)) => {
                assert!(message.contains("`deep/d`"), "{message}")
            }
            other => panic!("{mode:?}: expected an input error, got {other:?}"),
        }

        drop(backend);
        let reopened = open(mode, &dir);
        let stored = reopened.get_document("deep", "d").await.unwrap().unwrap();
        let mut value = &stored["n"];
        let mut levels = 1;
        while value.is_object() {
            value = &value["n"];
            levels += 1;
        }
        assert_eq!(levels, MAX_DOCUMENT_DEPTH, "{mode:?}");
        // The exported snapshot wraps documents in envelopes and must itself
        // stay parseable.
        let snapshot = reopened.export_snapshot().await.unwrap();
        let text = serde_json::to_string(&snapshot).unwrap();
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, snapshot);
    }
}
