//! Shared model-selection lifecycle in every supported persistent Native mode.
use cognigraph_core::{BatchOp, GraphBackend, VectorSearchOpts};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::Value;
use std::path::PathBuf;

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn batch_op(action: &Value) -> BatchOp {
    let collection = "vectors".into();
    let key = action["key"].as_str().unwrap().into();
    match action["op"].as_str().unwrap() {
        "create" => BatchOp::Insert {
            collection,
            doc: action["doc"].clone(),
        },
        "update" => BatchOp::Update {
            collection,
            key,
            merge: action["doc"].clone(),
        },
        other => panic!("unsupported batch fixture operation: {other}"),
    }
}

async fn check(backend: &NativeBackend, phase: &Value) {
    for case in phase["cases"].as_array().unwrap() {
        let opts: VectorSearchOpts = serde_json::from_value(case.clone()).unwrap();
        let hits = backend
            .vector_search("vectors", &[1.0, 0.0], &opts)
            .await
            .unwrap();
        let keys: Vec<_> = hits
            .iter()
            .map(|hit| hit.document["_key"].clone())
            .collect();
        assert_eq!(keys, *case["keys"].as_array().unwrap(), "{}", case["name"]);
        for (hit, expected) in hits.iter().zip(case["scores"].as_array().unwrap()) {
            assert!((hit.score - expected.as_f64().unwrap()).abs() < 1e-12);
        }
    }
}

async fn lifecycle(vector: VectorMode, storage: StorageMode) {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/sidecar-model-filter.json")).unwrap();
    let dir = TempDir(std::env::temp_dir().join(format!("cg34-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let open =
        || NativeBackend::open_with_modes(dir.0.join("db.redb"), vector, storage, 1 << 20).unwrap();
    let mut backend = open();
    for row in fixture["rows"].as_array().unwrap() {
        backend
            .create_document("vectors", row.clone())
            .await
            .unwrap();
    }
    for phase in fixture["phases"].as_array().unwrap() {
        for action in phase["actions"].as_array().unwrap() {
            let key = action["key"].as_str().unwrap_or_default();
            let doc = action["doc"].clone();
            match action["op"].as_str().unwrap() {
                "create" => {
                    backend.create_document("vectors", doc).await.unwrap();
                }
                "update" => {
                    backend.update_document("vectors", key, doc).await.unwrap();
                }
                "replace" => {
                    backend.replace_document("vectors", key, doc).await.unwrap();
                }
                "delete" => {
                    assert!(backend.delete_document("vectors", key).await.unwrap());
                }
                "batch" => {
                    let ops = action["operations"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(batch_op)
                        .collect();
                    backend.execute_batch(ops).await.unwrap();
                }
                other => panic!("unsupported fixture operation: {other}"),
            }
        }
        check(&backend, phase).await;
        if vector == VectorMode::Sidecar {
            let expected = if phase["name"] == "base" || phase["name"] == "rebuild" {
                1
            } else {
                0
            };
            assert_eq!(
                backend.sidecar_rebuild_count(),
                expected,
                "{}",
                phase["name"]
            );
        }
        if phase["name"] == "base" || phase["name"] == "rebuild" {
            drop(backend);
            backend = open();
            check(&backend, phase).await;
            assert_eq!(backend.sidecar_rebuild_count(), 0, "warm file reuse");
        }
    }
    // A model-only update must preserve the original full-precision vector.
    let snapshot = backend.export_snapshot().await.unwrap();
    let b0 = &snapshot["collections"]["vectors"]["documents"]["b0"];
    assert_eq!(b0["embedding"], fixture["rows"][80]["embedding"]);
}

#[tokio::test]
async fn resident_embedded_model_lifecycle() {
    lifecycle(VectorMode::Embedded, StorageMode::Resident).await;
}

#[tokio::test]
async fn resident_sidecar_model_lifecycle() {
    lifecycle(VectorMode::Sidecar, StorageMode::Resident).await;
}

#[tokio::test]
async fn paged_sidecar_model_lifecycle() {
    lifecycle(VectorMode::Sidecar, StorageMode::Paged).await;
}
