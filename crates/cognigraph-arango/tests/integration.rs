/// Integration tests for ArangoBackend.
///
/// These tests require a running ArangoDB instance.
/// Set ARANGO_URL, ARANGO_DB, ARANGO_USER, ARANGO_PASSWORD environment variables.
///
/// Run: cargo test -p cognigraph-arango --test integration -- --nocapture
use cognigraph_arango::{ArangoAuth, ArangoBackend, ArangoClient, VectorSearchMode};
use cognigraph_core::{CollectionType, Direction, GraphBackend};
use std::collections::HashMap;

#[path = "integration/vector_search.rs"]
mod vector_search;

fn backend() -> Option<ArangoBackend> {
    let url = std::env::var("ARANGO_URL").unwrap_or_else(|_| "http://localhost:8529".into());
    let db = std::env::var("ARANGO_DB").unwrap_or_else(|_| "cognigraph_test".into());
    let user = std::env::var("ARANGO_USER").unwrap_or_else(|_| "root".into());
    let password = match std::env::var("ARANGO_PASSWORD") {
        Ok(p) => p,
        Err(_) => return None, // Skip tests if no password configured
    };

    // Honor COGNIGRAPH_VECTOR_SEARCH_MODE like the server does, so the contract
    // suite can exercise either vector search path.
    let vector_mode = match std::env::var("COGNIGRAPH_VECTOR_SEARCH_MODE").as_deref() {
        Ok("fallback") => VectorSearchMode::Fallback,
        _ => VectorSearchMode::Native,
    };

    Some(
        ArangoBackend::new(ArangoClient::new(
            url,
            db,
            ArangoAuth::Basic {
                username: user,
                password,
            },
        ))
        .with_vector_mode(vector_mode),
    )
}

#[tokio::test]
async fn test_document_crud() {
    let Some(backend) = backend() else {
        eprintln!("Skipping: ARANGO_PASSWORD not set");
        return;
    };

    let collection = "test_docs";

    // Setup
    backend
        .ensure_collection(collection, CollectionType::Document)
        .await
        .unwrap();

    // Create
    let doc = serde_json::json!({
        "title": "Test Document",
        "content": "Hello from Rust CogniGraph",
        "category": "test"
    });
    let id = backend.create_document(collection, doc).await.unwrap();
    assert_eq!(id.collection, collection);
    assert!(!id.key.is_empty());

    // Read
    let fetched = backend.get_document(collection, &id.key).await.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched["title"], "Test Document");

    // Update
    let updated = backend
        .update_document(
            collection,
            &id.key,
            serde_json::json!({"category": "updated"}),
        )
        .await
        .unwrap();
    assert_eq!(updated["category"], "updated");

    // List
    let docs = backend
        .list_documents(collection, Some(10), None)
        .await
        .unwrap();
    assert!(!docs.is_empty());

    // Delete
    let deleted = backend.delete_document(collection, &id.key).await.unwrap();
    assert!(deleted);

    // Read after delete
    let gone = backend.get_document(collection, &id.key).await.unwrap();
    assert!(gone.is_none());

    // Delete non-existent
    let deleted_again = backend.delete_document(collection, &id.key).await.unwrap();
    assert!(!deleted_again);

    // Cleanup
    backend.drop_collection(collection).await.unwrap();
}

#[tokio::test]
async fn test_edge_operations() {
    let Some(backend) = backend() else {
        eprintln!("Skipping: ARANGO_PASSWORD not set");
        return;
    };

    let docs_col = "test_edge_docs";
    let edges_col = "test_edge_relations";

    backend
        .ensure_collection(docs_col, CollectionType::Document)
        .await
        .unwrap();
    backend
        .ensure_collection(edges_col, CollectionType::Edge)
        .await
        .unwrap();

    // Create two documents
    let id_a = backend
        .create_document(docs_col, serde_json::json!({"title": "Doc A"}))
        .await
        .unwrap();
    let id_b = backend
        .create_document(docs_col, serde_json::json!({"title": "Doc B"}))
        .await
        .unwrap();

    let from = id_a.full_id();
    let to = id_b.full_id();

    // Upsert edge
    let edge = backend
        .upsert_edge(
            edges_col,
            &from,
            &to,
            "related_to",
            serde_json::json!({"confidence": 0.9}),
        )
        .await
        .unwrap();
    assert_eq!(edge["relation_type"], "related_to");

    // Upsert again (should update, not duplicate)
    let edge2 = backend
        .upsert_edge(
            edges_col,
            &from,
            &to,
            "related_to",
            serde_json::json!({"confidence": 0.95}),
        )
        .await
        .unwrap();
    assert_eq!(edge2["_key"], edge["_key"]); // Same edge

    // Get outbound edges
    let outbound = backend
        .get_edges(edges_col, &from, Direction::Outbound)
        .await
        .unwrap();
    assert_eq!(outbound.len(), 1);

    // Get inbound edges
    let inbound = backend
        .get_edges(edges_col, &to, Direction::Inbound)
        .await
        .unwrap();
    assert_eq!(inbound.len(), 1);

    // Cleanup
    backend.drop_collection(edges_col).await.unwrap();
    backend.drop_collection(docs_col).await.unwrap();
}

#[tokio::test]
async fn test_raw_query() {
    let Some(backend) = backend() else {
        eprintln!("Skipping: ARANGO_PASSWORD not set");
        return;
    };

    // Simple AQL without collections
    let results = backend
        .query("RETURN { answer: 42 }", HashMap::new())
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["answer"], 42);
}

#[tokio::test]
async fn test_schema_idempotent() {
    let Some(backend) = backend() else {
        eprintln!("Skipping: ARANGO_PASSWORD not set");
        return;
    };

    // Initialize twice — should not fail
    cognigraph_arango::database::initialize(&backend)
        .await
        .unwrap();
    cognigraph_arango::database::initialize(&backend)
        .await
        .unwrap();
}

#[tokio::test]
async fn shared_backend_contract() {
    let Some(backend) = backend() else {
        eprintln!("Skipping: ARANGO_PASSWORD not set");
        return;
    };
    cognigraph_core::contract::run_all(&backend, "contract_test").await;
}

#[tokio::test]
async fn traversal_confidence_contract() {
    let Some(backend) = backend() else {
        eprintln!("Skipping: ARANGO_PASSWORD not set");
        return;
    };
    cognigraph_core::contract::traversal_confidence_contract(&backend, "cg19_test").await;
}
