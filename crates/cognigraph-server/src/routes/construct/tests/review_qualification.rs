//! Review qualification.

use super::*;

#[tokio::test]
async fn review_withholds_lane_a_from_unqualified_judge_models() {
    // The judge says accept at 0.99, the kind is whitelisted, the
    // rule is Lane-A eligible - but the active judge model is not in
    // qualified_judges: everything queues, verdict attached as triage.
    let state = seeded_state().await.with_judge(ScriptedCompletion(json!({
        "verdict": "accept", "confidence": 0.99,
        "reasoning": "verbatim and affirmed", "concerns": []
    })));
    store_policy(
        &state,
        json!({ "auto_accept": { "kinds": ["relation_hint"],
                                 "min_confidence": 0.9,
                                 "qualified_judges": ["gpt-5.4-mini"] } }),
    )
    .await;
    store_proposed(&state, "h-unqualified", "HOSTS").await;
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
    assert!(response["lane_a"].as_str().unwrap().contains("withheld"));
    let queued = &response["queued"].as_array().unwrap()[0];
    assert!(
        queued["lane_b_reason"]
            .as_str()
            .unwrap()
            .contains("qualified_judges")
    );
    let stored = state
        .backend
        .get_document(NEURONS, "h-unqualified")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "proposed"); // triage only, never authority
    assert_eq!(stored["judge_verdict"], "accept");
}
#[tokio::test]
async fn review_queues_below_threshold_and_gated_rules() {
    // Judge accepts but at 0.6 — below threshold: queued.
    let state = seeded_state().await.with_judge(ScriptedCompletion(json!({
        "verdict": "accept", "confidence": 0.6,
        "reasoning": "plausible but thin", "concerns": []
    })));
    store_policy(&state, json!({})).await;
    store_proposed(&state, "h-low", "HOSTS").await;
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
        .get_document(NEURONS, "h-low")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "proposed");
    assert_eq!(stored["judge_confidence"], 0.6);

    // A hint extending a SENTENCE-GATED rule queues as
    // precision-critical even at high confidence.
    let state = seeded_state().await.with_judge(ScriptedCompletion(json!({
        "verdict": "accept", "confidence": 0.99, "reasoning": "clear", "concerns": []
    })));
    state
        .managed_backend
        .update_document(
            "space_types",
            "acme",
            json!({ "relation_rules": [
                {"source": "Nimbus", "relation": "HOSTS", "target": "DataCloud",
                 "when_any": [], "require_in_sentence": ["source"]}
            ]}),
        )
        .await
        .unwrap();
    store_policy(&state, json!({})).await;
    store_proposed(&state, "h-gated", "HOSTS").await;
    let Json(response) = review(
        State(state),
        Json(ReviewRequest {
            space_type: "acme".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await
    .unwrap();
    let queued = &response["queued"].as_array().unwrap()[0];
    assert!(
        queued["lane_b_reason"]
            .as_str()
            .unwrap()
            .contains("sentence-gated")
    );
}
