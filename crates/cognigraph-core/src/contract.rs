//! Conformance suite for `GraphBackend` implementations.
//!
//! Backend crates call these functions from their tests so every backend
//! exposes the same observable semantics. Each function creates its own
//! collections under the given prefix and drops them afterwards, so the
//! suite is safe to run against a database that holds other data.
//!
//! Behavior that is deliberately backend-specific (for example collection
//! type enforcement, which ArangoDB does not apply to document inserts) is
//! tested in the individual backend crates instead of here.

use serde_json::json;

use crate::error::CogniGraphError;
use crate::traits::GraphBackend;
use crate::types::{CollectionType, Direction, TraversalOpts, VectorSearchOpts};

mod traversal;
pub use traversal::traversal_confidence_contract;

/// Document create/get/update/replace/delete/list semantics.
pub async fn document_crud_contract(backend: &dyn GraphBackend, prefix: &str) {
    let col = format!("{prefix}_docs");
    backend.drop_collection(&col).await.ok();
    backend
        .ensure_collection(&col, CollectionType::Document)
        .await
        .unwrap();

    let id = backend
        .create_document(&col, json!({ "_key": "one", "title": "One" }))
        .await
        .unwrap();
    assert_eq!(id.full_id(), format!("{col}/one"));

    let doc = backend.get_document(&col, "one").await.unwrap().unwrap();
    assert_eq!(doc["title"], json!("One"));

    let updated = backend
        .update_document(&col, "one", json!({ "extra": true }))
        .await
        .unwrap();
    assert_eq!(updated["title"], json!("One"), "update must merge");
    assert_eq!(updated["extra"], json!(true));

    let replaced = backend
        .replace_document(&col, "one", json!({ "title": "Two" }))
        .await
        .unwrap();
    assert_eq!(replaced["title"], json!("Two"));
    assert!(
        replaced.get("extra").is_none_or(serde_json::Value::is_null),
        "replace must not merge"
    );

    let listed = backend.list_documents(&col, Some(10), None).await.unwrap();
    assert_eq!(listed.len(), 1);

    assert!(backend.delete_document(&col, "one").await.unwrap());
    assert!(!backend.delete_document(&col, "one").await.unwrap());
    assert!(backend.get_document(&col, "one").await.unwrap().is_none());

    backend.drop_collection(&col).await.unwrap();
}

/// The first document or edge write materializes a missing collection with
/// the corresponding collection type.
pub async fn implicit_collection_creation_contract(backend: &dyn GraphBackend, prefix: &str) {
    let docs = format!("{prefix}_implicit_docs");
    let rels = format!("{prefix}_implicit_rels");
    let upsert_rels = format!("{prefix}_implicit_upsert_rels");
    backend.drop_collection(&upsert_rels).await.ok();
    backend.drop_collection(&rels).await.ok();
    backend.drop_collection(&docs).await.ok();

    let doc_id = backend
        .create_document(&docs, json!({ "_key": "one", "title": "One" }))
        .await
        .unwrap();
    assert_eq!(doc_id.full_id(), format!("{docs}/one"));
    assert!(backend.get_document(&docs, "one").await.unwrap().is_some());

    let edge_id = backend
        .create_edge(
            &rels,
            json!({
                "_key": "self",
                "_from": doc_id.full_id(),
                "_to": doc_id.full_id(),
                "relation_type": "self"
            }),
        )
        .await
        .unwrap();
    assert_eq!(edge_id.full_id(), format!("{rels}/self"));
    let edges = backend
        .get_edges(&rels, &doc_id.full_id(), Direction::Any)
        .await
        .unwrap();
    assert_eq!(edges.len(), 1);

    let first = backend
        .upsert_edge(
            &upsert_rels,
            &doc_id.full_id(),
            &doc_id.full_id(),
            "upserted_self",
            json!({ "weight": 1 }),
        )
        .await
        .unwrap();
    let second = backend
        .upsert_edge(
            &upsert_rels,
            &doc_id.full_id(),
            &doc_id.full_id(),
            "upserted_self",
            json!({ "weight": 2 }),
        )
        .await
        .unwrap();
    assert_eq!(first["_key"], second["_key"]);
    assert_eq!(second["weight"], json!(2));
    let upserted_edges = backend
        .get_edges(&upsert_rels, &doc_id.full_id(), Direction::Any)
        .await
        .unwrap();
    assert_eq!(upserted_edges.len(), 1);

    backend.drop_collection(&upsert_rels).await.unwrap();
    backend.drop_collection(&rels).await.unwrap();
    backend.drop_collection(&docs).await.unwrap();
}

/// An omitted list limit is an unbounded scan, including when an offset is
/// supplied. This guards backends against silently substituting a page-size
/// default and also exercises the default filtered-scan implementation.
pub async fn unbounded_scan_contract(backend: &dyn GraphBackend, prefix: &str) {
    use crate::types::{FieldPredicate, PredicateOp};

    const ROW_COUNT: usize = 101;

    let col = format!("{prefix}_unbounded");
    backend.drop_collection(&col).await.ok();
    backend
        .ensure_collection(&col, CollectionType::Document)
        .await
        .unwrap();

    for index in 0..ROW_COUNT {
        backend
            .create_document(
                &col,
                json!({
                    "_key": format!("row_{index:03}"),
                    "included": true,
                }),
            )
            .await
            .unwrap();
    }

    let rows = backend.list_documents(&col, None, None).await.unwrap();
    assert_eq!(rows.len(), ROW_COUNT, "None must not imply a row cap");

    let rows = backend.list_documents(&col, None, Some(100)).await.unwrap();
    assert_eq!(rows.len(), 1, "an offset without a limit remains unbounded");

    let rows = backend
        .list_documents_filtered(
            &col,
            &[FieldPredicate {
                path: vec!["included".into()],
                op: PredicateOp::Eq,
                value: json!(true),
            }],
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        ROW_COUNT,
        "an unbounded filtered scan must inspect every row"
    );

    backend.drop_collection(&col).await.unwrap();
}

/// Creating a document whose `_key` already exists is a conflict, and the
/// original document survives.
pub async fn create_conflict_contract(backend: &dyn GraphBackend, prefix: &str) {
    let col = format!("{prefix}_conflict");
    backend.drop_collection(&col).await.ok();
    backend
        .ensure_collection(&col, CollectionType::Document)
        .await
        .unwrap();

    backend
        .create_document(&col, json!({ "_key": "dup", "title": "Original" }))
        .await
        .unwrap();
    let err = backend
        .create_document(&col, json!({ "_key": "dup", "title": "Clone" }))
        .await
        .unwrap_err();
    assert!(
        matches!(err, CogniGraphError::DocumentConflict(_)),
        "expected DocumentConflict, got {err:?}"
    );
    let doc = backend.get_document(&col, "dup").await.unwrap().unwrap();
    assert_eq!(doc["title"], json!("Original"));

    backend.drop_collection(&col).await.unwrap();
}

/// Updating a document in a collection that does not exist reports
/// `CollectionNotFound` and must not create the collection as a side effect.
pub async fn missing_collection_update_contract(backend: &dyn GraphBackend, prefix: &str) {
    let col = format!("{prefix}_absent");
    backend.drop_collection(&col).await.ok();

    let err = backend
        .update_document(&col, "k", json!({ "x": 1 }))
        .await
        .unwrap_err();
    assert!(
        matches!(err, CogniGraphError::CollectionNotFound(_)),
        "expected CollectionNotFound, got {err:?}"
    );
    assert!(
        backend.list_documents(&col, None, None).await.is_err(),
        "failed update must not create the collection"
    );
}

/// Edge directionality and traversal depth semantics, including the
/// depth-0 start vertex.
pub async fn edges_and_traversal_contract(backend: &dyn GraphBackend, prefix: &str) {
    let docs = format!("{prefix}_verts");
    let rels = format!("{prefix}_rels");
    backend.drop_collection(&rels).await.ok();
    backend.drop_collection(&docs).await.ok();
    backend
        .ensure_collection(&docs, CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_collection(&rels, CollectionType::Edge)
        .await
        .unwrap();

    for key in ["a", "b", "c"] {
        backend
            .create_document(&docs, json!({ "_key": key }))
            .await
            .unwrap();
    }
    backend
        .create_edge(
            &rels,
            json!({
                "_from": format!("{docs}/a"),
                "_to": format!("{docs}/b"),
                "relation_type": "r",
                "confidence": 0.9
            }),
        )
        .await
        .unwrap();
    backend
        .create_edge(
            &rels,
            json!({
                "_from": format!("{docs}/b"),
                "_to": format!("{docs}/c"),
                "relation_type": "r",
                "confidence": 0.8
            }),
        )
        .await
        .unwrap();

    let out = backend
        .get_edges(&rels, &format!("{docs}/a"), Direction::Outbound)
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    let inbound = backend
        .get_edges(&rels, &format!("{docs}/b"), Direction::Inbound)
        .await
        .unwrap();
    assert_eq!(inbound.len(), 1);
    let any = backend
        .get_edges(&rels, &format!("{docs}/b"), Direction::Any)
        .await
        .unwrap();
    assert_eq!(any.len(), 2);

    let paths = backend
        .traverse(
            &format!("{docs}/a"),
            &TraversalOpts {
                max_depth: 2,
                min_depth: 0,
                direction: Direction::Outbound,
                edge_collection: rels.clone(),
                min_confidence: None,
                path_decay: 0.8,
            },
        )
        .await
        .unwrap();
    let depths: Vec<usize> = paths.iter().map(|path| path.depth).collect();
    assert!(depths.contains(&0), "depth 0 must include the start vertex");
    assert!(depths.contains(&1));
    assert!(depths.contains(&2));
    let depth0 = paths.iter().find(|path| path.depth == 0).unwrap();
    assert!(depth0.edges.is_empty());
    assert!((depth0.score - 1.0).abs() < 1e-12);

    let depth1 = paths.iter().find(|path| path.depth == 1).unwrap();
    assert!(
        (depth1.score - 0.9 * 0.8).abs() < 1e-12,
        "depth-1 score must include edge confidence and one decay: {}",
        depth1.score
    );
    let depth2 = paths.iter().find(|path| path.depth == 2).unwrap();
    assert!(
        (depth2.score - 0.9 * 0.8 * 0.8_f64.powi(2)).abs() < 1e-12,
        "depth-2 score must include every edge confidence and decay per hop: {}",
        depth2.score
    );

    backend.drop_collection(&rels).await.unwrap();
    backend.drop_collection(&docs).await.unwrap();
}

/// Vector search returns hits in descending score order, respects the
/// limit, and puts the best match first.
pub async fn vector_search_contract(backend: &dyn GraphBackend, prefix: &str) {
    let col = format!("{prefix}_vecs");
    backend.drop_collection(&col).await.ok();
    backend
        .ensure_collection(&col, CollectionType::Document)
        .await
        .unwrap();

    for (key, embedding) in [
        ("a", json!([1.0, 0.0])),
        ("b", json!([0.6, 0.8])),
        ("c", json!([0.0, 1.0])),
    ] {
        backend
            .create_document(&col, json!({ "_key": key, "embedding": embedding }))
            .await
            .unwrap();
    }

    let hits = backend
        .vector_search(
            &col,
            &[1.0, 0.0],
            &VectorSearchOpts {
                threshold: Some(0.0),
                limit: 2,
                model_name: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert!(hits[0].score >= hits[1].score);
    assert_eq!(hits[0].document["_key"], json!("a"));

    backend.drop_collection(&col).await.unwrap();
}

/// Filtered scan (filter pushdown): predicates apply with CGQL semantics
/// (numeric value equality, missing path = null, mixed types never order),
/// paging applies after filtering, projection keeps `_key`/`_id`.
pub async fn filtered_scan_contract(backend: &dyn GraphBackend, prefix: &str) {
    use crate::types::{FieldPredicate, PredicateOp};
    let col = format!("{prefix}_filtered");
    let _ = backend.drop_collection(&col).await;
    backend
        .ensure_collection(&col, CollectionType::Document)
        .await
        .unwrap();
    for (key, doc) in [
        ("a", json!({"_key": "a", "n": 2, "tag": "x", "extra": true})),
        ("b", json!({"_key": "b", "n": 2.0, "tag": "y"})),
        ("c", json!({"_key": "c", "n": 10, "tag": "x"})),
        ("d", json!({"_key": "d", "tag": "x"})), // n missing -> null
    ] {
        let _ = key;
        backend.create_document(&col, doc).await.unwrap();
    }
    let keys = |rows: Vec<serde_json::Value>| -> Vec<String> {
        let mut keys: Vec<String> = rows
            .iter()
            .filter_map(|d| d.get("_key").and_then(serde_json::Value::as_str))
            .map(str::to_string)
            .collect();
        keys.sort();
        keys
    };

    // Numeric value equality: stored int 2 and stored float 2.0 both match.
    let pred = |op, value| {
        vec![FieldPredicate {
            path: vec!["n".into()],
            op,
            value,
        }]
    };
    let rows = backend
        .list_documents_filtered(&col, &pred(PredicateOp::Eq, json!(2.0)), None, None, None)
        .await
        .unwrap();
    assert_eq!(keys(rows), ["a", "b"], "numeric value equality");

    // Missing path reads null: never orders, but equals null.
    let rows = backend
        .list_documents_filtered(&col, &pred(PredicateOp::Ge, json!(0)), None, None, None)
        .await
        .unwrap();
    assert_eq!(keys(rows), ["a", "b", "c"], "null never orders");
    let rows = backend
        .list_documents_filtered(&col, &pred(PredicateOp::Eq, json!(null)), None, None, None)
        .await
        .unwrap();
    assert_eq!(keys(rows), ["d"], "missing field equals null");

    // IN pushdown: membership by CGQL value equality.
    let rows = backend
        .list_documents_filtered(
            &col,
            &pred(PredicateOp::In, json!([2.0, 10])),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(keys(rows), ["a", "b", "c"], "IN matches by numeric value");

    // Conjunction + paging AFTER filtering + projection contract.
    let predicates = vec![
        FieldPredicate {
            path: vec!["tag".into()],
            op: PredicateOp::Eq,
            value: json!("x"),
        },
        FieldPredicate {
            path: vec!["n".into()],
            op: PredicateOp::Ge,
            value: json!(1),
        },
    ];
    let fields = ["n".to_string()];
    let rows = backend
        .list_documents_filtered(&col, &predicates, Some(&fields), Some(1), Some(1))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "offset 1 limit 1 after filtering (a, c)");
    let row = &rows[0];
    assert!(row.get("_key").is_some(), "projection keeps _key");
    assert!(row.get("n").is_some(), "projection keeps requested field");

    backend.drop_collection(&col).await.unwrap();
}

/// Keyset scan semantics: pages are ordered by `_key`, the cursor is
/// exclusive, insertion order is irrelevant, and projection preserves the
/// identity fields required to continue the scan.
pub async fn after_key_scan_contract(backend: &dyn GraphBackend, prefix: &str) {
    let col = format!("{prefix}_after_key");
    let _ = backend.drop_collection(&col).await;
    backend
        .ensure_collection(&col, CollectionType::Document)
        .await
        .unwrap();

    for key in ["delta", "alpha", "charlie", "bravo"] {
        backend
            .create_document(
                &col,
                json!({
                    "_key": key,
                    "marker": format!("marker-{key}"),
                    "unrequested": true,
                }),
            )
            .await
            .unwrap();
    }

    let fields = ["marker".to_string()];
    let first = backend
        .list_documents_after_key(&col, None, &fields, 2)
        .await
        .unwrap();
    assert_eq!(
        first
            .iter()
            .map(|row| row["_key"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["alpha", "bravo"]
    );
    assert!(first.iter().all(|row| row.get("_id").is_some()));
    assert!(first.iter().all(|row| row.get("marker").is_some()));

    let second = backend
        .list_documents_after_key(&col, Some("bravo"), &fields, 2)
        .await
        .unwrap();
    assert_eq!(
        second
            .iter()
            .map(|row| row["_key"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["charlie", "delta"]
    );

    let between = backend
        .list_documents_after_key(&col, Some("c"), &fields, 10)
        .await
        .unwrap();
    assert_eq!(
        between
            .iter()
            .map(|row| row["_key"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["charlie", "delta"]
    );
    assert!(
        backend
            .list_documents_after_key(&col, Some("delta"), &fields, 10)
            .await
            .unwrap()
            .is_empty()
    );

    backend.drop_collection(&col).await.unwrap();
}

/// Run the full conformance suite with a shared prefix.
pub async fn run_all(backend: &dyn GraphBackend, prefix: &str) {
    document_crud_contract(backend, prefix).await;
    implicit_collection_creation_contract(backend, prefix).await;
    unbounded_scan_contract(backend, prefix).await;
    create_conflict_contract(backend, prefix).await;
    missing_collection_update_contract(backend, prefix).await;
    edges_and_traversal_contract(backend, prefix).await;
    traversal_confidence_contract(backend, prefix).await;
    vector_search_contract(backend, prefix).await;
    filtered_scan_contract(backend, prefix).await;
    after_key_scan_contract(backend, prefix).await;
}
