//! Proposal.

use super::*;

#[tokio::test]
async fn propose_stores_inert_reviewable_neurons() {
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({
            "id": "p1",
            "type": "relation_hint",
            "confidence": 0.8,
            "rationale": "phrasing the ontology missed",
            "source": "Nimbus",
            "relation": "HOSTS",
            "target": "DataCloud",
            "triggers": ["nimbus hosts the datacloud platform"],
            "evidence": ["Nimbus hosts the DataCloud platform for Acme."]
        })));
    let _ = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("Nimbus hosts the DataCloud platform for Acme."),
        }),
    )
    .await
    .unwrap();

    let Json(response) = propose(
        State(state.clone()),
        None,
        Json(ProposeRequest {
            space_type: "acme".into(),
            gaps: Some(vec!["Nimbus --HOSTS--> DataCloud".into()]),
            eval: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["stored"], 1);
    assert_eq!(response["proposed"][0]["id"], "p1");

    // Inert: stored as proposed, attributed, NOT influencing the graph.
    let stored = state
        .backend
        .get_document(NEURONS, "p1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["status"], "proposed");
    assert_eq!(stored["proposed_by"], "anonymous");
    assert!(stored["proposed_at"].as_u64().unwrap() > 0);

    // Re-proposing the same id is a visible skip, not a failure.
    let Json(again) = propose(
        State(state.clone()),
        None,
        Json(ProposeRequest {
            space_type: "acme".into(),
            gaps: Some(vec!["Nimbus --HOSTS--> DataCloud".into()]),
            eval: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(again["stored"], 0);
    assert_eq!(again["skipped"][0]["reason"], "neuron id already exists");
}
#[tokio::test]
async fn propose_without_provider_or_with_bad_gap_fails() {
    let state = seeded_state().await; // no completion provider
    let result = propose(
        State(state.clone()),
        None,
        Json(ProposeRequest {
            space_type: "acme".into(),
            gaps: Some(vec!["Nimbus --HOSTS--> DataCloud".into()]),
            eval: None,
        }),
    )
    .await;
    assert!(result.is_err());

    let state = state.with_completion(ScriptedCompletion(json!({})));
    let result = propose(
        State(state),
        None,
        Json(ProposeRequest {
            space_type: "acme".into(),
            gaps: Some(vec!["not a fact line".into()]),
            eval: None,
        }),
    )
    .await;
    assert!(result.is_err());
}
