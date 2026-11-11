//! CG-110: the typed traversal route caps depth and surfaces the path budget
//! as a request error.

use super::*;
use crate::error::AppError;
use cognigraph_core::{CogniGraphError, MAX_TRAVERSAL_DEPTH};
use cognigraph_native::NativeBackend;
use serde_json::json;

fn request(max_depth: u32) -> TraverseRequest {
    TraverseRequest {
        start_vertex: "v/n0".into(),
        edge_collection: "links".into(),
        max_depth,
        min_depth: 1,
        direction: Direction::Any,
        min_confidence: None,
        path_decay: 1.0,
    }
}

fn validation(error: AppError) -> String {
    match error.0 {
        CogniGraphError::ValidationError(message) => message,
        other => panic!("expected a validation error, got {other}"),
    }
}

#[tokio::test]
async fn traverse_refuses_depth_above_the_cap_and_a_traversal_past_its_budget() {
    let state = AppState::new(NativeBackend::new());
    for i in 0..10 {
        state
            .backend
            .create_document("v", json!({"_key": format!("n{i}")}))
            .await
            .unwrap();
    }
    for i in 0..10 {
        for j in i + 1..10 {
            state
                .backend
                .create_edge(
                    "links",
                    json!({"_from": format!("v/n{i}"), "_to": format!("v/n{j}")}),
                )
                .await
                .unwrap();
        }
    }

    for depth in [MAX_TRAVERSAL_DEPTH + 1, 50, u32::MAX] {
        let message = validation(
            traverse(State(state.clone()), Json(request(depth)))
                .await
                .unwrap_err(),
        );
        assert!(message.contains("`max_depth`"), "{message}");
        assert!(message.contains("the maximum is"), "{message}");
    }

    // Within the depth cap, a dense graph still stops at the path budget.
    let message = validation(
        traverse(State(state.clone()), Json(request(MAX_TRAVERSAL_DEPTH)))
            .await
            .unwrap_err(),
    );
    assert!(message.contains("expands more than"), "{message}");

    let Json(body) = traverse(State(state), Json(request(3))).await.unwrap();
    assert_eq!(body["count"], json!(9 + 72 + 504));
}
