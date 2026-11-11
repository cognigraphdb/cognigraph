//! CG-109: caller-supplied result counts are capped on every search route.

use axum::Json;
use axum::extract::State;
use cognigraph_core::CogniGraphError;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

use super::*;
use crate::state::AppState;

fn refused(result: Result<Json<Value>, AppError>, name: &str) {
    match result {
        Err(AppError(CogniGraphError::ValidationError(message))) => {
            assert!(message.contains(&format!("`{name}`")), "{message}");
            assert!(message.contains("the maximum is"), "{message}");
        }
        Err(other) => panic!("expected a validation error for {name}, got {other:?}"),
        Ok(_) => panic!("{name} above the cap was accepted"),
    }
}

async fn state() -> AppState {
    let state = AppState::new(NativeBackend::new());
    for (key, title) in [("a", "alpha one"), ("b", "alpha two")] {
        state
            .backend
            .create_document("notes", json!({"_key": key, "title": title}))
            .await
            .unwrap();
    }
    state
}

fn body<T: serde::de::DeserializeOwned>(value: Value) -> Json<T> {
    Json(serde_json::from_value(value).unwrap())
}

#[tokio::test]
async fn text_search_caps_the_limit_and_serves_the_boundary() {
    let state = state().await;
    for limit in [MAX_SEARCH_LIMIT + 1, u32::MAX as usize, usize::MAX] {
        refused(
            basic::text_search(
                State(state.clone()),
                body(json!({"collection": "notes", "query": "alpha", "fields": ["title"], "limit": limit})),
            )
            .await,
            "limit",
        );
    }
    let Json(at_cap) = basic::text_search(
        State(state.clone()),
        body(json!({"collection": "notes", "query": "alpha", "fields": ["title"], "limit": MAX_SEARCH_LIMIT})),
    )
    .await
    .unwrap();
    assert_eq!(at_cap["count"], json!(2));
    // Field names the index library would assert on are a 400, not a panic.
    for fields in [json!([""]), json!(["-title"])] {
        let error = basic::text_search(
            State(state.clone()),
            body(json!({"collection": "notes", "query": "alpha", "fields": fields})),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(error.0, CogniGraphError::ValidationError(_)),
            "{error:?}"
        );
    }
}

/// The cap is checked before anything else, so these routes answer 400 for an
/// absurd count even on a server with no embedding provider configured.
#[tokio::test]
async fn every_other_search_route_caps_its_counts_first() {
    let state = state().await;
    let over = MAX_SEARCH_LIMIT + 1;

    refused(
        basic::vector_search(
            State(state.clone()),
            body(json!({"vector": [1.0, 0.0], "limit": over})),
        )
        .await,
        "limit",
    );
    refused(
        hybrid::hybrid_search(
            State(state.clone()),
            body(json!({"query": "alpha", "documents_collection": "notes",
                        "search_fields": ["title"], "limit": usize::MAX})),
        )
        .await,
        "limit",
    );
    refused(
        semantic::semantic_search(
            State(state.clone()),
            body(json!({"query": "alpha", "limit": over})),
        )
        .await,
        "limit",
    );
    for (field, value, name) in [
        ("limit", over, "limit"),
        ("seed_limit", over, "seed_limit"),
        (
            "graph_facts_limit",
            MAX_GRAPH_FACTS_LIMIT + 1,
            "graph_facts_limit",
        ),
        ("seed_limit", usize::MAX, "seed_limit"),
    ] {
        let mut request = json!({"query": "alpha"});
        request[field] = json!(value);
        refused(
            graph_augmented::graph_augmented_search(State(state.clone()), body(request)).await,
            name,
        );
    }
}

/// CG-110: the graph-augmented route caps traversal depth before it embeds.
#[tokio::test]
async fn graph_augmented_search_caps_traversal_depth() {
    let state = state().await;
    for depth in [cognigraph_core::MAX_TRAVERSAL_DEPTH + 1, u32::MAX] {
        refused(
            graph_augmented::graph_augmented_search(
                State(state.clone()),
                body(json!({"query": "alpha", "max_depth": depth})),
            )
            .await,
            "max_depth",
        );
    }
}
