//! Review lanes.

use super::*;

#[tokio::test]
async fn review_applies_lanes_with_attribution_and_sampling() {
    // Judge accepts at 0.95 — above the 0.9 threshold.
    let state = seeded_state().await.with_judge(ScriptedCompletion(json!({
        "verdict": "accept", "confidence": 0.95,
        "reasoning": "verbatim, affirmative, right endpoints", "concerns": []
    })));
    store_policy(&state, json!({})).await;
    // Hint extends the EXISTING ungated HOSTS rule -> Lane A eligible.
    store_proposed(&state, "h-auto", "HOSTS").await;

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

    let stored = state
        .backend
        .get_document(NEURONS, "h-auto")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "accepted");
    assert_eq!(stored["reviewed_by"], "judge:scripted@judge-policy-v2");
    assert_eq!(stored["judge_confidence"], 0.95);
    assert_eq!(stored["audit_sample"], true); // sampling_rate 1.0
    assert!(
        stored["judge_reasoning"]
            .as_str()
            .unwrap()
            .contains("verbatim")
    );

    // A hint that would CREATE a new triple queues with the verdict
    // attached (Lane B), even though the judge said accept.
    store_proposed(&state, "h-new-triple", "SPONSORS").await;
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
    let queued = &response["queued"].as_array().unwrap()[0];
    assert_eq!(queued["key"], "h-new-triple");
    assert!(
        queued["lane_b_reason"]
            .as_str()
            .unwrap()
            .contains("new triple")
    );
    let stored = state
        .backend
        .get_document(NEURONS, "h-new-triple")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "proposed"); // never auto-rejected
    assert_eq!(stored["judge_verdict"], "accept");
}
