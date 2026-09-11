//! Evaluation.

use super::*;

#[derive(Deserialize)]
pub(super) struct EvaluateRequest {
    /// Space id — used to scope the facts being measured.
    pub(super) space_type: String,
    /// Inline eval spec; when absent, a pre-existing legacy spec is read
    /// from `eval_specs/{space_type}`. M25 blocks new generic mutations of
    /// that managed collection, so new workflows pass the spec inline; the
    /// independently governed M18-M23 evaluation context/evidence workflow
    /// remains the promotion authority path.
    pub(super) eval: Option<EvalSpec>,
}
pub(super) fn fact_lines(facts: &[Fact]) -> Vec<String> {
    facts
        .iter()
        .map(|f| format!("{} --{}--> {}", f.source, f.relation, f.target))
        .collect()
}
/// Resolve an eval spec: inline when given, else the stored
/// `eval_specs/{space_type}` document.
pub(crate) async fn resolve_spec(
    state: &AppState,
    space_type: &str,
    inline: Option<EvalSpec>,
) -> Result<EvalSpec, AppError> {
    if let Some(spec) = inline {
        return Ok(spec);
    }
    let doc = state
        .backend
        .get_document(EVAL_SPECS, space_type)
        .await?
        .ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(format!(
                "no inline `eval` and no stored spec `{space_type}` in `{EVAL_SPECS}` \
                 (pass `eval` inline; legacy stored specs remain readable)"
            )))
        })?;
    serde_json::from_value(doc).map_err(|e| {
        AppError(CogniGraphError::ValidationError(format!(
            "bad stored eval spec: {e}"
        )))
    })
}
pub(super) async fn evaluate_space(
    State(state): State<AppState>,
    Json(req): Json<EvaluateRequest>,
) -> Result<Json<Value>, AppError> {
    let spec = resolve_spec(&state, &req.space_type, req.eval).await?;
    let outcome = evaluate(&*state.managed_backend, &req.space_type, &spec).await?;
    Ok(Json(json!({
        "space_type": req.space_type,
        "recall": { "found": outcome.expected_found, "total": outcome.expected_total },
        "restraint": { "violations": outcome.forbidden_triggered, "total": outcome.forbidden_total },
        "recall_ok": outcome.recall_ok(),
        "restraint_ok": outcome.restraint_ok(),
        "missing": fact_lines(&outcome.missing),
        "violations": fact_lines(&outcome.violations),
    })))
}
