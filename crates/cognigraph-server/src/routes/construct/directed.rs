//! Directed.

use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectedRequest {
    /// Space the facts land in. Auto-created (as an accepted, rule-less
    /// space) when absent — directed facts need a space identity for the
    /// same reconciliation and quarantine semantics as everything else.
    pub(super) space_type: String,
    pub(super) taxonomy: Vec<DirectedRelation>,
    pub(super) chunks: Vec<Chunk>,
}
/// POST /api/construct/directed — D12. Taxonomy in, evidence-bound facts
/// out, through the same gates and the same writer as rule grounding. The
/// model nominates, the gates decide; every rejection is returned verbatim.
pub(super) async fn directed(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    Json(req): Json<DirectedRequest>,
) -> Result<Json<Value>, AppError> {
    let Some(provider) = state.completion.clone() else {
        return Err(AppError(CogniGraphError::BackendError(
            "No completion provider configured. Set OPENAI_API_KEY or GEMINI_API_KEY \
             (model via COGNIGRAPH_COMPLETION_MODEL)."
                .into(),
        )));
    };
    if req.chunks.is_empty() {
        return Err(AppError(CogniGraphError::ValidationError(
            "chunks is empty".into(),
        )));
    }
    if req.chunks.len() > MAX_DIRECTED_CHUNKS {
        return Err(AppError(CogniGraphError::ValidationError(format!(
            "directed construction accepts at most {MAX_DIRECTED_CHUNKS} chunks per request \
             (one completion call); submit consecutive slices instead"
        ))));
    }
    // Resolve the incarnation only after acquiring the transition boundary.
    // Hold it across admission, space creation, completion, and reconciliation:
    // a signed deployment must never be overwritten by an in-flight completion.
    let transition = state.promotions.transition_lock.lock().await;
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    state
        .promotions
        .ensure_unmaterialized_ingest_allowed_locked(&tenant, &incarnation, &req.space_type)
        .await?;

    // Ensure the space exists so facts have an owner; a directed space is an
    // ordinary accepted space with no rules — rule ingest over it grounds
    // nothing, and directed facts attach to it like any other occurrence.
    if state
        .backend
        .get_document(SPACE_TYPES, &req.space_type)
        .await?
        .is_none()
    {
        let space = SpaceType {
            id: req.space_type.clone(),
            name: req.space_type.clone(),
            version: 1,
            description: "directed extraction space (D12)".into(),
            entities: Vec::new(),
            relation_rules: Vec::new(),
        };
        let mut doc = serde_json::to_value(&space).map_err(CogniGraphError::from)?;
        if let Some(fields) = doc.as_object_mut() {
            fields.insert("_key".into(), json!(req.space_type));
            fields.insert("accepted_by".into(), json!(actor(&user)));
            fields.insert("accepted_at".into(), json!(now_secs()));
            fields.insert(
                "drafted_by".into(),
                json!(cognigraph_construct::directed::DIRECTED_POLICY),
            );
        }
        state
            .managed_backend
            .create_document(SPACE_TYPES, doc)
            .await?;
    }
    let outcome = directed_ingest(
        &*state.managed_backend,
        &req.space_type,
        &req.taxonomy,
        &req.chunks,
        provider.as_ref(),
    )
    .await?;
    // Ledger append in the same request, under the same transition boundary.
    let rows: Vec<RefusalRow> = outcome.refusals.iter().map(RefusalRow::from).collect();
    let ledger = record_refusals(
        &*state.managed_backend,
        &RefusalContext {
            space_type: &req.space_type,
            origin: "directed",
            policy: Some(cognigraph_construct::directed::DIRECTED_POLICY),
            attribution: &outcome.extracted_by,
            actor: &actor(&user),
        },
        &rows,
    )
    .await;
    drop(transition);
    state.invalidate_search_results().await;
    let mut response = json!({
        "space_type": req.space_type,
        "chunks": req.chunks.len(),
        "proposed": outcome.proposed,
        "facts_grounded": outcome.facts_grounded,
        "skips": outcome.skips,
        "extracted_by": outcome.extracted_by,
    });
    attach(&mut response, ledger);
    Ok(Json(response))
}
