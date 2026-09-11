//! Ingest.

use super::*;

#[derive(Deserialize)]
pub(super) struct IngestRequest {
    /// Space id — must exist in the `space_types` collection.
    pub(super) space_type: String,
    /// Pre-chunked text: `{id, title?, text}` per item.
    pub(super) chunks: Vec<Chunk>,
}
pub(super) async fn ingest(
    State(state): State<AppState>,
    Json(req): Json<IngestRequest>,
) -> Result<Json<Value>, AppError> {
    if req.chunks.is_empty() {
        return Err(AppError(CogniGraphError::ValidationError(
            "chunks is empty".into(),
        )));
    }
    let space = load_space(&*state.managed_backend, &req.space_type).await?;
    let accepted = load_accepted(&*state.managed_backend, &req.space_type, "").await?;
    let config = effective_config(&space, &accepted);
    let vetoes = effective_vetoes(&accepted);
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    let facts_grounded = state
        .promotions
        .ingest_unmaterialized_chunks(
            &*state.managed_backend,
            (&tenant, &incarnation),
            &req.space_type,
            &config,
            &req.chunks,
            &vetoes,
        )
        .await?;

    state.invalidate_search_results().await;

    Ok(Json(json!({
        "space_type": req.space_type,
        "chunks": req.chunks.len(),
        "facts_grounded": facts_grounded,
        "accepted_neurons": accepted.len(),
    })))
}
