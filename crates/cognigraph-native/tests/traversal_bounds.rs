//! CG-110: a traversal is bounded by the number of paths it expands, in every
//! storage mode, whatever depth the caller asks for.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use cognigraph_core::{
    CogniGraphError, Direction, GraphBackend, MAX_TRAVERSAL_PATHS, TraversalOpts,
};
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::json;

struct TempDir(PathBuf);
impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("cognigraph-traversal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(path.join("store")).unwrap();
        Self(path)
    }
    fn open(&self, mode: StorageMode) -> NativeBackend {
        NativeBackend::open_with_modes(
            self.0.join("store/data.redb"),
            VectorMode::Sidecar,
            mode,
            8 << 20,
        )
        .unwrap()
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn opts(max_depth: u32, direction: Direction) -> TraversalOpts {
    TraversalOpts {
        max_depth,
        min_depth: 1,
        direction,
        edge_collection: "links".into(),
        min_confidence: None,
        path_decay: 1.0,
    }
}

/// Every pair of `nodes` documents is connected: the densest small graph.
async fn complete_graph(backend: &NativeBackend, nodes: usize) {
    for i in 0..nodes {
        backend
            .create_document("v", json!({"_key": format!("n{i}")}))
            .await
            .unwrap();
    }
    for i in 0..nodes {
        for j in i + 1..nodes {
            backend
                .create_edge(
                    "links",
                    json!({"_from": format!("v/n{i}"), "_to": format!("v/n{j}")}),
                )
                .await
                .unwrap();
        }
    }
}

fn assert_over_budget(error: CogniGraphError, context: &str) {
    match error {
        CogniGraphError::ValidationError(message) => {
            assert!(
                message.contains("expands more than"),
                "{context}: {message}"
            );
            assert!(message.contains("`v/n0`"), "{context}: {message}");
        }
        other => panic!("{context}: expected a validation error, got {other}"),
    }
}

#[tokio::test]
async fn a_dense_graph_is_served_within_the_budget_and_refused_beyond_it() {
    let dir = TempDir::new();
    let backends = [
        ("memory", NativeBackend::new()),
        ("resident", dir.open(StorageMode::Resident)),
    ];
    let paged_dir = TempDir::new();
    let paged = [("paged", paged_dir.open(StorageMode::Paged))];
    for (name, backend) in backends.iter().chain(paged.iter()) {
        complete_graph(backend, 10).await;
        // Simple paths of length 1..=4 from one vertex of a 10-node complete
        // graph: 9 + 9*8 + 9*8*7 + 9*8*7*6.
        let paths = backend
            .traverse("v/n0", &opts(4, Direction::Any))
            .await
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(paths.len(), 9 + 72 + 504 + 3024, "{name}");

        // Depth 5 alone adds 15,120 paths; every deeper request must stop at
        // the same point, quickly, instead of growing with the depth asked.
        for depth in [5, 6, 9, 10, 50, 1_000_000, u32::MAX] {
            let started = Instant::now();
            let error = backend
                .traverse("v/n0", &opts(depth, Direction::Any))
                .await
                .expect_err(&format!("{name}: depth {depth} must be refused"));
            assert_over_budget(error, &format!("{name} depth {depth}"));
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "{name}: depth {depth} took {:?}",
                started.elapsed()
            );
        }
    }
}

#[tokio::test]
async fn depth_alone_is_not_refused_on_a_sparse_graph() {
    let backend = NativeBackend::new();
    for i in 0..=40 {
        backend
            .create_document("v", json!({"_key": format!("n{i}")}))
            .await
            .unwrap();
    }
    for i in 0..40 {
        backend
            .create_edge(
                "links",
                json!({"_from": format!("v/n{i}"), "_to": format!("v/n{}", i + 1)}),
            )
            .await
            .unwrap();
    }
    // A chain has one path per depth, so even an absurd depth is cheap.
    let paths = backend
        .traverse("v/n0", &opts(u32::MAX, Direction::Outbound))
        .await
        .unwrap();
    assert_eq!(paths.len(), 40);
    assert_eq!(paths.last().unwrap().depth, 40);
}

#[tokio::test]
async fn the_budget_is_exact_at_its_boundary() {
    let backend = NativeBackend::new();
    backend
        .create_document("v", json!({"_key": "n0"}))
        .await
        .unwrap();
    // A hub with exactly the budget's number of spokes: one path per spoke.
    for i in 1..=MAX_TRAVERSAL_PATHS {
        backend
            .create_edge(
                "links",
                json!({"_from": "v/n0", "_to": format!("v/leaf{i}")}),
            )
            .await
            .unwrap();
    }
    let paths = backend
        .traverse("v/n0", &opts(1, Direction::Outbound))
        .await
        .unwrap();
    assert_eq!(paths.len(), MAX_TRAVERSAL_PATHS);

    backend
        .create_edge("links", json!({"_from": "v/n0", "_to": "v/one-too-many"}))
        .await
        .unwrap();
    let error = backend
        .traverse("v/n0", &opts(1, Direction::Outbound))
        .await
        .unwrap_err();
    assert_over_budget(error, "hub");

    // A confidence floor that prunes the extra spoke brings it back in budget.
    backend
        .create_edge(
            "links",
            json!({"_from": "v/n0", "_to": "v/weak", "confidence": 0.1}),
        )
        .await
        .unwrap();
    let mut narrowed = opts(1, Direction::Inbound);
    narrowed.min_confidence = Some(0.5);
    assert!(
        backend
            .traverse("v/n0", &narrowed)
            .await
            .unwrap()
            .is_empty()
    );
}
