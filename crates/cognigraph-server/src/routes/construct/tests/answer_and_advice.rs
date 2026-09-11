//! Answer and advice.

use super::*;

#[tokio::test]
async fn answer_eval_scores_answers_and_requires_a_provider() {
    // No completion provider -> loud refusal.
    let state = seeded_state().await;
    let result = answer_eval_route(
        State(state.clone()),
        Json(AnswerEvalRequest {
            space_type: "acme".into(),
            eval: None,
            two_pass: false,
            evidence_sentences: false,
        }),
    )
    .await;
    assert!(result.is_err());

    let state = state.with_completion(ScriptedCompletion(json!({
        "facts": ["Nimbus --SUPPLIES--> DataCloud"]
    })));
    let _ = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("Everyone knows DataCloud runs on Nimbus these days."),
        }),
    )
    .await
    .unwrap();
    let spec: EvalSpec = serde_json::from_value(json!({
        "space_id": "acme",
        "questions": [{
            "id": "q1",
            "question": "Who supplies DataCloud?",
            "expected_facts": ["Nimbus --SUPPLIES--> DataCloud"],
            "forbidden_facts": ["DataCloud --SUPPLIES--> Nimbus"]
        }]
    }))
    .unwrap();
    // two_pass: the scripted provider answers both passes with the
    // same fact — the union dedupes, recall 1/1, nothing fabricated.
    let Json(report) = answer_eval_route(
        State(state.clone()),
        Json(AnswerEvalRequest {
            space_type: "acme".into(),
            eval: Some(spec),
            two_pass: true,
            evidence_sentences: false,
        }),
    )
    .await
    .unwrap();
    assert_eq!(report["recall_ok"], true);
    assert_eq!(report["restraint_ok"], true);
    assert_eq!(report["questions"][0]["recall"]["found"], 1);
    assert_eq!(
        report["questions"][0]["asserted"].as_array().unwrap().len(),
        1
    );
    assert!(
        report["questions"][0]["fabricated"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn advise_reports_safe_gates_and_review_flags() {
    let state = seeded_state().await;
    // Inline corpus: one on-subject grounding, one whose licensing
    // sentence lacks BOTH endpoints (off-subject leakage shape).
    let corpus = vec![
        serde_json::from_value::<Chunk>(
            json!({"id": "good", "text": "Everyone knows DataCloud runs on Nimbus."}),
        )
        .unwrap(),
        serde_json::from_value::<Chunk>(json!({
            "id": "leak",
            "text": "Nimbus and DataCloud aside. Rivals say DataCloud runs on Nimbus hardware."
        }))
        .unwrap(),
    ];
    let Json(response) = advise(
        State(state.clone()),
        Json(AdviseRequest {
            space_type: "acme".into(),
            chunks: Some(corpus),
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["chunks"], 2);
    let rules = response["rules"].as_array().unwrap();
    let supplies = rules
        .iter()
        .find(|r| r["fact"] == "Nimbus --SUPPLIES--> DataCloud")
        .unwrap();
    // Both sentences name both endpoints here — nothing to gate.
    assert_eq!(supplies["grounds_ungated"], 2);
    assert!(supplies["suggestion"].is_null());

    // No inline chunks and nothing ingested -> a loud refusal.
    let result = advise(
        State(state.clone()),
        Json(AdviseRequest {
            space_type: "acme".into(),
            chunks: None,
        }),
    )
    .await;
    assert!(result.is_err());

    // After ingest, the stored corpus is used.
    let _ = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("Everyone knows DataCloud runs on Nimbus these days."),
        }),
    )
    .await
    .unwrap();
    let Json(response) = advise(
        State(state),
        Json(AdviseRequest {
            space_type: "acme".into(),
            chunks: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["chunks"], 1);
}
