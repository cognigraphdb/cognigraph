//! CG-86: a unique violation raised inside a CGQL mutation keeps its
//! identity through the executor and maps to 409 on the HTTP routes.

use cognigraph_core::{CogniGraphError, CollectionType, GraphBackend, IndexDef, IndexType};
use cognigraph_native::NativeBackend;
use cognigraph_query::{ExecutionError, QueryMode, parse_and_execute_backend_with_mode};
use serde_json::json;
use std::collections::HashMap;

use super::cgql_error;

async fn backend_with_constraint() -> NativeBackend {
    let backend = NativeBackend::new();
    backend
        .ensure_collection("users", CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_index(
            "users",
            &IndexDef {
                index_type: IndexType::Persistent,
                fields: vec!["email".into()],
                unique: true,
                sparse: false,
                name: None,
            },
        )
        .await
        .unwrap();
    backend
        .create_document("users", json!({ "_key": "a", "email": "x" }))
        .await
        .unwrap();
    backend
}

#[tokio::test]
async fn cgql_insert_and_upsert_surface_the_violation_as_409() {
    let backend = backend_with_constraint().await;
    for query in [
        r#"INSERT { _key: "b", email: "x" } INTO users"#,
        r#"UPSERT { _key: "b" } INSERT { _key: "b", email: "x" } UPDATE { email: "x" } IN users"#,
        r#"UPDATE "a" WITH { email: "x" } IN users RETURN NEW"#,
    ] {
        let result = parse_and_execute_backend_with_mode(
            query,
            &backend,
            &HashMap::new(),
            QueryMode::ReadWrite,
        )
        .await;
        match result {
            Err(ExecutionError::UniqueViolation {
                collection,
                index,
                existing,
            }) => {
                assert_eq!(
                    (collection.as_str(), index.as_str(), existing.as_str()),
                    ("users", "email_unique", "a")
                );
                let mapped = cgql_error(ExecutionError::UniqueViolation {
                    collection,
                    index,
                    existing,
                });
                assert!(
                    matches!(mapped.0, CogniGraphError::UniqueViolation { .. }),
                    "{mapped:?}"
                );
            }
            Ok(rows) if query.starts_with("UPDATE") => {
                // A document may keep its own value.
                assert_eq!(rows.len(), 1);
            }
            other => panic!("{query}: {other:?}"),
        }
    }
    assert!(backend.get_document("users", "b").await.unwrap().is_none());
}

#[tokio::test]
async fn cgql_key_conflicts_map_to_409_not_500() {
    let backend = backend_with_constraint().await;
    let err = parse_and_execute_backend_with_mode(
        r#"INSERT { _key: "a", email: "fresh" } INTO users"#,
        &backend,
        &HashMap::new(),
        QueryMode::ReadWrite,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ExecutionError::Conflict(_)), "{err:?}");
    let mapped = cgql_error(err);
    assert!(
        matches!(mapped.0, CogniGraphError::DocumentConflict(_)),
        "{mapped:?}"
    );
}
