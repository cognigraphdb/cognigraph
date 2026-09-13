//! Review policy.

use super::*;

#[tokio::test]
async fn review_requires_policy_attestation_and_whitelist() {
    // No policy stored -> refused (today's all-human behavior).
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({})));
    let result = review(
        State(state.clone()),
        Json(ReviewRequest {
            space_type: "acme".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await;
    assert!(result.is_err());

    // Policy without the D5 injection attestation -> refused.
    store_policy(&state, json!({ "injection_suite_passed": false })).await;
    let result = review(
        State(state.clone()),
        Json(ReviewRequest {
            space_type: "acme".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await;
    assert!(result.is_err());

    // A policy whose auto_accept lacks qualified_judges -> refused
    // (Lane A authority is model-bound).
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({})));
    store_policy(
        &state,
        json!({ "auto_accept": { "kinds": ["relation_hint"], "min_confidence": 0.9 } }),
    )
    .await;
    let result = review(
        State(state),
        Json(ReviewRequest {
            space_type: "acme".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await;
    assert!(result.is_err());

    // Blocker and rank policies are refused until each has a qualified,
    // kind-specific judge packet.
    for unsafe_kind in ["relation_blocker", "relation_rank_hint"] {
        let state = seeded_state()
            .await
            .with_completion(ScriptedCompletion(json!({})));
        store_policy(
            &state,
            json!({ "auto_accept": { "kinds": [unsafe_kind], "min_confidence": 0.9 } }),
        )
        .await;
        let result = review(
            State(state),
            Json(ReviewRequest {
                space_type: "acme".into(),
                limit: None,
                rejudge: false,
            }),
        )
        .await;
        assert!(result.is_err(), "unsafe kind `{unsafe_kind}` was accepted");
    }
}
#[tokio::test]
async fn legacy_alias_policy_queues_alias_while_relation_hint_still_auto_accepts() {
    let state = seeded_state().await.with_judge(ScriptedCompletion(json!({
        "verdict": "accept", "confidence": 0.95,
        "reasoning": "supported by the packet", "concerns": []
    })));
    store_policy(
        &state,
        json!({
            "auto_accept": {
                "kinds": ["relation_hint", "alias"],
                "min_confidence": 0.9,
                "qualified_judges": ["scripted"]
            }
        }),
    )
    .await;
    state
        .managed_backend
        .create_document(
            NEURONS,
            json!({
                "_key": "a-legacy", "id": "a-legacy", "type": "alias",
                "status": "proposed", "space_type": "acme", "confidence": 0.8,
                "evidence": ["Nimbus is also known as Nimbus Corp"],
                "entity": "Nimbus", "aliases": ["Nimbus Corp"]
            }),
        )
        .await
        .unwrap();
    store_proposed(&state, "h-legacy-policy", "HOSTS").await;

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
    assert_eq!(response["auto_accepted"].as_array().unwrap().len(), 1);
    assert_eq!(response["auto_accepted"][0]["key"], "h-legacy-policy");
    assert_eq!(response["queued"].as_array().unwrap().len(), 1);
    assert_eq!(response["queued"][0]["key"], "a-legacy");
    assert!(
        response["queued"][0]["lane_b_reason"]
            .as_str()
            .unwrap()
            .contains("alias auto-accept is disabled")
    );
    assert_eq!(
        state
            .backend
            .get_document(NEURONS, "a-legacy")
            .await
            .unwrap()
            .unwrap()["status"],
        "proposed"
    );
    assert_eq!(
        state
            .backend
            .get_document(NEURONS, "h-legacy-policy")
            .await
            .unwrap()
            .unwrap()["status"],
        "accepted"
    );
}
#[test]
fn review_policy_omission_uses_the_documented_sampling_default() {
    let policy: ReviewPolicy = serde_json::from_value(json!({
        "auto_accept": {
            "kinds": ["relation_hint"],
            "min_confidence": 0.9,
            "qualified_judges": ["qualified-model"]
        },
        "injection_suite_passed": true
    }))
    .unwrap();

    assert_eq!(policy.sampling_rate, 0.1);
}
