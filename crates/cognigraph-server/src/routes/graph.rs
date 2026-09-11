use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use cognigraph_core::{Direction, TraversalOpts};

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/relationships", post(create_relationship))
        .route("/relationships", get(get_relationships))
        .route("/traverse", post(traverse))
}

#[derive(Deserialize)]
struct CreateRelationshipRequest {
    /// Edge collection name (defaults to "document_relations")
    #[serde(default = "default_edge_collection")]
    collection: String,
    from: String,
    to: String,
    relation_type: String,
    #[serde(default = "default_confidence")]
    confidence: f64,
    #[serde(default)]
    metadata: serde_json::Value,
}

fn default_edge_collection() -> String {
    "document_relations".into()
}
fn default_confidence() -> f64 {
    1.0
}

async fn create_relationship(
    State(state): State<AppState>,
    Json(req): Json<CreateRelationshipRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut data = if req.metadata.is_object() {
        req.metadata
    } else {
        serde_json::json!({})
    };
    data["confidence"] = serde_json::json!(req.confidence);

    let edge = state
        .backend
        .upsert_edge(
            &req.collection,
            &req.from,
            &req.to,
            &req.relation_type,
            data,
        )
        .await?;

    // Graph-augmented entries are keyed by their embeddings collection, not
    // by this edge collection or either endpoint collection.
    state.invalidate_search_results().await;

    Ok(Json(edge))
}

#[derive(Deserialize)]
struct RelationshipsQuery {
    /// Edge collection name
    #[serde(default = "default_edge_collection")]
    collection: String,
    /// Vertex ID to get relationships for
    vertex_id: String,
    /// Direction: outbound, inbound, any
    #[serde(default)]
    direction: Direction,
}

async fn get_relationships(
    State(state): State<AppState>,
    Query(q): Query<RelationshipsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let edges = state
        .backend
        .get_edges(&q.collection, &q.vertex_id, q.direction)
        .await?;

    Ok(Json(serde_json::json!({
        "results": edges,
        "count": edges.len(),
    })))
}

#[derive(Deserialize)]
struct TraverseRequest {
    start_vertex: String,
    #[serde(default = "default_edge_collection")]
    edge_collection: String,
    #[serde(default = "default_max_depth")]
    max_depth: u32,
    #[serde(default = "default_min_depth")]
    min_depth: u32,
    #[serde(default)]
    direction: Direction,
    #[serde(default)]
    min_confidence: Option<f64>,
    #[serde(default = "default_path_decay")]
    path_decay: f64,
}

fn default_max_depth() -> u32 {
    3
}
fn default_min_depth() -> u32 {
    1
}
fn default_path_decay() -> f64 {
    0.8
}

async fn traverse(
    State(state): State<AppState>,
    Json(req): Json<TraverseRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let opts = TraversalOpts {
        max_depth: req.max_depth,
        min_depth: req.min_depth,
        direction: req.direction,
        edge_collection: req.edge_collection,
        min_confidence: req.min_confidence,
        path_decay: req.path_decay,
    };

    let paths = state.backend.traverse(&req.start_vertex, &opts).await?;

    Ok(Json(serde_json::json!({
        "results": paths,
        "count": paths.len(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppError;
    use cognigraph_core::CogniGraphError;
    use cognigraph_native::NativeBackend;

    fn forbidden(err: AppError) {
        assert!(matches!(err.0, CogniGraphError::Forbidden(_)), "{err:?}");
    }

    fn relationship(collection: &str, from: &str) -> CreateRelationshipRequest {
        CreateRelationshipRequest {
            collection: collection.into(),
            from: from.into(),
            to: "notes/b".into(),
            relation_type: "LINKS".into(),
            confidence: 1.0,
            metadata: serde_json::json!({}),
        }
    }

    fn traversal(edge_collection: &str, start: &str) -> TraverseRequest {
        TraverseRequest {
            start_vertex: start.into(),
            edge_collection: edge_collection.into(),
            max_depth: 2,
            min_depth: 1,
            direction: Direction::default(),
            min_confidence: None,
            path_decay: 0.8,
        }
    }

    /// System collections must be untouchable both as edge collections and
    /// as edge/traversal endpoints (`_users/admin`) — an edge into `_users`
    /// would let a later traversal fetch the credential document
    /// (decision_system_collections.md).
    #[tokio::test]
    async fn system_collections_answer_forbidden() {
        let state = AppState::new(NativeBackend::new());

        forbidden(
            create_relationship(State(state.clone()), Json(relationship("_rels", "notes/a")))
                .await
                .unwrap_err(),
        );
        forbidden(
            create_relationship(
                State(state.clone()),
                Json(relationship("rels", "_users/admin")),
            )
            .await
            .unwrap_err(),
        );
        forbidden(
            get_relationships(
                State(state.clone()),
                Query(RelationshipsQuery {
                    collection: "_tokens".into(),
                    vertex_id: "notes/a".into(),
                    direction: Direction::default(),
                }),
            )
            .await
            .unwrap_err(),
        );
        forbidden(
            get_relationships(
                State(state.clone()),
                Query(RelationshipsQuery {
                    collection: "rels".into(),
                    vertex_id: "_users/admin".into(),
                    direction: Direction::default(),
                }),
            )
            .await
            .unwrap_err(),
        );
        forbidden(
            traverse(State(state.clone()), Json(traversal("_tokens", "notes/a")))
                .await
                .unwrap_err(),
        );
        forbidden(
            traverse(State(state), Json(traversal("rels", "_users/admin")))
                .await
                .unwrap_err(),
        );
    }
}
