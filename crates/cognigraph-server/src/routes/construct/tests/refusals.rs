//! CG-90: both construction origins record their gate refusals in the
//! ledger, in the same request, without changing the existing string
//! surfaces.

use super::*;
use crate::system_collections::REFUSALS_COLLECTION;

fn taxonomy() -> Vec<DirectedRelation> {
    vec![DirectedRelation {
        relation: "OWNS".into(),
        description: "The source owns the target.".into(),
        require_in_sentence: vec!["owns".into()],
    }]
}

fn nomination(source: &str, relation: &str, target: &str, evidence: &str, chunk: &str) -> Value {
    json!({
        "source": source, "source_type": "entity",
        "target": target, "target_type": "entity",
        "relation": relation, "evidence": evidence, "chunk_id": chunk,
    })
}

async fn run_directed(state: &AppState, text: &str) -> Value {
    let Json(response) = directed(
        State(state.clone()),
        None,
        Json(DirectedRequest {
            space_type: "dir".into(),
            taxonomy: taxonomy(),
            chunks: chunks(text),
        }),
    )
    .await
    .unwrap();
    response
}

#[tokio::test]
async fn directed_records_every_gate_refusal_and_keeps_the_skip_strings() {
    let text = "Ann owns Acme.";
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({
            "facts": [
                nomination("Ann", "OWNS", "Acme", text, "c1"),
                nomination("Ann", "SELLS", "Acme", text, "c1"),
                nomination("Ann", "OWNS", "Zed", text, "c1"),
                nomination("Ann", "OWNS", "Acme", text, "missing"),
            ]
        })));
    let response = run_directed(&state, text).await;
    assert_eq!(response["facts_grounded"], 1, "{response}");
    let skips = response["skips"].as_array().unwrap();
    assert_eq!(skips.len(), 3);
    assert!(
        skips.iter().all(Value::is_string),
        "skips stay strings: {response}"
    );

    let refusals = response["refusals"].as_array().unwrap();
    assert_eq!(refusals.len(), 3, "{response}");
    assert_eq!(response["refusals_stored"], 3);
    assert_eq!(response["refusals_dropped"], 0);
    let gates: Vec<&str> = refusals
        .iter()
        .map(|r| r["gate"].as_str().unwrap())
        .collect();
    assert_eq!(
        gates,
        [
            "relation_not_in_taxonomy",
            "target_not_in_sentence",
            "chunk_not_in_request"
        ]
    );
    for (refusal, skip) in refusals.iter().zip(skips) {
        assert_eq!(
            &refusal["reason"], skip,
            "reason text equals the skip string"
        );
        assert_eq!(refusal["stored"], true);
        let stored = state
            .backend
            .get_document(REFUSALS_COLLECTION, refusal["_key"].as_str().unwrap())
            .await
            .unwrap()
            .expect("row persisted in the same request");
        assert_eq!(stored["origin"], "directed");
        assert_eq!(stored["space_type"], "dir");
        assert_eq!(stored["policy"], "directed-policy-v2");
        assert_eq!(stored["attribution"], response["extracted_by"]);
        assert_eq!(stored["chunk_id"], refusal["chunk_id"]);
        assert_eq!(stored["evidence"], text);
    }
}

#[tokio::test]
async fn directed_with_no_refusals_writes_no_ledger_rows() {
    let text = "Ann owns Acme.";
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({
            "facts": [nomination("Ann", "OWNS", "Acme", text, "c1")]
        })));
    let response = run_directed(&state, text).await;
    assert_eq!(response["refusals"], json!([]), "{response}");
    assert_eq!(response["refusals_stored"], 0);
    assert_eq!(response["refusals_dropped"], 0);
    assert!(
        state
            .backend
            .list_collections()
            .await
            .unwrap()
            .iter()
            .all(|c| c.name != REFUSALS_COLLECTION)
    );
}

#[tokio::test]
async fn directed_that_grounds_nothing_still_records_its_refusals() {
    let text = "Ann owns Acme.";
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({
            "facts": [nomination("[***]", "OWNS", "Acme", text, "c1")]
        })));
    let response = run_directed(&state, text).await;
    assert_eq!(response["facts_grounded"], 0);
    assert_eq!(
        response["refusals"][0]["gate"], "unusable_endpoint_identity",
        "{response}"
    );
    assert_eq!(response["refusals_stored"], 1);
}

#[tokio::test]
async fn resubmitting_the_same_nominations_records_each_refusal_once() {
    let text = "Ann owns Acme.";
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({
            "facts": [nomination("Ann", "SELLS", "Acme", text, "c1")]
        })));
    let first = run_directed(&state, text).await;
    let second = run_directed(&state, text).await;
    assert_eq!(first["refusals"][0]["_key"], second["refusals"][0]["_key"]);
    assert_eq!(second["refusals_stored"], 1);
    let count = state
        .backend
        .list_collections()
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.name == REFUSALS_COLLECTION)
        .map(|c| c.count);
    assert_eq!(count, Some(1));
}

fn scripted_hint(confidence: f64) -> Value {
    json!({
        "id": "p-dup",
        "type": "relation_hint",
        "confidence": confidence,
        "rationale": "phrasing",
        "source": "Nimbus",
        "relation": "HOSTS",
        "target": "DataCloud",
        "triggers": ["nimbus hosts the datacloud platform"],
        "evidence": ["Nimbus hosts the DataCloud platform for Acme."]
    })
}

async fn ingested_state(script: Value) -> AppState {
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(script));
    let _ = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("Nimbus hosts the DataCloud platform for Acme."),
        }),
    )
    .await
    .unwrap();
    state
}

fn gap_request() -> Json<ProposeRequest> {
    Json(ProposeRequest {
        space_type: "acme".into(),
        gaps: Some(vec!["Nimbus --HOSTS--> DataCloud".into()]),
        eval: None,
        side_views: None,
    })
}

async fn assert_propose_rows(state: &AppState, response: &Value, gate: &str) {
    let skipped = response["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{response}");
    assert!(skipped[0]["fact"].is_string() && skipped[0]["reason"].is_string());
    let refusals = response["refusals"].as_array().unwrap();
    assert_eq!(refusals.len(), 1, "{response}");
    assert_eq!(refusals[0]["gate"], gate, "{response}");
    assert_eq!(refusals[0]["reason"], skipped[0]["reason"]);
    assert_eq!(response["refusals_stored"], 1);
    let stored = state
        .backend
        .get_document(REFUSALS_COLLECTION, refusals[0]["_key"].as_str().unwrap())
        .await
        .unwrap()
        .expect("row persisted");
    assert_eq!(stored["origin"], "propose");
    assert_eq!(stored["space_type"], "acme");
    assert_eq!(stored["policy"], json!(null));
    assert_eq!(stored["chunk_id"], json!(null));
    assert_eq!(stored["evidence"], json!(null));
    assert_eq!(stored["source"], "Nimbus");
    assert_eq!(stored["relation"], "HOSTS");
    assert_eq!(stored["target"], "DataCloud");
    assert!(
        stored["attribution"]
            .as_str()
            .unwrap()
            .starts_with("propose:")
    );
}

#[tokio::test]
async fn propose_records_a_duplicate_id_refusal_with_origin_propose() {
    let state = ingested_state(scripted_hint(0.8)).await;
    let Json(first) = propose(State(state.clone()), None, gap_request())
        .await
        .unwrap();
    assert_eq!(first["stored"], 1, "{first}");
    assert_eq!(first["refusals"], json!([]), "{first}");
    assert_eq!(first["refusals_stored"], 0);

    // The same scripted id again: stored nothing, refused as a duplicate.
    let Json(second) = propose(State(state.clone()), None, gap_request())
        .await
        .unwrap();
    assert_eq!(second["stored"], 0, "{second}");
    assert_propose_rows(&state, &second, "duplicate_id").await;
}

#[tokio::test]
async fn propose_records_a_rejected_proposal_and_stores_no_neuron() {
    // Confidence outside 0..=1 is refused inside the proposal step itself,
    // before the route's own validation branch, so the gate is the
    // pipeline's; the reason carries the validation message.
    let state = ingested_state(scripted_hint(2.0)).await;
    let Json(response) = propose(State(state.clone()), None, gap_request())
        .await
        .unwrap();
    assert_eq!(response["stored"], 0, "{response}");
    assert_propose_rows(&state, &response, "proposal_rejected").await;
    assert!(
        response["refusals"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("confidence"),
        "{response}"
    );
    assert!(
        state
            .backend
            .get_document(NEURONS, "p-dup")
            .await
            .unwrap()
            .is_none(),
        "a refused proposal never reaches the neurons collection"
    );
}
