//! Gate advice.

use super::*;

#[derive(Deserialize)]
pub(super) struct AdviseRequest {
    pub(super) space_type: String,
    /// Inline corpus; when absent, the space's stored chunks are used.
    pub(super) chunks: Option<Vec<Chunk>>,
}
/// The gate advisor (deterministic, no LLM, read-only): per rule, where
/// a `require_in_sentence` gate is SAFE to apply (blocks off-subject
/// groundings, keeps the fact grounding) and which endpoints never
/// appear in any licensing sentence (the cross-company leakage
/// signature — flagged for the author with sample sentences, never
/// auto-suggested, because only the author can tell leakage from
/// legitimately cross-sentence evidence).
pub(super) async fn advise(
    State(state): State<AppState>,
    Json(req): Json<AdviseRequest>,
) -> Result<Json<Value>, AppError> {
    let space = load_space(&*state.managed_backend, &req.space_type).await?;
    let accepted = load_accepted(&*state.managed_backend, &req.space_type, "").await?;
    let chunks = match req.chunks {
        Some(chunks) if !chunks.is_empty() => chunks,
        _ => {
            let predicates = [cognigraph_core::FieldPredicate {
                path: vec!["space_id".into()],
                op: cognigraph_core::PredicateOp::Eq,
                value: json!(req.space_type),
            }];
            let stored: Vec<Chunk> = state
                .backend
                .list_documents_filtered("chunks", &predicates, None, None, None)
                .await?
                .into_iter()
                .filter_map(|doc| {
                    Some(Chunk {
                        id: doc.get("_key")?.as_str()?.to_string(),
                        title: doc
                            .get("title")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        text: doc.get("text")?.as_str()?.to_string(),
                    })
                })
                .collect();
            if stored.is_empty() {
                return Err(AppError(CogniGraphError::ValidationError(format!(
                    "no inline `chunks` and no stored chunks for space `{}` — ingest first \
                     or pass the corpus inline",
                    req.space_type
                ))));
            }
            stored
        }
    };
    let report = advise_gates(&space, &accepted, &chunks);
    let suggestions = report
        .rules
        .iter()
        .filter(|advice| advice.suggestion.is_some())
        .count();
    let review_flags = report
        .rules
        .iter()
        .filter(|advice| advice.suggestion.is_none() && !advice.never_in_sentence.is_empty())
        .count();
    Ok(Json(json!({
        "space_type": req.space_type,
        "chunks": chunks.len(),
        "suggestions": suggestions,
        "review_flags": review_flags,
        "rules": serde_json::to_value(&report.rules).map_err(|e| {
            AppError(CogniGraphError::BackendError(format!("serializing: {e}")))
        })?,
    })))
}
