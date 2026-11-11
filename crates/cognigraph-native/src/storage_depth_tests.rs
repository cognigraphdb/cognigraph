//! CG-107: depth accounting and how an already unreadable row is reported.

use super::*;

/// Built by moving, not with `json!`: the macro serializes an interpolated
/// value, which recurses and would overflow at the depths used here.
fn nested(levels: usize) -> Value {
    let mut value = serde_json::json!(1);
    for _ in 0..levels {
        let mut object = serde_json::Map::new();
        object.insert("n".to_string(), value);
        value = Value::Object(object);
    }
    value
}

#[test]
fn depth_counts_arrays_and_objects_and_is_exact_at_the_limit() {
    assert!(check_document_depth("c", "k", &serde_json::json!(1)).is_ok());
    assert!(check_document_depth("c", "k", &serde_json::json!({})).is_ok());
    assert!(check_document_depth("c", "k", &nested(MAX_DOCUMENT_DEPTH)).is_ok());
    let error = check_document_depth("c", "k", &nested(MAX_DOCUMENT_DEPTH + 1)).unwrap_err();
    assert!(
        matches!(error, CogniGraphError::ValidationError(_)),
        "{error}"
    );
    assert!(error.to_string().contains("`c/k`"), "{error}");

    // Arrays nest like objects, and an empty container still counts.
    let mut arrays = serde_json::json!([]);
    for _ in 0..MAX_DOCUMENT_DEPTH - 1 {
        arrays = serde_json::json!([arrays]);
    }
    assert!(check_document_depth("c", "k", &arrays).is_ok());
    assert!(check_document_depth("c", "k", &serde_json::json!([arrays])).is_err());

    // Width is not depth, and the deep branch need not be the first one.
    let wide: Vec<Value> = (0..10_000).map(|n| serde_json::json!({ "n": n })).collect();
    assert!(check_document_depth("c", "k", &serde_json::json!({ "items": wide })).is_ok());
    let hidden = serde_json::json!({ "a": 1, "b": [1, 2, nested(MAX_DOCUMENT_DEPTH - 1)] });
    assert!(check_document_depth("c", "k", &hidden).is_err());
}

#[test]
fn the_walk_does_not_recurse_on_hostile_depth() {
    let deep = nested(200_000);
    let result = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(move || {
            let refused = check_document_depth("c", "k", &deep).is_err();
            // Dropping a value this deep recurses; leak it instead.
            std::mem::forget(deep);
            refused
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(result);
}

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cg-storage-depth-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A row written before the limit existed cannot be parsed. The store fails
/// closed, and the error names the row so an operator can find it.
#[test]
fn an_unreadable_stored_row_is_named_in_the_error() {
    let dir = TempDir::new();
    let store = RedbStore::open(&dir.0.join("data.redb")).unwrap();
    let readable = serde_json::json!({ "_key": "ok" });
    let unreadable = nested(140);
    store
        .apply(&[
            StoreOp::PutCollection {
                name: "deep",
                collection_type: CollectionType::Document,
            },
            StoreOp::PutDocument {
                collection: "deep",
                key: "ok",
                doc: &readable,
            },
            StoreOp::PutDocument {
                collection: "deep",
                key: "too-deep",
                doc: &unreadable,
            },
        ])
        .unwrap();

    let error = store.load().unwrap_err().to_string();
    assert!(
        error.contains("stored document `deep/too-deep` cannot be read"),
        "{error}"
    );
    assert!(error.contains("recursion limit"), "{error}");
    let error = store.scan_collection("deep").unwrap_err().to_string();
    assert!(error.contains("`deep/too-deep`"), "{error}");
}
