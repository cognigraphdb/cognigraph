//! Collection catalog and explicit creation. Collections also materialize
//! implicitly on the first document write; POST makes the empty-collection
//! case reachable from the console (GET = catalog, POST = ensure).

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use cognigraph_core::{CogniGraphError, CollectionType, IndexDef, IndexType};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::AppError;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/{name}", axum::routing::delete(remove))
        .route("/{name}/indexes", get(list_indexes).post(ensure_index))
        .route("/{name}/indexes/{index}", axum::routing::delete(drop_index))
}

/// A unique constraint declaration (CG-86). `unique` defaults to true on
/// this route: it exists to declare constraints; non-unique declarations
/// are recorded but not used for acceleration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IndexRequest {
    pub(super) fields: Vec<String>,
    #[serde(default = "default_true")]
    pub(super) unique: bool,
    #[serde(default)]
    pub(super) sparse: bool,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(default = "default_index_type")]
    pub(super) index_type: IndexType,
}

fn default_true() -> bool {
    true
}

fn default_index_type() -> IndexType {
    IndexType::Persistent
}

fn validated_name(name: &str) -> Result<(), AppError> {
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
    Ok(())
}

/// GET /{name}/indexes — declared indexes; unknown collections list nothing.
pub(super) async fn list_indexes(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<Value>, AppError> {
    validated_name(&name)?;
    let indexes = state.backend.list_indexes(&name).await?;
    Ok(Json(json!({
        "collection": name,
        "count": indexes.len(),
        "indexes": indexes,
    })))
}

/// POST /{name}/indexes — idempotent ensure. The backend refuses edge
/// collections, unsupported types, malformed definitions, a different
/// definition under an existing name, and existing duplicates (409).
pub(super) async fn ensure_index(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<IndexRequest>,
) -> Result<Json<Value>, AppError> {
    validated_name(&name)?;
    if req.fields.is_empty() || req.fields.iter().any(|f| f.trim().is_empty()) {
        return Err(AppError(CogniGraphError::ValidationError(
            "an index needs at least one non-empty field".into(),
        )));
    }
    if !matches!(req.index_type, IndexType::Persistent | IndexType::Hash) {
        return Err(AppError(CogniGraphError::ValidationError(
            "only persistent and hash indexes are supported".into(),
        )));
    }
    if let Some(index_name) = req.name.as_deref()
        && (index_name.is_empty() || index_name.contains('/') || index_name.contains('\u{0}'))
    {
        return Err(AppError(CogniGraphError::ValidationError(
            "index names must be non-empty and must not contain `/` or NUL".into(),
        )));
    }
    let def = IndexDef {
        index_type: req.index_type,
        fields: req.fields,
        unique: req.unique,
        sparse: req.sparse,
        name: req.name,
    };
    state.backend.ensure_index(&name, &def).await?;
    let stored = state
        .backend
        .list_indexes(&name)
        .await?
        .into_iter()
        .find(|d| d.fields == def.fields && d.unique == def.unique && d.sparse == def.sparse)
        .unwrap_or(def);
    Ok(Json(json!({ "collection": name, "index": stored })))
}

/// DELETE /{name}/indexes/{index} — `dropped: false` when absent.
pub(super) async fn drop_index(
    State(state): State<AppState>,
    Path((name, index)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    validated_name(&name)?;
    let dropped = state.backend.drop_index(&name, &index).await?;
    Ok(Json(
        json!({ "collection": name, "index": index, "dropped": dropped }),
    ))
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

#[cfg(test)]
#[path = "collections_index_tests.rs"]
mod index_tests;
