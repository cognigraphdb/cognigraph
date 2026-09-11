//! Answer evaluation.

use super::*;

#[derive(Deserialize)]
pub(super) struct AnswerEvalRequest {
    pub(super) space_type: String,
    /// Inline eval spec; when absent, `eval_specs/{space_type}` is used.
    pub(super) eval: Option<EvalSpec>,
    /// Completeness-critic second pass (answers::AnswerOpts::two_pass).
    #[serde(default)]
    pub(super) two_pass: bool,
    /// Augment trace fact lines with their licensing sentences
    /// (answers::AnswerOpts::evidence_sentences).
    #[serde(default)]
    pub(super) evidence_sentences: bool,
}
/// Answer-level measurement: `/evaluate` proves the facts EXIST;
/// this proves they carry through the graph-augmented trace into a
/// model's ANSWERS — per-question recall and restraint, with
/// fabrications listed. Needs the completion provider (it asks a
/// model), so unlike /evaluate it is not deterministic; restraint
/// here is a behavioral measurement, never a structural guarantee.
pub(super) async fn answer_eval_route(
    State(state): State<AppState>,
    Json(req): Json<AnswerEvalRequest>,
) -> Result<Json<Value>, AppError> {
    let Some(provider) = state.completion.clone() else {
        return Err(AppError(CogniGraphError::BackendError(
            "No completion provider configured. Set OPENAI_API_KEY or GEMINI_API_KEY \
             (model via COGNIGRAPH_COMPLETION_MODEL)."
                .into(),
        )));
    };
    let spec = resolve_spec(&state, &req.space_type, req.eval).await?;
    let space = load_space(&*state.managed_backend, &req.space_type).await?;
    let accepted = load_accepted(&*state.managed_backend, &req.space_type, "").await?;
    let outcome = answer_eval_with(
        &*state.managed_backend,
        &req.space_type,
        &space,
        &accepted,
        &spec,
        &*provider,
        AnswerOpts {
            two_pass: req.two_pass,
            evidence_sentences: req.evidence_sentences,
        },
    )
    .await
    .map_err(|e| AppError(CogniGraphError::BackendError(format!("answering: {e}"))))?;
    Ok(Json(json!({
        "space_type": req.space_type,
        "two_pass": req.two_pass,
        "evidence_sentences": req.evidence_sentences,
        "recall_ok": outcome.recall_ok(),
        "restraint_ok": outcome.restraint_ok(),
        "questions": outcome.questions.iter().map(|q| json!({
            "id": q.id,
            "question": q.question,
            "recall": { "found": q.expected_found, "total": q.expected_total },
            "restraint": { "asserted": q.forbidden_asserted, "total": q.forbidden_total },
            "asserted": q.asserted,
            "fabricated": q.fabricated,
        })).collect::<Vec<_>>(),
    })))
}
