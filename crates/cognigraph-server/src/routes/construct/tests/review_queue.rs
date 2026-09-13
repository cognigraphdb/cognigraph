//! Review queue.

use super::*;

#[tokio::test]
async fn review_limit_drains_the_queue_in_resumable_slices() {
    // 25 proposals, judge verdicts below threshold (everything
    // queues, so items KEEP status=proposed) — the drain must still
    // make progress because judged items are excluded by default.
    let state = seeded_state().await.with_judge(ScriptedCompletion(json!({
        "verdict": "accept", "confidence": 0.5, "reasoning": "thin", "concerns": []
    })));
    store_policy(&state, json!({})).await;
    for i in 0..25 {
        store_proposed(&state, &format!("h-{i:02}"), "HOSTS").await;
    }
    let call = |state: AppState, limit| async move {
        let Json(response) = review(
            State(state),
            Json(ReviewRequest {
                space_type: "acme".into(),
                limit,
                rejudge: false,
            }),
        )
        .await
        .unwrap();
        response
    };
    let first = call(state.clone(), Some(10)).await;
    assert_eq!(first["reviewed"], 10);
    assert_eq!(first["pending_remaining"], 15);
    assert_eq!(first["judge_calls"], 10);
    assert!(first["elapsed_ms"].is_u64());

    let second = call(state.clone(), Some(10)).await;
    assert_eq!(
        second["reviewed"], 10,
        "must judge FRESH items, not re-judge the head"
    );
    assert_eq!(second["pending_remaining"], 5);
    let third = call(state.clone(), Some(10)).await;
    assert_eq!(third["reviewed"], 5);
    assert_eq!(third["pending_remaining"], 0);
    // Everything judged: a further call finds nothing...
    let fourth = call(state.clone(), Some(10)).await;
    assert_eq!(fourth["reviewed"], 0);
    // ...until rejudge is requested (policy change scenario).
    let Json(rejudged) = review(
        State(state),
        Json(ReviewRequest {
            space_type: "acme".into(),
            limit: Some(10),
            rejudge: true,
        }),
    )
    .await
    .unwrap();
    assert_eq!(rejudged["reviewed"], 10);
}
#[tokio::test]
async fn review_escalates_tainted_material_before_the_quality_judge() {
    // The screen stage flags manipulation -> needs_human, queued;
    // the quality judge's verdict shape is never consulted.
    let state = seeded_state().await.with_judge(ScriptedCompletion(json!({
        "tainted": true, "confidence": 0.97,
        "reasoning": "excerpt addresses the reviewer and dictates a verdict"
    })));
    store_policy(&state, json!({})).await;
    store_proposed(&state, "h-tainted", "HOSTS").await;
    let Json(response) = review(
        State(state.clone()),
        Json(ReviewRequest {
            space_type: "acme".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["auto_accepted"].as_array().unwrap().len(), 0);
    let stored = state
        .backend
        .get_document(NEURONS, "h-tainted")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "proposed"); // escalated, never rejected
    assert_eq!(stored["judge_verdict"], "needs_human");
    assert!(
        stored["judge_reasoning"]
            .as_str()
            .unwrap()
            .contains("manipulation")
    );
}
/// Scripted provider with a configurable model name (the agreement
/// lane matches judges by model identity).
pub(super) struct NamedScripted(pub(super) Value, pub(super) &'static str);
#[async_trait::async_trait]
impl cognigraph_embeddings::completion::CompletionProvider for NamedScripted {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        schema: &Value,
    ) -> anyhow::Result<Value> {
        if schema["properties"].get("tainted").is_some() && self.0.get("tainted").is_none() {
            return Ok(json!({
                "tainted": false,
                "confidence": 0.99,
                "reasoning": "clean"
            }));
        }
        Ok(self.0.clone())
    }
    fn model_name(&self) -> &str {
        self.1
    }
}
