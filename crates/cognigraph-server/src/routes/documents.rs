use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use serde::Deserialize;

use crate::error::AppError;
use crate::state::AppState;
#[cfg(all(test, feature = "enterprise"))]
use crate::system_collections::SIDE_VIEWS_COLLECTION;

/// Search rows may embed source documents while being keyed by an embeddings
/// collection, so every successful data mutation uses the dependency-safe
/// result-only barrier (query embeddings remain cached).
async fn invalidate_cache(state: &AppState, _collection: &str) {
    state.invalidate_search_results().await;
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_document))
        .route("/", get(list_documents))
        .route("/embed", post(embed_documents))
        .route("/{collection}/{key}", get(get_document))
        .route("/{collection}/{key}", patch(update_document))
        .route("/{collection}/{key}", put(replace_document))
        .route("/{collection}/{key}", delete(delete_document))
}

#[derive(Deserialize)]
struct CreateRequest {
    collection: String,
    #[serde(flatten)]
    document: serde_json::Value,
}

async fn create_document(
    State(state): State<AppState>,
    Json(req): Json<CreateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let id = state
        .backend
        .create_document(&req.collection, req.document)
        .await?;

    invalidate_cache(&state, &id.collection).await;

    Ok(Json(serde_json::json!({
        "_id": id.full_id(),
        "_key": id.key,
        "collection": id.collection,
    })))
}

#[derive(Deserialize)]
struct ListQuery {
    collection: String,
    limit: Option<usize>,
    offset: Option<usize>,
}

async fn list_documents(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let docs = state
        .backend
        .list_documents(&q.collection, q.limit, q.offset)
        .await?;

    Ok(Json(serde_json::json!({
        "results": docs,
        "count": docs.len(),
    })))
}

#[derive(Deserialize)]
struct DocPath {
    collection: String,
    key: String,
}

async fn get_document(
    State(state): State<AppState>,
    Path(path): Path<DocPath>,
) -> Result<Json<serde_json::Value>, AppError> {
    let doc = state
        .backend
        .get_document(&path.collection, &path.key)
        .await?;

    match doc {
        Some(d) => Ok(Json(d)),
        None => Err(AppError(
            cognigraph_core::CogniGraphError::DocumentNotFound {
                collection: path.collection,
                key: path.key,
            },
        )),
    }
}

async fn update_document(
    State(state): State<AppState>,
    Path(path): Path<DocPath>,
    Json(update): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let doc = state
        .backend
        .update_document(&path.collection, &path.key, update)
        .await?;
    invalidate_cache(&state, &path.collection).await;
    Ok(Json(doc))
}

async fn replace_document(
    State(state): State<AppState>,
    Path(path): Path<DocPath>,
    Json(doc): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let doc = state
        .backend
        .replace_document(&path.collection, &path.key, doc)
        .await?;
    invalidate_cache(&state, &path.collection).await;
    Ok(Json(doc))
}

async fn delete_document(
    State(state): State<AppState>,
    Path(path): Path<DocPath>,
) -> Result<Json<serde_json::Value>, AppError> {
    #[cfg(not(feature = "enterprise"))]
    let deletion = state
        .backend
        .delete_document(&path.collection, &path.key)
        .await
        .map(|deleted| (deleted, 0));
    #[cfg(feature = "enterprise")]
    let deletion = state
        .side_views
        .delete_document(state.managed_backend.as_ref(), &path.collection, &path.key)
        .await;
    // A remote cleanup can make partial progress before returning an error.
    invalidate_cache(&state, &path.collection).await;
    let (deleted, side_views_deleted) = deletion?;

    Ok(Json(serde_json::json!({
        "deleted": deleted,
        "_id": format!("{}/{}", path.collection, path.key),
        "side_views_deleted": side_views_deleted,
    })))
}

// ---------------------------------------------------------------------------
// H8: batch embedding pipeline — embed each item's text server-side in
// provider batches (bounded concurrency), then store everything in ONE
// atomic backend batch. All-or-nothing: validation and every embed batch
// must succeed before anything is written.
// ---------------------------------------------------------------------------

const EMBED_CONCURRENCY: usize = 4;

fn default_text_field() -> String {
    "text".into()
}
fn default_embedding_field() -> String {
    "embedding".into()
}
fn default_batch_size() -> usize {
    64
}

#[derive(Deserialize)]
struct EmbedBatchRequest {
    collection: String,
    /// Pre-chunked documents; each must carry a non-empty string at
    /// `text_field`. Chunking itself stays upstream (cognigraph-chunker).
    items: Vec<serde_json::Value>,
    #[serde(default = "default_text_field")]
    text_field: String,
    #[serde(default = "default_embedding_field")]
    embedding_field: String,
    /// Texts per provider request (one API call each).
    #[serde(default = "default_batch_size")]
    batch_size: usize,
    /// When true, an item whose `_key` already exists MERGES into that
    /// document (embedding + the item's fields) instead of answering 409 —
    /// the re-embed path for existing documents (e.g. the console's
    /// inspector). New keys still insert.
    #[serde(default)]
    upsert: bool,
}

async fn embed_documents(
    State(state): State<AppState>,
    Json(req): Json<EmbedBatchRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use cognigraph_core::CogniGraphError;

    // Fail before invoking a potentially billable provider. The guarded batch
    // remains the authoritative write barrier; this is its embedding-specific
    // preflight so managed/system collection requests do no external work.
    crate::system_collections::deny_public_collection_mutation(&req.collection)?;

    let embedder = state.embedder.clone().ok_or_else(|| {
        AppError(CogniGraphError::BackendError(
            "No embedding provider configured. Set COGNIGRAPH_EMBEDDING_PROVIDER env var.".into(),
        ))
    })?;
    if req.items.is_empty() {
        return Err(AppError(CogniGraphError::ValidationError(
            "items is empty".into(),
        )));
    }
    let batch_size = req.batch_size.clamp(1, 2048);

    // Validate everything BEFORE the first provider call.
    let mut texts: Vec<String> = Vec::with_capacity(req.items.len());
    for (index, item) in req.items.iter().enumerate() {
        if !item.is_object() {
            return Err(AppError(CogniGraphError::ValidationError(format!(
                "items[{index}] is not an object"
            ))));
        }
        match item.get(&req.text_field).and_then(|v| v.as_str()) {
            Some(text) if !text.trim().is_empty() => texts.push(text.to_string()),
            _ => {
                return Err(AppError(CogniGraphError::ValidationError(format!(
                    "items[{index}] has no non-empty string at `{}`",
                    req.text_field
                ))));
            }
        }
    }

    // Embed in provider batches, up to EMBED_CONCURRENCY requests in
    // flight; order restored by batch index.
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(EMBED_CONCURRENCY));
    let mut handles = Vec::new();
    for (index, chunk) in texts.chunks(batch_size).enumerate() {
        let chunk: Vec<String> = chunk.to_vec();
        let embedder = embedder.clone();
        let semaphore = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = semaphore.acquire().await;
            let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
            embedder.embed(&refs).await.map(|vectors| (index, vectors))
        }));
    }
    let batches = handles.len();
    let mut embedded: Vec<(usize, Vec<Vec<f64>>)> = Vec::with_capacity(batches);
    for handle in handles {
        let result = handle
            .await
            .map_err(|e| AppError(CogniGraphError::EmbeddingError(e.to_string())))?
            .map_err(|e| AppError(CogniGraphError::EmbeddingError(e.to_string())))?;
        embedded.push(result);
    }
    embedded.sort_by_key(|(index, _)| *index);
    let vectors: Vec<Vec<f64>> = embedded.into_iter().flat_map(|(_, v)| v).collect();
    if vectors.len() != req.items.len() {
        return Err(AppError(CogniGraphError::EmbeddingError(format!(
            "provider returned {} vectors for {} items",
            vectors.len(),
            req.items.len()
        ))));
    }

    // Store: one atomic transaction for the whole batch (H4: this is the
    // fast persistent-write path — one commit, not one per document).
    let mut ops: Vec<cognigraph_core::BatchOp> = Vec::with_capacity(req.items.len());
    for (mut item, vector) in req.items.into_iter().zip(vectors) {
        item[req.embedding_field.as_str()] = serde_json::json!(vector);
        let existing_key = if req.upsert {
            match item.get("_key").and_then(|v| v.as_str()) {
                Some(key)
                    if state
                        .backend
                        .get_document(&req.collection, key)
                        .await?
                        .is_some() =>
                {
                    Some(key.to_string())
                }
                _ => None,
            }
        } else {
            None
        };
        ops.push(match existing_key {
            Some(key) => cognigraph_core::BatchOp::Update {
                collection: req.collection.clone(),
                key,
                merge: item,
            },
            None => cognigraph_core::BatchOp::Insert {
                collection: req.collection.clone(),
                doc: item,
            },
        });
    }
    let stored = state.backend.execute_batch(ops).await?;
    invalidate_cache(&state, &req.collection).await;

    let keys: Vec<serde_json::Value> = stored
        .iter()
        .filter_map(|doc| doc.get("_key").cloned())
        .collect();
    Ok(Json(serde_json::json!({
        "stored": keys.len(),
        "keys": keys,
        "model": embedder.model_name(),
        "batches": batches,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognigraph_core::CogniGraphError;
    use cognigraph_native::NativeBackend;
    use serde_json::json;

    /// Deterministic embedder: [len, 1.0, 0.0] per text.
    struct MockEmbedder;

    #[async_trait::async_trait]
    impl cognigraph_embeddings::EmbeddingProvider for MockEmbedder {
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>> {
            Ok(texts
                .iter()
                .map(|t| vec![t.len() as f64, 1.0, 0.0])
                .collect())
        }
        fn model_name(&self) -> &str {
            "mock"
        }
    }

    fn state() -> AppState {
        AppState::new(NativeBackend::new()).with_embedder(MockEmbedder)
    }

    fn request(items: Vec<serde_json::Value>, batch_size: usize) -> EmbedBatchRequest {
        EmbedBatchRequest {
            collection: "documents".into(),
            items,
            text_field: "text".into(),
            embedding_field: "embedding".into(),
            batch_size,
            upsert: false,
        }
    }

    /// Upsert mode re-embeds an existing document by merging the item's
    /// fields plus the vector into it — no 409, no clobbered fields.
    #[tokio::test]
    async fn upsert_merges_into_the_existing_document() {
        let state = state();
        state
            .backend
            .ensure_collection("documents", cognigraph_core::CollectionType::Document)
            .await
            .unwrap();
        state
            .backend
            .create_document(
                "documents",
                json!({"_key": "doc1", "title": "Original title", "kept": true}),
            )
            .await
            .unwrap();

        let Json(response) = embed_documents(
            State(state.clone()),
            Json(EmbedBatchRequest {
                upsert: true,
                ..request(vec![json!({"_key": "doc1", "text": "alpha"})], 64)
            }),
        )
        .await
        .unwrap();
        assert_eq!(response["stored"], 1);

        let doc = state
            .backend
            .get_document("documents", "doc1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(doc["embedding"], json!([5.0, 1.0, 0.0]));
        assert_eq!(doc["title"], "Original title", "{doc}");
        assert_eq!(doc["kept"], true, "{doc}");
        assert_eq!(doc["text"], "alpha");

        // Without upsert the same request still answers the conflict.
        let err = embed_documents(
            State(state),
            Json(request(vec![json!({"_key": "doc1", "text": "alpha"})], 64)),
        )
        .await
        .unwrap_err();
        assert!(matches!(err.0, CogniGraphError::DocumentConflict(_)));
    }

    #[tokio::test]
    async fn pipeline_embeds_and_stores_atomically() {
        let state = state();
        let items = vec![
            json!({"_key": "c1", "text": "alpha"}),
            json!({"_key": "c2", "text": "beta beta"}),
            json!({"_key": "c3", "text": "y"}),
        ];
        let Json(response) = embed_documents(State(state.clone()), Json(request(items, 2)))
            .await
            .unwrap();
        assert_eq!(response["stored"], 3);
        assert_eq!(response["model"], "mock");
        assert_eq!(response["batches"], 2); // 3 items, batch_size 2

        // Embeddings landed per item, order preserved across batches.
        let doc = state
            .backend
            .get_document("documents", "c2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(doc["embedding"], json!([9.0, 1.0, 0.0]));
        let doc = state
            .backend
            .get_document("documents", "c3")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(doc["embedding"], json!([1.0, 1.0, 0.0]));
    }

    #[tokio::test]
    async fn embedding_cannot_write_managed_chunks() {
        let state = state();
        let mut managed = request(vec![json!({"_key": "c1", "text": "alpha"})], 64);
        managed.collection = "chunks".into();

        let err = embed_documents(State(state.clone()), Json(managed))
            .await
            .unwrap_err();
        assert!(matches!(err.0, CogniGraphError::Forbidden(_)), "{err:?}");
        assert!(
            state
                .backend
                .get_document("chunks", "c1")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn missing_text_field_rejected_before_any_write() {
        let state = state();
        let items = vec![
            json!({"_key": "c1", "text": "fine"}),
            json!({"_key": "c2", "body": "wrong field"}),
        ];
        let err = embed_documents(State(state.clone()), Json(request(items, 64)))
            .await
            .unwrap_err();
        assert!(matches!(err.0, CogniGraphError::ValidationError(_)));
        assert!(
            state
                .backend
                .get_document("documents", "c1")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn conflict_stores_nothing() {
        let state = state();
        let items = vec![
            json!({"_key": "dup", "text": "one"}),
            json!({"_key": "dup", "text": "two"}),
        ];
        let err = embed_documents(State(state.clone()), Json(request(items, 64)))
            .await
            .unwrap_err();
        assert!(matches!(err.0, CogniGraphError::DocumentConflict(_)));
        assert!(
            state
                .backend
                .get_document("documents", "dup")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn requires_an_embedder() {
        let state = AppState::new(NativeBackend::new());
        let items = vec![json!({"text": "x"})];
        assert!(
            embed_documents(State(state), Json(request(items, 64)))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn source_document_delete_flushes_embedding_keyed_results_only() {
        use cognigraph_cache::{CacheConfig, CacheKey, InMemoryCache, SearchMode};
        use cognigraph_core::SearchHit;

        let state =
            AppState::new(NativeBackend::new()).with_cache(InMemoryCache::new(CacheConfig {
                enabled: true,
                ..Default::default()
            }));
        state
            .backend
            .create_document("documents", json!({"_key": "secret", "body": "old"}))
            .await
            .unwrap();
        let key = CacheKey {
            collection: "embeddings".into(),
            search_mode: SearchMode::Semantic,
            normalized_query: "find secret".into(),
            params: "t=0;l=10;m=".into(),
        };
        let cache = state.cache.as_deref().unwrap();
        cache
            .put_embedding("find secret", "mock", vec![1.0, 0.0])
            .await;
        cache
            .put_results(
                &key,
                Some(vec![1.0, 0.0]),
                vec![SearchHit {
                    document: json!({
                        "document_id": "documents/secret",
                        "document": {"_id": "documents/secret", "body": "old"}
                    }),
                    score: 1.0,
                    source: Some("semantic".into()),
                }],
                None,
            )
            .await;

        let _ = delete_document(
            State(state.clone()),
            Path(DocPath {
                collection: "documents".into(),
                key: "secret".into(),
            }),
        )
        .await
        .unwrap();

        assert!(cache.get_results(&key, None).await.is_none());
        assert_eq!(
            cache.get_embedding("find secret", "mock").await,
            Some(vec![1.0, 0.0]),
            "the result-only barrier must retain deterministic embeddings"
        );
    }

    #[tokio::test]
    async fn managed_documents_are_readable_but_generic_mutations_are_forbidden() {
        let state = state();
        state
            .managed_backend
            .create_document(
                "neurons",
                json!({"_key": "n1", "status": "proposed", "evidence": ["fixture"]}),
            )
            .await
            .unwrap();

        let Json(found) = get_document(
            State(state.clone()),
            Path(DocPath {
                collection: "neurons".into(),
                key: "n1".into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(found["status"], json!("proposed"));

        let create_error = create_document(
            State(state.clone()),
            Json(CreateRequest {
                collection: "neurons".into(),
                document: json!({"_key": "bypass", "status": "accepted"}),
            }),
        )
        .await
        .unwrap_err();
        assert!(
            matches!(create_error.0, CogniGraphError::Forbidden(_)),
            "{create_error:?}"
        );

        for error in [
            update_document(
                State(state.clone()),
                Path(DocPath {
                    collection: "neurons".into(),
                    key: "n1".into(),
                }),
                Json(json!({"status": "accepted"})),
            )
            .await
            .unwrap_err(),
            replace_document(
                State(state.clone()),
                Path(DocPath {
                    collection: "neurons".into(),
                    key: "n1".into(),
                }),
                Json(json!({"status": "accepted"})),
            )
            .await
            .unwrap_err(),
            delete_document(
                State(state.clone()),
                Path(DocPath {
                    collection: "neurons".into(),
                    key: "n1".into(),
                }),
            )
            .await
            .unwrap_err(),
        ] {
            assert!(
                matches!(error.0, CogniGraphError::Forbidden(_)),
                "{error:?}"
            );
        }

        assert_eq!(
            state
                .backend
                .get_document("neurons", "n1")
                .await
                .unwrap()
                .unwrap()["status"],
            json!("proposed")
        );
    }

    /// Single-store mode keeps `_users`/`_tokens` in the data backend —
    /// every documents endpoint must refuse system collections
    /// (decision_system_collections.md).
    #[tokio::test]
    async fn system_collections_answer_forbidden() {
        let state = state();
        fn forbidden(err: crate::error::AppError) {
            assert!(matches!(err.0, CogniGraphError::Forbidden(_)), "{err:?}");
        }
        let path = || DocPath {
            collection: "_users".into(),
            key: "admin".into(),
        };

        forbidden(
            create_document(
                State(state.clone()),
                Json(CreateRequest {
                    collection: "_users".into(),
                    document: json!({"role": "Admin"}),
                }),
            )
            .await
            .unwrap_err(),
        );
        forbidden(
            list_documents(
                State(state.clone()),
                axum::extract::Query(ListQuery {
                    collection: "_users".into(),
                    limit: None,
                    offset: None,
                }),
            )
            .await
            .unwrap_err(),
        );
        forbidden(
            get_document(State(state.clone()), Path(path()))
                .await
                .unwrap_err(),
        );
        forbidden(
            update_document(
                State(state.clone()),
                Path(path()),
                Json(json!({"role": "Admin"})),
            )
            .await
            .unwrap_err(),
        );
        forbidden(
            replace_document(State(state.clone()), Path(path()), Json(json!({})))
                .await
                .unwrap_err(),
        );
        forbidden(
            delete_document(State(state.clone()), Path(path()))
                .await
                .unwrap_err(),
        );
        forbidden(
            embed_documents(
                State(state),
                Json(EmbedBatchRequest {
                    collection: "_users".into(),
                    items: vec![json!({"text": "x"})],
                    text_field: "text".into(),
                    embedding_field: "embedding".into(),
                    batch_size: 64,
                    upsert: false,
                }),
            )
            .await
            .unwrap_err(),
        );
    }

    #[cfg(feature = "enterprise")]
    #[tokio::test]
    async fn deleting_a_parent_cascades_to_its_side_views() {
        let state = state();
        state
            .backend
            .create_document("documents", json!({"_key": "p1", "text": "hello"}))
            .await
            .unwrap();
        // Two side-views for p1 plus one for an unrelated parent, seeded through
        // the raw handle exactly as the generation job writes them (side_views
        // is write-protected on the public backend).
        for (key, parent) in [
            ("sv1", "documents/p1"),
            ("sv2", "documents/p1"),
            ("sv-other", "documents/p2"),
        ] {
            state
                .managed_backend
                .create_document(
                    SIDE_VIEWS_COLLECTION,
                    json!({
                        "_key": key,
                        "document_id": parent,
                        "kind": "side_view",
                        "question": "q",
                        "answer": "a",
                        "embedding": [0.1, 0.2],
                    }),
                )
                .await
                .unwrap();
        }

        let response = delete_document(
            State(state.clone()),
            Path(DocPath {
                collection: "documents".into(),
                key: "p1".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(response["deleted"], json!(true));
        assert_eq!(
            response["side_views_deleted"],
            json!(2),
            "both of p1's side-views are removed: {response}"
        );

        // p1's side-views are gone; the unrelated parent's side-view survives.
        let remaining = state
            .managed_backend
            .list_documents(SIDE_VIEWS_COLLECTION, None, None)
            .await
            .unwrap();
        let keys: Vec<&str> = remaining
            .iter()
            .filter_map(|d| d.get("_key").and_then(|v| v.as_str()))
            .collect();
        assert_eq!(
            keys,
            vec!["sv-other"],
            "only the unrelated side-view remains"
        );
    }

    #[tokio::test]
    async fn deleting_a_document_without_side_views_is_a_clean_noop() {
        // No side_views collection has ever been created; the explicit absence
        // check must allow an ordinary document deletion.
        let state = state();
        state
            .backend
            .create_document("documents", json!({"_key": "p1", "text": "hi"}))
            .await
            .unwrap();
        let response = delete_document(
            State(state.clone()),
            Path(DocPath {
                collection: "documents".into(),
                key: "p1".into(),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(response["deleted"], json!(true));
        assert_eq!(response["side_views_deleted"], json!(0));
    }
}
