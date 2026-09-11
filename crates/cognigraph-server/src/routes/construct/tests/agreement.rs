//! Agreement.

use super::*;

#[tokio::test]
async fn agreement_lane_auto_accepts_on_concordance_with_pair_attribution() {
    let state = agreement_state(json!({
        "verdict": "accept", "confidence": 0.95, "reasoning": "direction faithful", "concerns": []
    }))
    .await;
    let Json(response) = review(
        State(state.clone()),
        Json(ReviewRequest {
            space_type: "pact".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["lane_a_plus"], "active");
    let accepted = response["auto_accepted"].as_array().unwrap();
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0]["lane"], "A+");
    let stored = state
        .backend
        .get_document(NEURONS, "h-gated")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "accepted");
    assert_eq!(
        stored["reviewed_by"],
        "judges:primary+partner@judge-policy-v2"
    );
    assert_eq!(stored["partner_verdict"], "accept");
    assert_eq!(stored["partner_judged_by"], "judge:partner@judge-policy-v2");
}
#[tokio::test]
async fn agreement_lane_queues_on_disagreement_with_both_verdicts() {
    let state = agreement_state(json!({
        "verdict": "needs_human", "confidence": 0.6, "reasoning": "direction unclear", "concerns": []
    }))
    .await;
    let Json(response) = review(
        State(state.clone()),
        Json(ReviewRequest {
            space_type: "pact".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["auto_accepted"].as_array().unwrap().len(), 0);
    let queued = &response["queued"].as_array().unwrap()[0];
    assert!(
        queued["lane_b_reason"]
            .as_str()
            .unwrap()
            .contains("no concordance")
    );
    let stored = state
        .backend
        .get_document(NEURONS, "h-gated")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "proposed"); // triage, never rejection
    assert_eq!(stored["judge_verdict"], "accept"); // primary
    assert_eq!(stored["partner_verdict"], "needs_human");
}
#[tokio::test]
async fn agreement_lane_degrades_to_queue_when_pair_mismatches_or_unattested() {
    // Partner model name differs from the attested pair -> withheld.
    let state = agreement_state(json!({
        "verdict": "accept", "confidence": 0.99, "reasoning": "fine", "concerns": []
    }))
    .await;
    let state = state.with_judge_partner(NamedScripted(
        json!({ "verdict": "accept", "confidence": 0.99, "reasoning": "fine", "concerns": [] }),
        "someone-else",
    ));
    let Json(response) = review(
        State(state.clone()),
        Json(ReviewRequest {
            space_type: "pact".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await
    .unwrap();
    assert!(
        response["lane_a_plus"]
            .as_str()
            .unwrap()
            .contains("withheld")
    );
    assert_eq!(response["auto_accepted"].as_array().unwrap().len(), 0);

    // Missing concordance attestation -> the whole review refuses.
    let state2 = agreement_state(json!({})).await;
    state2
        .managed_backend
        .update_document(
            REVIEW_POLICIES,
            "pact",
            json!({ "auto_accept": {
                "kinds": ["relation_hint"], "min_confidence": 0.9,
                "qualified_judges": ["primary"],
                "agreement": { "kinds": ["relation_hint"],
                               "judges": ["primary", "partner"],
                               "min_confidence": 0.9 }
            }}),
        )
        .await
        .unwrap();
    let refused = review(
        State(state2),
        Json(ReviewRequest {
            space_type: "pact".into(),
            limit: None,
            rejudge: false,
        }),
    )
    .await;
    assert!(refused.is_err());
}
