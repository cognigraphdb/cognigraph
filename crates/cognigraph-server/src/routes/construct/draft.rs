//! Draft.

use super::*;

/// The ontology drafter (decision_ontology_drafter.md): an LLM
/// proposes a DRAFT space type from sample chunks — entities and naive
/// rules only, every surface and trigger symbolically checked, gate
/// advisor annotations attached. The draft lands in
/// `space_type_drafts`, a collection no grounding path reads:
/// structurally inert until a human accepts it (D1). The drafter
/// refuses ids that already exist as accepted spaces (D3) and never
/// authors gates or templates (D4).
///
/// `async: true` moves the whole loop into a durable `construct.draft` job —
/// the only viable path for a real corpus, since drafting spends two LLM
/// completions per document and a synchronous request runs out of time.
pub(super) async fn draft(
    State(state): State<AppState>,
    user: Option<Extension<User>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<DraftRequest>,
) -> Result<axum::response::Response, AppError> {
    if req.run_async {
        return enqueue_draft(state, user, &headers, req).await;
    }
    Ok(axum::response::IntoResponse::into_response(
        draft_sync(state, user, req).await?,
    ))
}
/// Hand the corpus to the durable job framework: capture the immutable input
/// snapshot and enqueue, exactly like every other job-submitting route. All
/// validation (D3 refusal, empty corpus, provider availability, document cap)
/// happens when the payload is prepared, so an ineligible request never becomes
/// a queued job.
pub(super) async fn enqueue_draft(
    state: AppState,
    user: Option<Extension<User>>,
    headers: &axum::http::HeaderMap,
    req: DraftRequest,
) -> Result<axum::response::Response, AppError> {
    let key = headers
        .get("Idempotency-Key")
        .ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(
                "missing Idempotency-Key header (required for async drafting)".into(),
            ))
        })?
        .to_str()
        .map_err(|_| {
            AppError(CogniGraphError::ValidationError(
                "Idempotency-Key must be printable ASCII".into(),
            ))
        })?;
    let mut input = json!({
        "space_type": req.space_type,
        "chunks": req.chunks,
    });
    if let Some(sample_cap) = req.sample_cap {
        input["sample_cap"] = json!(sample_cap);
    }
    let tenant = current_tenant();
    let incarnation = JobManager::tenant_incarnation(&state, &tenant).await?;
    let submission = state
        .jobs
        .submit(
            state.clone(),
            tenant,
            incarnation,
            crate::jobs::JobActor::request(user.as_ref().map(|Extension(user)| user)),
            key,
            crate::jobs::JobKind::ConstructDraft,
            input,
        )
        .await?;
    let status = if submission.replayed {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::ACCEPTED
    };
    let mut response = axum::response::IntoResponse::into_response((
        status,
        Json(json!({
            "job": submission.job.public_value(),
            "replayed": submission.replayed,
        })),
    ));
    response.headers_mut().insert(
        axum::http::header::LOCATION,
        format!("/api/jobs/{}", submission.job.id)
            .parse()
            .expect("job location is valid"),
    );
    Ok(response)
}
pub(super) async fn draft_sync(
    state: AppState,
    user: Option<Extension<User>>,
    req: DraftRequest,
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
    if state
        .backend
        .get_document(SPACE_TYPES, &req.space_type)
        .await?
        .is_some()
    {
        return Err(AppError(CogniGraphError::ValidationError(format!(
            "space `{}` already exists in `{SPACE_TYPES}` — the drafter creates NEW spaces \
             only (D3); extend accepted ontologies via versioned edits and neurons",
            req.space_type
        ))));
    }
    let documents = req.per_document.then(|| group_chunks_by_title(&req.chunks));
    let report = match &documents {
        Some(docs) => {
            cognigraph_construct::draft_space_type_per_document(
                &*provider,
                &req.space_type,
                docs,
                req.sample_cap.unwrap_or(40),
            )
            .await
        }
        None => {
            cognigraph_construct::draft_space_type(
                &*provider,
                &req.space_type,
                &req.chunks,
                req.sample_cap.unwrap_or(40),
            )
            .await
        }
    }
    .map_err(|e| AppError(CogniGraphError::BackendError(format!("drafting: {e}"))))?;

    let drafted_by = format!(
        "draft:{}@{}",
        provider.model_name(),
        cognigraph_construct::DRAFT_REV
    );
    let mut doc = serde_json::to_value(&report.space).map_err(CogniGraphError::from)?;
    if let Some(fields) = doc.as_object_mut() {
        fields.insert("_key".into(), json!(req.space_type));
        fields.insert("status".into(), json!("draft"));
        fields.insert("drafted_by".into(), json!(drafted_by));
        fields.insert("requested_by".into(), json!(actor(&user)));
        fields.insert("drafted_at".into(), json!(now_secs()));
        fields.insert("skips".into(), json!(report.skips));
        fields.insert("sampled_chunks".into(), json!(report.sampled_chunks));
        fields.insert(
            "advisor".into(),
            serde_json::to_value(&report.advisor.rules).map_err(CogniGraphError::from)?,
        );
    }
    // Re-drafting replaces a prior unaccepted draft (drafts are inert).
    let _ = state
        .backend
        .delete_document(SPACE_TYPE_DRAFTS, &req.space_type)
        .await;
    // The delete is deliberately best-effort, so invalidate after the attempt
    // even when its result is unavailable: a backend error may follow effects.
    state.invalidate_search_results().await;
    state
        .backend
        .create_document(SPACE_TYPE_DRAFTS, doc.clone())
        .await?;
    state.invalidate_search_results().await;
    Ok(Json(json!({
        "space_type": req.space_type,
        "status": "draft",
        "drafted_by": drafted_by,
        "entities": report.space.entities.len(),
        "relation_rules": report.space.relation_rules.len(),
        "per_document": req.per_document,
        "documents": documents.as_ref().map(Vec::len),
        "skips": report.skips,
        "advisor": serde_json::to_value(&report.advisor.rules).map_err(CogniGraphError::from)?,
        "note": format!(
            "inert until accepted: review/edit `{SPACE_TYPE_DRAFTS}/{}` via the documents \
             API, then POST /api/construct/draft/{}/accept",
            req.space_type, req.space_type
        ),
    })))
}
