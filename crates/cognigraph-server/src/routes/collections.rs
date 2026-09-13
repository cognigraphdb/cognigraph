//! Collection catalog and explicit creation. Collections also materialize
//! implicitly on the first document write; POST makes the empty-collection
//! case reachable from the console (GET = catalog, POST = ensure).

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use cognigraph_core::{CogniGraphError, CollectionType};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{name}", axum::routing::delete(remove))
}

#[derive(Deserialize)]
struct CreateRequest {
    name: String,
    /// "document" (default) or "edge".
    #[serde(default)]
    collection_type: Option<String>,
}

/// Idempotent explicit creation (`ensure_collection`): an existing
/// collection of the same shape is a success, not a conflict. System
/// names are refused by the GuardedBackend, but we reject them here too
/// for a validation-shaped error before any backend work.
async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateRequest>,
) -> Result<Json<Value>, AppError> {
    let name = req.name.trim();
    if name.is_empty() || name.contains('/') {
        return Err(AppError(CogniGraphError::ValidationError(
            "collection names must be non-empty and must not contain `/`".into(),
        )));
    }
    if name.starts_with('_') {
        return Err(AppError(CogniGraphError::ValidationError(
            "names starting with `_` are reserved for system collections".into(),
        )));
    }
    let collection_type = match req.collection_type.as_deref() {
        None | Some("document") => CollectionType::Document,
        Some("edge") => CollectionType::Edge,
        Some(other) => {
            return Err(AppError(CogniGraphError::ValidationError(format!(
                "unknown collection type `{other}` (expected `document` or `edge`)"
            ))));
        }
    };
    state
        .backend
        .ensure_collection(name, collection_type)
        .await?;
    Ok(Json(json!({
        "name": name,
        "collection_type": collection_type.as_str(),
    })))
}

async fn list(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    // Underscore-prefixed collections are system storage (in single-store
    // mode the auth control records live in the same backend) — the catalog
    // reports application data only.
    let collections: Vec<_> = state
        .backend
        .list_collections()
        .await?
        .into_iter()
        .filter(|info| !info.name.starts_with('_'))
        .collect();
    Ok(Json(json!({
        "count": collections.len(),
        "collections": collections,
    })))
}

/// Drop a collection and everything in it. System names are refused by
/// the GuardedBackend; the semantic cache forgets the collection too.
async fn remove(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Value>, AppError> {
    let deletion = state.backend.drop_collection(&name).await;
    // Side-view cleanup can commit before collection DDL fails.
    state.invalidate_search_results().await;
    deletion?;
    Ok(Json(json!({ "dropped": name })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_core::GraphBackend;
    use cognigraph_native::NativeBackend;
    use serde_json::json;

    #[tokio::test]
    async fn create_is_explicit_validated_and_idempotent() {
        let state = AppState::new(NativeBackend::new());
        let Json(made) = create(
            State(state.clone()),
            Json(CreateRequest {
                name: "trials".into(),
                collection_type: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(made["name"], "trials");
        assert_eq!(made["collection_type"], "document");

        // Idempotent: creating again succeeds.
        assert!(
            create(
                State(state.clone()),
                Json(CreateRequest {
                    name: "trials".into(),
                    collection_type: Some("document".into()),
                }),
            )
            .await
            .is_ok()
        );

        // Edge collections are explicit too.
        let Json(edges) = create(
            State(state.clone()),
            Json(CreateRequest {
                name: "links".into(),
                collection_type: Some("edge".into()),
            }),
        )
        .await
        .unwrap();
        assert_eq!(edges["collection_type"], "edge");

        // The catalog reports both, empty.
        let Json(catalog) = list(State(state.clone())).await.unwrap();
        assert_eq!(catalog["count"], 2);
        assert_eq!(catalog["collections"][0]["count"], 0);

        // System names and malformed names are refused.
        for bad in ["_users", "", "a/b"] {
            assert!(
                create(
                    State(state.clone()),
                    Json(CreateRequest {
                        name: bad.into(),
                        collection_type: None,
                    }),
                )
                .await
                .is_err(),
                "{bad}"
            );
        }

        // Unknown type is refused.
        assert!(
            create(
                State(state),
                Json(CreateRequest {
                    name: "ok".into(),
                    collection_type: Some("graph".into()),
                }),
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn drop_removes_the_collection_and_refuses_system_names() {
        let backend = NativeBackend::new();
        backend
            .create_document("scratch", json!({"_key": "a"}))
            .await
            .unwrap();
        let state = AppState::new(backend);

        let Json(gone) = remove(State(state.clone()), Path("scratch".into()))
            .await
            .unwrap();
        assert_eq!(gone["dropped"], "scratch");
        let Json(catalog) = list(State(state.clone())).await.unwrap();
        assert_eq!(catalog["count"], 0);

        // The GuardedBackend refuses system names end to end.
        let raw = NativeBackend::new();
        raw.create_document("_users", json!({"_key": "u"}))
            .await
            .unwrap();
        let guarded_state = AppState::new_shared(std::sync::Arc::new(
            crate::system_collections::GuardedBackend::new(std::sync::Arc::new(raw)),
        ));
        assert!(
            remove(State(guarded_state), Path("_users".into()))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn catalog_reports_names_types_and_counts() {
        let backend = NativeBackend::new();
        backend
            .create_document("notes", json!({"_key": "a", "title": "Alpha"}))
            .await
            .unwrap();
        backend
            .create_document("notes", json!({"_key": "b", "title": "Beta"}))
            .await
            .unwrap();
        backend
            .upsert_edge("rels", "notes/a", "notes/b", "LINKS", json!({}))
            .await
            .unwrap();

        // System collections (underscore prefix) stay out of the catalog.
        backend
            .create_document("_users", json!({"_key": "u1"}))
            .await
            .unwrap();

        let Json(catalog) = list(State(AppState::new(backend))).await.unwrap();
        assert_eq!(catalog["count"], 2);
        let entries = catalog["collections"].as_array().unwrap();
        // Sorted by name: notes before rels.
        assert_eq!(entries[0]["name"], "notes");
        assert_eq!(entries[0]["collection_type"], "document");
        assert_eq!(entries[0]["count"], 2);
        assert_eq!(entries[1]["name"], "rels");
        assert_eq!(entries[1]["collection_type"], "edge");
        assert_eq!(entries[1]["count"], 1);
    }
}
