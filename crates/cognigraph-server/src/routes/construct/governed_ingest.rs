//! Governed ingest.

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GovernedIngestRequest {
    pub(super) target: PromotionTarget,
    /// One bounded atomic reconciliation. Larger corpora are intentionally
    /// submitted in multiple calls so governance transitions are not held by
    /// an unbounded request.
    pub(super) chunks: Vec<Chunk>,
}
pub(super) async fn governed_ingest(
    State(state): State<AppState>,
    Json(req): Json<GovernedIngestRequest>,
) -> Result<Json<Value>, AppError> {
    req.target.validate()?;
    if req.chunks.is_empty() {
        return Err(AppError(CogniGraphError::ValidationError(
            "chunks is empty".into(),
        )));
    }
    if req.chunks.len() > MAX_GOVERNED_INGEST_CHUNKS {
        return Err(AppError(CogniGraphError::CapacityExceeded(format!(
            "governed construction accepts at most {MAX_GOVERNED_INGEST_CHUNKS} chunks per atomic request"
        ))));
    }

    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    state.promotions.ensure_repository(&tenant).await?;
    state
        .promotions
        .ensure_semantic_repair_repository(&tenant)
        .await?;

    let transition = state.promotions.transition_lock.lock().await;
    state
        .promotions
        .ensure_unmaterialized_ingest_allowed_locked(&tenant, &incarnation, &req.target.space_type)
        .await?;
    let authority = state
        .promotions
        .resolve_current_semantic_repair_authority_locked(&tenant, &incarnation, &req.target)
        .await?;
    let head = state
        .promotions
        .current_raw(&tenant, &incarnation, &req.target)
        .await?
        .ok_or_else(|| {
            AppError(CogniGraphError::DocumentConflict(
                "governed construction lost its promotion head".into(),
            ))
        })?;
    let (space, accepted) = authority
        .revision
        .candidate
        .validate_semantic_repair_candidate(&req.target)?;
    let config = effective_config(&space, &accepted);
    let vetoes = effective_vetoes(&accepted);
    crate::artifact_consumption::validate_semantic_repair_grounding_work(
        &config,
        &vetoes,
        &req.chunks,
    )?;
    let facts_grounded = ingest_chunks(
        &*state.managed_backend,
        &req.target.space_type,
        &config,
        &req.chunks,
        &vetoes,
    )
    .await?;
    drop(transition);

    state.invalidate_search_results().await;
    Ok(Json(json!({
        "target": req.target,
        "chunks": req.chunks.len(),
        "facts_grounded": facts_grounded,
        "accepted_neurons": accepted.len(),
        "authority": {
            "promotion_head_decision_id": head.applied_decision_id,
            "promotion_generation": head.selection.generation,
            "candidate_digest": authority.revision.candidate_digest,
            "semantic_repair_revision_id": authority.revision.semantic_repair_revision_id,
            "semantic_repair_revision_digest": authority.revision.semantic_repair_revision_digest,
            "semantic_repair_review_id": authority.review.semantic_repair_review_id,
            "semantic_repair_review_digest": authority.review.semantic_repair_review_digest,
        }
    })))
}
