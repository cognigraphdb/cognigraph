//! Caller-owned strings are exact across every Native write and read path.
use cognigraph_core::{
    BatchOp, CollectionType, Direction, GraphBackend, TraversalOpts, VectorSearchOpts,
};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::json;
use std::collections::HashMap;

const COMPOSED: &str = "café";
const DECOMPOSED: &str = "cafe\u{301}";

async fn check(backend: &NativeBackend, collection: &str) {
    for (key, marker) in [(COMPOSED, "composed"), (DECOMPOSED, "decomposed")] {
        let handle = format!("{collection}/{key}");
        let document = backend
            .get_document(collection, key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(document["marker"], marker);
        assert_eq!(document["reference"], handle);
        assert_eq!(document["nested"]["opaque"][0], DECOMPOSED);
        for expression in [serde_json::to_string(&handle).unwrap(), "@id".into()] {
            let rows = backend
                .query(
                    &format!("RETURN DOCUMENT({expression}).marker"),
                    if expression == "@id" {
                        HashMap::from([("id".into(), json!(handle))])
                    } else {
                        HashMap::new()
                    },
                )
                .await
                .unwrap();
            assert_eq!(rows, vec![json!(marker)]);
        }
        // Dynamic resolution and exact model filters cannot conflate the two forms.
        let rows = backend
            .query(
                "FOR d IN DOCUMENT(@ids) FILTER d.reference == @id RETURN d.marker",
                HashMap::from([
                    (
                        "ids".into(),
                        json!([
                            format!("{collection}/{COMPOSED}"),
                            format!("{collection}/{DECOMPOSED}")
                        ]),
                    ),
                    ("id".into(), json!(handle)),
                ]),
            )
            .await
            .unwrap();
        assert_eq!(rows, vec![json!(marker)]);
        // A collection scan can push this equality into Native projection reads.
        for expression in [serde_json::to_string(&handle).unwrap(), "@id".into()] {
            let rows = backend
                .query(
                    &format!(
                        "FOR d IN filter_docs FILTER d.reference == {expression} RETURN d.marker"
                    ),
                    if expression == "@id" {
                        HashMap::from([("id".into(), json!(handle))])
                    } else {
                        HashMap::new()
                    },
                )
                .await
                .unwrap();
            assert_eq!(rows, vec![json!(marker)]);
        }
        let hits = backend
            .vector_search(
                collection,
                &[1.0, 0.0],
                &VectorSearchOpts {
                    model_name: Some(key.into()),
                    limit: 1,
                    threshold: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document["_key"], key);
        let edges = backend
            .get_edges("rels", &handle, Direction::Outbound)
            .await
            .unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["_from"], handle);
        assert_eq!(edges[0]["relation_type"], key);
        let paths = backend
            .traverse(
                &handle,
                &TraversalOpts {
                    direction: Direction::Outbound,
                    min_depth: 1,
                    max_depth: 1,
                    edge_collection: "rels".into(),
                    min_confidence: None,
                    path_decay: 0.8,
                },
            )
            .await
            .unwrap();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].vertices.last().unwrap()["_id"], "targets/end");
    }
    let job = backend
        .get_document("_cognigraph_jobs", "synthetic")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job["_execution"]["collection"], collection);
    assert_eq!(job["_execution"]["keys"], json!([DECOMPOSED, COMPOSED]));
    assert_eq!(
        job["signed_bytes"], DECOMPOSED,
        "storage must not canonicalize opaque payloads"
    );
}

async fn writes(backend: &NativeBackend, collection: &str) {
    backend
        .create_document("targets", json!({"_key":"end"}))
        .await
        .unwrap();
    backend
        .ensure_collection("rels", CollectionType::Edge)
        .await
        .unwrap();
    for (key, marker) in [(COMPOSED, "composed"), (DECOMPOSED, "decomposed")] {
        let handle = format!("{collection}/{key}");
        let doc = json!({"_key":key,"marker":marker,"reference":handle,
            "nested":{"opaque":[DECOMPOSED]},"embedding":[1.0,0.0],"model_name":key});
        backend
            .create_document("filter_docs", doc.clone())
            .await
            .unwrap();
        backend
            .create_document(collection, doc.clone())
            .await
            .unwrap();
        backend
            .update_document(collection, key, json!({"other":DECOMPOSED}))
            .await
            .unwrap();
        backend
            .replace_document(collection, key, doc.clone())
            .await
            .unwrap();
        backend
            .execute_batch(vec![BatchOp::Update {
                collection: collection.into(),
                key: key.into(),
                merge: doc,
            }])
            .await
            .unwrap();
        let edge = json!({"_key":key,"_from":handle,"_to":"targets/end","relation_type":key,"opaque":DECOMPOSED});
        backend.create_edge("rels", edge.clone()).await.unwrap();
        let upsert = backend
            .upsert_edge(
                "rels",
                &handle,
                "targets/end",
                key,
                json!({"opaque":DECOMPOSED}),
            )
            .await
            .unwrap();
        assert_eq!(upsert["_key"], key, "upsert retains exact triple identity");
        backend
            .execute_batch(vec![BatchOp::Replace {
                collection: "rels".into(),
                key: key.into(),
                doc: edge,
            }])
            .await
            .unwrap();
    }
    backend
        .execute_batch(vec![BatchOp::Insert {
            collection: "_cognigraph_jobs".into(),
            doc: json!({
        "_key":"synthetic","_execution":{"collection":collection,"keys":[DECOMPOSED,COMPOSED]},
        "signed_bytes":DECOMPOSED}),
        }])
        .await
        .unwrap();
    // Literal values must also survive the mutation executor and persistence.
    let query = format!(
        "INSERT {{_key: \"query-write\", opaque: {}}} INTO payloads RETURN NEW.opaque",
        serde_json::to_string(DECOMPOSED).unwrap()
    );
    assert_eq!(
        cognigraph_query::parse_and_execute_backend_with_mode(
            &query,
            backend,
            &HashMap::new(),
            cognigraph_query::QueryMode::ReadWrite
        )
        .await
        .unwrap(),
        vec![json!(DECOMPOSED)]
    );
}

async fn lifecycle(vector: VectorMode, storage: StorageMode) {
    let path = std::env::temp_dir().join(format!("cg33-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    let open =
        || NativeBackend::open_with_modes(path.join("db.redb"), vector, storage, 1 << 20).unwrap();
    let backend = open();
    writes(&backend, DECOMPOSED).await;
    check(&backend, DECOMPOSED).await;
    let snapshot = backend.export_snapshot().await.unwrap();
    let restored = NativeBackend::new();
    restored.import_snapshot(&snapshot).await.unwrap();
    check(&restored, DECOMPOSED).await;
    assert_eq!(restored.export_snapshot().await.unwrap(), snapshot);
    drop(backend);
    let reopened = open();
    check(&reopened, DECOMPOSED).await;
    assert_eq!(reopened.export_snapshot().await.unwrap(), snapshot);
    drop(reopened);
    std::fs::remove_dir_all(path).unwrap();
}

#[tokio::test]
async fn resident_embedded_exact_identity() {
    lifecycle(VectorMode::Embedded, StorageMode::Resident).await;
}
#[tokio::test]
async fn resident_sidecar_exact_identity() {
    lifecycle(VectorMode::Sidecar, StorageMode::Resident).await;
}
#[tokio::test]
async fn paged_sidecar_exact_identity() {
    lifecycle(VectorMode::Sidecar, StorageMode::Paged).await;
}
