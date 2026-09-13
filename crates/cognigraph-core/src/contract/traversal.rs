//! Confidence filtering cases shared with the release HTTP regression harness.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{CollectionType, Direction, GraphBackend, TraversalOpts};

#[derive(Deserialize)]
struct Fixture {
    edges: Vec<Value>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    start: String,
    min_depth: u32,
    max_depth: u32,
    direction: Direction,
    min_confidence: Option<f64>,
    path_decay: f64,
    expected: BTreeMap<String, f64>,
}

/// Every edge must meet the inclusive threshold, even below `min_depth`.
/// Missing and nonnumeric confidence default to 1; depth zero has no edges.
pub async fn traversal_confidence_contract(backend: &dyn GraphBackend, prefix: &str) {
    let fixture: Fixture = serde_json::from_str(include_str!("traversal-confidence.json")).unwrap();
    let docs = format!("{prefix}_confidence_docs");
    let rels = format!("{prefix}_confidence_rels");
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
    backend
        .create_document(&docs, json!({"_key": "a"}))
        .await
        .unwrap();
    for edge in fixture.edges {
        let from = edge["from"].as_str().unwrap();
        let to = edge["to"].as_str().unwrap();
        backend
            .create_document(&docs, json!({"_key": to}))
            .await
            .unwrap();
        let mut data = json!({"_from": format!("{docs}/{from}"), "_to": format!("{docs}/{to}")});
        if let Some(confidence) = edge.get("confidence") {
            data["confidence"] = confidence.clone();
        }
        backend.create_edge(&rels, data).await.unwrap();
    }

    for case in fixture.cases {
        let paths = backend
            .traverse(
                &format!("{docs}/{}", case.start),
                &TraversalOpts {
                    min_depth: case.min_depth,
                    max_depth: case.max_depth,
                    direction: case.direction,
                    edge_collection: rels.clone(),
                    min_confidence: case.min_confidence,
                    path_decay: case.path_decay,
                },
            )
            .await
            .unwrap();
        let mut actual = BTreeMap::new();
        for path in paths {
            assert_eq!(path.depth, path.edges.len(), "{}", case.name);
            assert_eq!(path.vertices.len(), path.depth + 1, "{}", case.name);
            let key = path
                .vertices
                .iter()
                .map(|v| v["_key"].as_str().unwrap())
                .collect::<Vec<_>>()
                .join("/");
            assert!(actual.insert(key, path.score).is_none(), "{}", case.name);
        }
        assert_eq!(
            actual.keys().collect::<Vec<_>>(),
            case.expected.keys().collect::<Vec<_>>(),
            "{}",
            case.name
        );
        for (path, expected) in case.expected {
            assert!(
                (actual[&path] - expected).abs() < 1e-12,
                "{}: {path} score {} != {expected}",
                case.name,
                actual[&path]
            );
        }
    }

    backend.drop_collection(&rels).await.unwrap();
    backend.drop_collection(&docs).await.unwrap();
}
