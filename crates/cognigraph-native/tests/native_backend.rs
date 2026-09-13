use cognigraph_core::{
    CogniGraphError, CollectionType, Direction, GraphBackend, QueryLanguage, TraversalOpts,
};
use cognigraph_native::NativeBackend;
use serde_json::json;

#[tokio::test]
async fn document_crud_works() {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("documents", CollectionType::Document)
        .await
        .unwrap();

    let id = backend
        .create_document(
            "documents",
            json!({
                "_key": "a",
                "title": "Alpha",
                "category": "research"
            }),
        )
        .await
        .unwrap();

    assert_eq!(id.full_id(), "documents/a");
    assert_eq!(
        backend
            .get_document("documents", "a")
            .await
            .unwrap()
            .unwrap()["title"],
        json!("Alpha")
    );

    let updated = backend
        .update_document("documents", "a", json!({ "title": "Updated" }))
        .await
        .unwrap();
    assert_eq!(updated["title"], json!("Updated"));

    let listed = backend
        .list_documents("documents", Some(10), Some(0))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);

    assert!(backend.delete_document("documents", "a").await.unwrap());
    assert!(!backend.delete_document("documents", "a").await.unwrap());
}

#[tokio::test]
async fn edges_and_traversal_work() {
    let backend = seeded_backend().await;
    let edges = backend
        .get_edges("relationships", "documents/a", Direction::Outbound)
        .await
        .unwrap();
    assert_eq!(edges.len(), 2);

    let paths = backend
        .traverse(
            "documents/a",
            &TraversalOpts {
                max_depth: 2,
                min_depth: 1,
                direction: Direction::Outbound,
                edge_collection: "relationships".into(),
                min_confidence: Some(0.7),
                path_decay: 0.8,
            },
        )
        .await
        .unwrap();

    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|path| path.depth == 2));
}

#[tokio::test]
async fn query_speaks_cgql() {
    let backend = seeded_backend().await;
    assert_eq!(backend.query_language(), QueryLanguage::Cgql);

    let rows = backend
        .query(
            r#"
            FOR d IN documents
            FILTER d.category == @category
            SORT d.title ASC
            RETURN d.title
            "#,
            std::collections::HashMap::from([("category".into(), json!("research"))]),
        )
        .await
        .unwrap();

    assert_eq!(rows, vec![json!("Alpha"), json!("Beta")]);
}

async fn seeded_backend() -> NativeBackend {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("documents", CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_collection("relationships", CollectionType::Edge)
        .await
        .unwrap();

    for doc in [
        json!({
            "_key": "a",
            "title": "Alpha",
            "category": "research",
            "embedding": [1.0, 0.0]
        }),
        json!({
            "_key": "b",
            "title": "Beta",
            "category": "research",
            "embedding": [0.8, 0.2]
        }),
        json!({
            "_key": "c",
            "title": "Gamma",
            "category": "notes",
            "embedding": [0.0, 1.0]
        }),
    ] {
        backend.create_document("documents", doc).await.unwrap();
    }

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
    backend
        .create_edge(
            "relationships",
            json!({
                "_from": "documents/b",
                "_to": "documents/c",
                "relation_type": "links",
                "confidence": 0.8
            }),
        )
        .await
        .unwrap();
    backend
        .create_edge(
            "relationships",
            json!({
                "_from": "documents/a",
                "_to": "documents/c",
                "relation_type": "weak",
                "confidence": 0.2
            }),
        )
        .await
        .unwrap();

    backend
}

#[tokio::test]
async fn traversal_min_depth_zero_includes_start() {
    let backend = seeded_backend().await;
    let paths = backend
        .traverse(
            "documents/a",
            &TraversalOpts {
                max_depth: 1,
                min_depth: 0,
                direction: Direction::Outbound,
                edge_collection: "relationships".into(),
                min_confidence: None,
                path_decay: 0.8,
            },
        )
        .await
        .unwrap();

    let depth0 = paths
        .iter()
        .find(|path| path.depth == 0)
        .expect("depth-0 path for the start vertex");
    assert!(depth0.edges.is_empty());
    assert!((depth0.score - 1.0).abs() < f64::EPSILON);
    assert!(paths.iter().any(|path| path.depth == 1));
}

#[tokio::test]
async fn create_document_with_duplicate_key_conflicts() {
    let backend = seeded_backend().await;
    let err = backend
        .create_document("documents", json!({ "_key": "a", "title": "Clone" }))
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::DocumentConflict(_)));

    let doc = backend
        .get_document("documents", "a")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(doc["title"], json!("Alpha"));
}

#[tokio::test]
async fn update_on_missing_collection_does_not_create_it() {
    let backend = NativeBackend::new();
    let err = backend
        .update_document("ghost", "a", json!({ "x": 1 }))
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::CollectionNotFound(_)));
    assert!(backend.list_documents("ghost", None, None).await.is_err());
}

#[tokio::test]
async fn collection_type_is_enforced_on_writes() {
    let backend = seeded_backend().await;

    let err = backend
        .create_edge(
            "documents",
            json!({ "_from": "documents/a", "_to": "documents/b" }),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::ValidationError(_)));

    let err = backend
        .create_document("relationships", json!({ "title": "not an edge" }))
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::ValidationError(_)));
}

#[tokio::test]
async fn shared_backend_contract() {
    let backend = NativeBackend::new();
    cognigraph_core::contract::run_all(&backend, "contract").await;
}

#[tokio::test]
async fn cgql_mutations_read_write_lifecycle() {
    use cognigraph_query::{QueryMode, parse_and_execute_backend_with_mode};
    let backend = seeded_backend().await;

    // ReadOnly mode refuses mutations (the /search/query and Lua path).
    let err = parse_and_execute_backend_with_mode(
        r#"REMOVE "a" IN documents"#,
        &backend,
        &std::collections::HashMap::new(),
        QueryMode::ReadOnly,
    )
    .await
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "mutations are not allowed on this endpoint"
    );

    let binds = std::collections::HashMap::new();
    let rw = |q: &'static str| {
        parse_and_execute_backend_with_mode(q, &backend, &binds, QueryMode::ReadWrite)
    };

    // INSERT with RETURN NEW.
    let rows = rw(r#"INSERT { _key: "m1", title: "Inserted" } INTO documents RETURN NEW.title"#)
        .await
        .unwrap();
    assert_eq!(rows, vec![json!("Inserted")]);

    // UPDATE is a partial update (merge) — title survives.
    let rows = rw(r#"UPDATE "m1" WITH { reviewed: true } IN documents RETURN { t: NEW.title, r: NEW.reviewed, old: OLD.reviewed }"#)
        .await
        .unwrap();
    assert_eq!(
        rows,
        vec![json!({ "t": "Inserted", "r": true, "old": null })]
    );

    // REPLACE swaps the whole document — reviewed is gone.
    let rows = rw(r#"REPLACE "m1" WITH { title: "Replaced" } IN documents RETURN NEW.reviewed"#)
        .await
        .unwrap();
    assert_eq!(rows, vec![json!(null)]);

    // FOR-driven bulk REMOVE with OLD projection; accepts full ids too.
    let rows = rw(r#"FOR d IN documents FILTER d.category == "research" REMOVE d._id IN documents RETURN OLD._key"#)
        .await
        .unwrap();
    assert_eq!(rows, vec![json!("a"), json!("b")]);
    assert!(
        backend
            .get_document("documents", "a")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn batch_transactions_are_atomic() {
    use cognigraph_core::BatchOp;
    let backend = seeded_backend().await;

    // Success path: mixed ops, later ops see earlier ops' effects.
    let results = backend
        .execute_batch(vec![
            BatchOp::Insert {
                collection: "documents".into(),
                doc: json!({ "_key": "t1", "n": 1 }),
            },
            BatchOp::Update {
                collection: "documents".into(),
                key: "t1".into(),
                merge: json!({ "seen": true }),
            },
            BatchOp::Delete {
                collection: "documents".into(),
                key: "b".into(),
            },
        ])
        .await
        .unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results[1]["seen"], json!(true));
    assert_eq!(results[1]["n"], json!(1), "update saw the batch insert");
    assert!(results[2].is_null());
    assert!(
        backend
            .get_document("documents", "b")
            .await
            .unwrap()
            .is_none()
    );

    // Atomicity: a failing op (conflict on existing key "a") must roll
    // back everything, including the earlier insert in the same batch.
    let err = backend
        .execute_batch(vec![
            BatchOp::Insert {
                collection: "documents".into(),
                doc: json!({ "_key": "t2", "n": 2 }),
            },
            BatchOp::Insert {
                collection: "documents".into(),
                doc: json!({ "_key": "a", "n": 3 }),
            },
        ])
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        cognigraph_core::CogniGraphError::DocumentConflict(_)
    ));
    assert!(
        backend
            .get_document("documents", "t2")
            .await
            .unwrap()
            .is_none(),
        "nothing from the failed batch applied"
    );

    // CGQL UPSERT: update branch (match by field), then insert branch.
    use cognigraph_query::{QueryMode, parse_and_execute_backend_with_mode};
    let binds = std::collections::HashMap::new();
    let rows = parse_and_execute_backend_with_mode(
        r#"UPSERT { title: "Alpha" } INSERT { title: "Alpha", fresh: true } UPDATE { upserted: true } IN documents RETURN { old_title: OLD.title, new_flag: NEW.upserted }"#,
        &backend,
        &binds,
        QueryMode::ReadWrite,
    )
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![json!({ "old_title": "Alpha", "new_flag": true })]
    );
    let rows = parse_and_execute_backend_with_mode(
        r#"UPSERT { _key: "ghost" } INSERT { _key: "ghost", born: true } UPDATE { seen: true } IN documents RETURN { old: OLD, born: NEW.born }"#,
        &backend,
        &binds,
        QueryMode::ReadWrite,
    )
    .await
    .unwrap();
    assert_eq!(rows, vec![json!({ "old": null, "born": true })]);
}

#[tokio::test]
async fn batch_transactions_support_edge_occurrences() {
    use cognigraph_core::BatchOp;

    let backend = NativeBackend::new();
    backend
        .ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();

    let rows = backend
        .execute_batch(vec![BatchOp::Insert {
            collection: "facts".into(),
            doc: json!({
                "_key": "occ-1",
                "_from": "entities/a",
                "_to": "entities/b",
                "relation_type": "LINKS",
                "evidence_chunk_id": "c1",
            }),
        }])
        .await
        .unwrap();
    assert_eq!(rows[0]["_id"], "facts/occ-1");
    assert_eq!(rows[0]["confidence"], 1.0);

    let edges = backend
        .get_edges("facts", "entities/a", Direction::Outbound)
        .await
        .unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["relation_type"], "LINKS");

    let err = backend
        .execute_batch(vec![
            BatchOp::Delete {
                collection: "facts".into(),
                key: "occ-1".into(),
            },
            BatchOp::Insert {
                collection: "facts".into(),
                doc: json!({ "_key": "invalid", "_from": "entities/a" }),
            },
        ])
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::ValidationError(_)));
    assert!(
        backend
            .get_document("facts", "occ-1")
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn nfc_text_comparison_is_explicit() {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("cafes", CollectionType::Document)
        .await
        .unwrap();
    // Store the DECOMPOSED form (e + combining acute).
    backend
        .create_document("cafes", json!({ "_key": "k", "name": "cafe\u{0301}" }))
        .await
        .unwrap();
    // Text comparisons may opt in; the stored value and references stay exact.
    let rows = backend
        .query(
            "FOR c IN cafes FILTER NORMALIZE_NFC(c.name) == \"caf\u{00E9}\" RETURN c._key",
            std::collections::HashMap::new(),
        )
        .await
        .unwrap();
    assert_eq!(rows, vec![json!("k")]);
}

#[tokio::test]
async fn upsert_edge_triple_index_survives_out_of_band_writes() {
    use cognigraph_core::Direction;
    let backend = NativeBackend::new();
    backend
        .ensure_collection("rels", cognigraph_core::CollectionType::Edge)
        .await
        .unwrap();

    // Upsert twice: one edge, updated in place (and the index warm).
    let first = backend
        .upsert_edge("rels", "docs/a", "docs/b", "rel", json!({"w": 1}))
        .await
        .unwrap();
    let second = backend
        .upsert_edge("rels", "docs/a", "docs/b", "rel", json!({"w": 2}))
        .await
        .unwrap();
    assert_eq!(first["_key"], second["_key"]);
    let edges = backend
        .get_edges("rels", "docs/a", Direction::Outbound)
        .await
        .unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["w"], 2);

    // Retarget the edge's triple through the generic document API: a stale
    // index would still match the old triple and overwrite the wrong edge.
    let key = second["_key"].as_str().unwrap().to_string();
    backend
        .update_document("rels", &key, json!({"relation_type": "other"}))
        .await
        .unwrap();
    backend
        .upsert_edge("rels", "docs/a", "docs/b", "rel", json!({"w": 3}))
        .await
        .unwrap();
    let edges = backend
        .get_edges("rels", "docs/a", Direction::Outbound)
        .await
        .unwrap();
    assert_eq!(edges.len(), 2, "retargeted edge kept, new edge created");

    // A duplicate triple created out of band with a smaller key must win the
    // next upsert (first-in-scan-order, matching the old linear scan).
    backend
        .create_edge(
            "rels",
            json!({"_key": "000", "_from": "docs/a", "_to": "docs/b", "relation_type": "rel"}),
        )
        .await
        .unwrap();
    let upserted = backend
        .upsert_edge("rels", "docs/a", "docs/b", "rel", json!({"w": 4}))
        .await
        .unwrap();
    assert_eq!(upserted["_key"], "000");
}

/// An equality on `_from`/`_to` is served from the adjacency index instead of a
/// collection scan (decision_cgql_v2_workload_gaps.md, D1). The index only
/// NARROWS the candidate set, so the results must be byte-identical to the scan
/// it replaces — including every other predicate, offset and limit.
#[tokio::test]
async fn edge_endpoint_equality_uses_adjacency_and_matches_the_scan() {
    use cognigraph_core::{CollectionType, FieldPredicate, PredicateOp};

    let backend = NativeBackend::new();
    backend
        .ensure_collection("nodes", CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_collection("links", CollectionType::Edge)
        .await
        .unwrap();
    for i in 0..40 {
        backend
            .create_document("nodes", json!({ "_key": format!("n{i}"), "i": i }))
            .await
            .unwrap();
    }
    // 30 edges into n0 (half of them tagged), 10 elsewhere.
    for i in 0..30 {
        backend
            .create_edge(
                "links",
                json!({ "_key": format!("e{i}"), "_from": format!("nodes/n{}", i + 1),
                        "_to": "nodes/n0", "tag": if i % 2 == 0 { "keep" } else { "drop" } }),
            )
            .await
            .unwrap();
    }
    for i in 30..40 {
        backend
            .create_edge(
                "links",
                json!({ "_key": format!("e{i}"), "_from": "nodes/n39",
                        "_to": "nodes/n38", "tag": "keep" }),
            )
            .await
            .unwrap();
    }

    let to_n0 = FieldPredicate {
        path: vec!["_to".into()],
        op: PredicateOp::Eq,
        value: json!("nodes/n0"),
    };
    let tagged = FieldPredicate {
        path: vec!["tag".into()],
        op: PredicateOp::Eq,
        value: json!("keep"),
    };

    // Endpoint alone.
    let rows = backend
        .list_documents_filtered("links", std::slice::from_ref(&to_n0), None, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 30, "every edge into n0, and only those");
    assert!(rows.iter().all(|e| e["_to"] == json!("nodes/n0")));

    // Endpoint + an unrelated predicate: the index narrows, the predicate still
    // applies. This is the case that would silently over-return if the extra
    // predicates were dropped on the indexed path.
    let rows = backend
        .list_documents_filtered("links", &[to_n0.clone(), tagged.clone()], None, None, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 15, "half of n0's edges are tagged: {rows:?}");
    assert!(rows.iter().all(|e| e["tag"] == json!("keep")));

    // Offset and limit behave as on the scan path.
    let page = backend
        .list_documents_filtered(
            "links",
            std::slice::from_ref(&to_n0),
            None,
            Some(4),
            Some(2),
        )
        .await
        .unwrap();
    assert_eq!(page.len(), 4);

    // A vertex with no edges yields nothing rather than falling back to a scan.
    let none = backend
        .list_documents_filtered(
            "links",
            &[FieldPredicate {
                path: vec!["_to".into()],
                op: PredicateOp::Eq,
                value: json!("nodes/n17"),
            }],
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert!(none.is_empty());

    // A DOCUMENT collection with a field literally called `_to` must not be
    // routed through the edge index.
    backend
        .ensure_collection("notes", CollectionType::Document)
        .await
        .unwrap();
    backend
        .create_document("notes", json!({ "_key": "a", "_to": "nodes/n0" }))
        .await
        .unwrap();
    let notes = backend
        .list_documents_filtered("notes", &[to_n0], None, None, None)
        .await
        .unwrap();
    assert_eq!(notes.len(), 1, "document collections keep the scan path");
}
