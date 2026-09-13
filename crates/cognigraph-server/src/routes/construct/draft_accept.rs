//! Draft accept.

use super::*;

/// Accept a draft: the explicit, attributed human act that turns a
/// draft into vocabulary (D1). Uses the CURRENT stored draft — editing
/// it via the documents API before accepting is the intended review
/// path — and re-validates that it still parses as a space type (a
/// typo'd gate enum fails loudly here, not silently at grounding).
pub(super) async fn draft_accept(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    user: Option<Extension<User>>,
) -> Result<Json<Value>, AppError> {
    let doc = state
        .backend
        .get_document(SPACE_TYPE_DRAFTS, &id)
        .await?
        .ok_or_else(|| {
            AppError(CogniGraphError::ValidationError(format!(
                "no draft `{id}` in `{SPACE_TYPE_DRAFTS}`"
            )))
        })?;
    if doc.get("status").and_then(Value::as_str) != Some("draft") {
        return Err(AppError(CogniGraphError::ValidationError(format!(
            "draft `{id}` is not in draft status"
        ))));
    }
    if state
        .backend
        .get_document(SPACE_TYPES, &id)
        .await?
        .is_some()
    {
        return Err(AppError(CogniGraphError::ValidationError(format!(
            "space `{id}` already exists in `{SPACE_TYPES}`"
        ))));
    }
    let space: SpaceType = serde_json::from_value(doc.clone()).map_err(|e| {
        AppError(CogniGraphError::ValidationError(format!(
            "draft `{id}` no longer parses as a space type (edited?): {e}"
        )))
    })?;
    let accepted_by = actor(&user);
    let now = now_secs();
    let mut accepted = serde_json::to_value(&space).map_err(CogniGraphError::from)?;
    if let Some(fields) = accepted.as_object_mut() {
        fields.insert("_key".into(), json!(id));
        fields.insert(
            "drafted_by".into(),
            doc.get("drafted_by").cloned().unwrap_or(Value::Null),
        );
        fields.insert("accepted_by".into(), json!(accepted_by));
        fields.insert("accepted_at".into(), json!(now));
    }
    state
        .managed_backend
        .create_document(SPACE_TYPES, accepted)
        .await?;
    state.invalidate_search_results().await;
    state
        .managed_backend
        .update_document(
            SPACE_TYPE_DRAFTS,
            &id,
            json!({ "status": "accepted", "accepted_by": accepted_by, "accepted_at": now }),
        )
        .await?;
    state.invalidate_search_results().await;
    Ok(Json(json!({
        "space_type": id,
        "status": "accepted",
        "accepted_by": accepted_by,
        "entities": space.entities.len(),
        "relation_rules": space.relation_rules.len(),
    })))
}
