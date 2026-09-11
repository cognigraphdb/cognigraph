//! Evaluation.

use super::*;

#[tokio::test]
async fn evaluate_measures_recall_and_restraint() {
    let state = seeded_state().await;
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
            "expected_facts": [
                "Nimbus --SUPPLIES--> DataCloud",
                "Nimbus --HOSTS--> DataCloud"
            ],
            "forbidden_facts": ["DataCloud --SUPPLIES--> Nimbus"]
        }]
    }))
    .unwrap();

    // Inline spec.
    let Json(report) = evaluate_space(
        State(state.clone()),
        Json(EvaluateRequest {
            space_type: "acme".into(),
            eval: Some(spec.clone()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(report["recall"]["found"], 1);
    assert_eq!(report["recall"]["total"], 2);
    assert_eq!(report["restraint"]["violations"], 0);
    assert_eq!(report["recall_ok"], false);
    assert_eq!(report["restraint_ok"], true);
    assert_eq!(report["missing"][0], "Nimbus --HOSTS--> DataCloud");

    // Stored spec: same result from the eval_specs collection.
    let mut doc = serde_json::to_value(&spec).unwrap();
    doc["_key"] = json!("acme");
    state
        .managed_backend
        .create_document(EVAL_SPECS, doc)
        .await
        .unwrap();
    let Json(report) = evaluate_space(
        State(state.clone()),
        Json(EvaluateRequest {
            space_type: "acme".into(),
            eval: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(report["recall"]["found"], 1);

    // No inline spec and nothing stored -> validation error.
    let missing = evaluate_space(
        State(state),
        Json(EvaluateRequest {
            space_type: "other".into(),
            eval: None,
        }),
    )
    .await;
    assert!(missing.is_err());
}
#[tokio::test]
async fn unknown_space_and_empty_chunks_rejected() {
    let state = seeded_state().await;
    assert!(
        ingest(
            State(state.clone()),
            Json(IngestRequest {
                space_type: "nope".into(),
                chunks: chunks("text"),
            }),
        )
        .await
        .is_err()
    );
    assert!(
        ingest(
            State(state),
            Json(IngestRequest {
                space_type: "acme".into(),
                chunks: Vec::new(),
            }),
        )
        .await
        .is_err()
    );
}
