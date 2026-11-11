//! CG-106: a mutation key may be a plain key or a full id of the mutation's
//! own collection, never an id of another collection.

use std::collections::HashMap;

use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use cognigraph_query::{ExecutionError, QueryMode, parse_and_execute_backend_with_mode};
use serde_json::{Value, json};

async fn backend() -> NativeBackend {
    let backend = NativeBackend::new();
    for (collection, key, name) in [
        ("persons", "42", "person"),
        ("persons", "7", "other person"),
        ("organizations", "42", "organization"),
        ("docs", "a/b", "slash key"),
        ("docs", "b", "plain b"),
    ] {
        backend
            .create_document(collection, json!({"_key": key, "name": name}))
            .await
            .unwrap();
    }
    backend
        .create_edge(
            "member_of",
            json!({"_from": "persons/7", "_to": "organizations/42"}),
        )
        .await
        .unwrap();
    backend
}

async fn rw(backend: &NativeBackend, query: &str) -> Result<Vec<Value>, ExecutionError> {
    parse_and_execute_backend_with_mode(query, backend, &HashMap::new(), QueryMode::ReadWrite).await
}

async fn name(backend: &NativeBackend, collection: &str, key: &str) -> Option<Value> {
    backend
        .get_document(collection, key)
        .await
        .unwrap()
        .map(|doc| doc["name"].clone())
}

fn assert_outside(error: ExecutionError, id: &str, collection: &str) {
    match error {
        ExecutionError::MutationKeyOutsideCollection {
            id: got_id,
            collection: got_collection,
        } => assert_eq!((got_id.as_str(), got_collection.as_str()), (id, collection)),
        other => panic!("expected a collection mismatch, got {other}"),
    }
}

#[tokio::test]
async fn an_id_of_another_collection_never_writes_to_the_target() {
    let backend = backend().await;
    for query in [
        r#"REMOVE "organizations/42" IN persons"#,
        r#"UPDATE "organizations/42" WITH {name: "overwritten"} IN persons"#,
        r#"REPLACE "organizations/42" WITH {name: "overwritten"} IN persons"#,
    ] {
        let error = rw(&backend, query).await.unwrap_err();
        assert_outside(error, "organizations/42", "persons");
    }
    // The same-key document in the target collection is untouched.
    assert_eq!(name(&backend, "persons", "42").await, Some(json!("person")));
    assert_eq!(
        name(&backend, "organizations", "42").await,
        Some(json!("organization"))
    );
}

#[tokio::test]
async fn an_edge_endpoint_used_as_a_key_is_refused_before_any_write() {
    let backend = backend().await;
    // `_to` is `organizations/42`; the statement must not delete `persons/42`.
    let error = rw(&backend, "FOR e IN member_of REMOVE e._to IN persons")
        .await
        .unwrap_err();
    assert_outside(error, "organizations/42", "persons");
    assert_eq!(name(&backend, "persons", "42").await, Some(json!("person")));

    // The matching collection works, through the same expression.
    let rows = rw(
        &backend,
        "FOR e IN member_of REMOVE e._to IN organizations RETURN OLD.name",
    )
    .await
    .unwrap();
    assert_eq!(rows, vec![json!("organization")]);
    assert_eq!(name(&backend, "organizations", "42").await, None);
}

#[tokio::test]
async fn plain_keys_and_full_ids_of_the_target_both_work() {
    let backend = backend().await;
    let rows = rw(
        &backend,
        r#"UPDATE "42" WITH {seen: 1} IN persons RETURN NEW.name"#,
    )
    .await
    .unwrap();
    assert_eq!(rows, vec![json!("person")]);
    let rows = rw(
        &backend,
        r#"UPDATE "persons/42" WITH {seen: 2} IN persons RETURN NEW.seen"#,
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    let rows = rw(
        &backend,
        "FOR p IN persons FILTER p._key == \"7\" REMOVE p._id IN persons RETURN OLD.name",
    )
    .await
    .unwrap();
    assert_eq!(rows, vec![json!("other person")]);
}

#[tokio::test]
async fn a_key_containing_a_slash_needs_the_full_id_and_splits_from_the_left() {
    let backend = backend().await;
    // `a/b` alone reads as an id of collection `a`. It used to delete `docs/b`.
    let error = rw(&backend, r#"REMOVE "a/b" IN docs"#).await.unwrap_err();
    assert_outside(error, "a/b", "docs");
    assert_eq!(name(&backend, "docs", "b").await, Some(json!("plain b")));
    assert_eq!(
        name(&backend, "docs", "a/b").await,
        Some(json!("slash key"))
    );

    let rows = rw(&backend, r#"REMOVE "docs/a/b" IN docs RETURN OLD.name"#)
        .await
        .unwrap();
    assert_eq!(rows, vec![json!("slash key")]);
    assert_eq!(name(&backend, "docs", "b").await, Some(json!("plain b")));
}

#[tokio::test]
async fn degenerate_ids_are_refused() {
    let backend = backend().await;
    for (key, id) in [
        (r#""persons/""#, "persons/"),
        (r#""/42""#, "/42"),
        (r#""/""#, "/"),
        (r#""Persons/42""#, "Persons/42"),
        (r#""persons /42""#, "persons /42"),
    ] {
        let error = rw(&backend, &format!("REMOVE {key} IN persons"))
            .await
            .unwrap_err();
        assert_outside(error, id, "persons");
    }
    // A non-string key keeps its own error.
    let error = rw(&backend, "REMOVE 42 IN persons").await.unwrap_err();
    assert!(
        matches!(error, ExecutionError::InvalidDocumentKey),
        "{error}"
    );
    assert_eq!(name(&backend, "persons", "42").await, Some(json!("person")));
}
