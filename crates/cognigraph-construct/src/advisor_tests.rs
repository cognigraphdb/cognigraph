use super::*;

fn space(rules: serde_json::Value) -> SpaceType {
    serde_json::from_value(serde_json::json!({
        "id": "s",
        "entities": [
            {"name": "Harbor", "type": "org", "aliases": []},
            {"name": "Northstar", "type": "org", "aliases": ["NS"]},
            {"name": "Stack+", "type": "platform", "aliases": []}
        ],
        "relation_rules": rules
    }))
    .unwrap()
}

fn chunk(id: &str, text: &str) -> Chunk {
    Chunk {
        id: id.into(),
        title: String::new(),
        text: text.into(),
    }
}

#[test]
fn suggests_a_gate_that_blocks_leakage_and_keeps_recall() {
    let space = space(serde_json::json!([
        {"source": "Harbor", "relation": "SELECTED", "target": "Stack+",
         "when_any": ["selected stack+"]}
    ]));
    let chunks = vec![
        // Legitimate: the licensing sentence names Harbor.
        chunk("good", "Harbor selected Stack+ last spring."),
        // Leakage shape: trigger fires in a sentence about another org.
        chunk(
            "leak",
            "Harbor is discussed elsewhere. Northstar selected Stack+ for its stack.",
        ),
    ];
    let report = advise_gates(&space, &[], &chunks);
    let advice = &report.rules[0];
    assert_eq!(advice.grounds_ungated, 2);
    assert_eq!(advice.source_gate.grounds, 1);
    assert_eq!(advice.source_gate.blocked, 1);
    assert_eq!(advice.suggestion, Some(vec![EndpointRef::Source]));
    assert_eq!(advice.samples.len(), 1);
    assert_eq!(advice.samples[0].chunk_id, "leak");
    assert!(advice.samples[0].sentence.contains("Northstar"));
}

#[test]
fn flags_endpoints_never_in_any_licensing_sentence_for_review() {
    // The hostile-corpus shape: the rule grounds, but the endpoint is
    // in NO licensing sentence — the advisor must not silently pick a
    // side (trap vs cross-sentence evidence); it flags with samples.
    let space = space(serde_json::json!([
        {"source": "Harbor", "relation": "SELECTED", "target": "Stack+",
         "when_any": ["the platform decision"]}
    ]));
    let chunks = vec![chunk(
        "c1",
        "Harbor weighed options for months. The platform decision came in March.",
    )];
    let report = advise_gates(&space, &[], &chunks);
    let advice = &report.rules[0];
    assert_eq!(advice.grounds_ungated, 1);
    assert_eq!(advice.suggestion, None);
    assert_eq!(
        advice.never_in_sentence,
        vec![EndpointRef::Source, EndpointRef::Target]
    );
    assert!(advice.reason.contains("REVIEW"));
    assert_eq!(advice.samples.len(), 1);
    assert!(advice.samples[0].sentence.contains("platform decision"));
}

#[test]
fn declines_when_a_gate_changes_nothing_and_reports_dead_rules() {
    let space = space(serde_json::json!([
        {"source": "Harbor", "relation": "SELECTED", "target": "Stack+",
         "when_any": ["harbor selected stack+"]},
        {"source": "Northstar", "relation": "SELECTED", "target": "Stack+",
         "when_any": ["a phrase that appears nowhere"]}
    ]));
    let chunks = vec![chunk("c1", "Harbor selected Stack+ in 2024.")];
    let report = advise_gates(&space, &[], &chunks);
    // Suggestion-less rules sort after suggested ones; here both lack
    // suggestions, in authored order.
    let clean = report
        .rules
        .iter()
        .find(|a| a.fact.starts_with("Harbor"))
        .unwrap();
    assert_eq!(clean.suggestion, None);
    assert!(clean.reason.contains("already names both endpoints"));
    let dead = report
        .rules
        .iter()
        .find(|a| a.fact.starts_with("Northstar"))
        .unwrap();
    assert_eq!(dead.grounds_ungated, 0);
    assert!(dead.reason.contains("dead rule"));
}

#[test]
fn analyzes_the_effective_config_including_accepted_hints() {
    let space = space(serde_json::json!([]));
    let hint: Neuron = serde_json::from_value(serde_json::json!({
        "id": "h1", "type": "relation_hint", "status": "accepted",
        "source": "Harbor", "relation": "SELECTED", "target": "Stack+",
        "confidence": 0.9, "rationale": "t", "evidence": [],
        "triggers": ["went with stack+"]
    }))
    .unwrap();
    let chunks = vec![chunk(
        "c1",
        "Northstar went with Stack+. Harbor demurred entirely.",
    )];
    let report = advise_gates(&space, &[hint], &chunks);
    let advice = report
        .rules
        .iter()
        .find(|a| a.fact == "Harbor --SELECTED--> Stack+")
        .expect("hint-created rule analyzed");
    assert_eq!(advice.grounds_ungated, 1);
    // The only licensing sentence names Northstar, not Harbor: no
    // safe gate exists; the source endpoint is flagged for review.
    assert_eq!(advice.suggestion, None);
    assert!(advice.never_in_sentence.contains(&EndpointRef::Source));
}

/// A clinical space for the relation-semantics detector, which needs real
/// relation vocabulary to reason about.
fn clinical(rules: serde_json::Value) -> SpaceType {
    serde_json::from_value(serde_json::json!({
        "id": "labels",
        "entities": [
            {"name": "atorvastatin", "type": "drug", "aliases": []},
            {"name": "MI", "type": "condition", "aliases": []},
            {"name": "myopathy", "type": "condition", "aliases": []},
            {"name": "rhabdomyolysis", "type": "condition", "aliases": []},
            {"name": "Metformin", "type": "drug", "aliases": []},
            {"name": "lactic acidosis", "type": "condition", "aliases": []}
        ],
        "relation_rules": rules
    }))
    .unwrap()
}

/// The sentence is about PREVENTION ("reduce the risk of MI") while the
/// rule claims TREATS — the trigger is verbatim and affirmed, both
/// endpoints are governed, and the gate analysis sees nothing wrong.
#[test]
fn flags_a_relation_the_licensing_sentence_never_asserts() {
    let space = clinical(serde_json::json!([
        {"source": "atorvastatin", "relation": "TREATS", "target": "MI",
         "when_any": ["reduce the risk of mi"]}
    ]));
    let chunks = vec![chunk(
        "c1",
        "In adults, atorvastatin is indicated to reduce the risk of MI.",
    )];
    let report = advise_gates(&space, &[], &chunks);
    let advice = &report.rules[0];
    assert_eq!(advice.grounds_ungated, 1, "the fact does ground");
    assert_eq!(
        advice.semantics_suspect, 1,
        "{:?}",
        advice.semantics_samples
    );
    assert!(
        advice.semantics_samples[0]
            .signals
            .iter()
            .any(|s| s.contains("vocabulary")),
        "{:?}",
        advice.semantics_samples[0]
    );
}

/// `myopathy and rhabdomyolysis` is an enumeration, not an assertion that
/// one causes the other.
#[test]
fn flags_endpoints_that_are_merely_co_listed() {
    let space = clinical(serde_json::json!([
        {"source": "myopathy", "relation": "CAUSES", "target": "rhabdomyolysis",
         "when_any": ["myopathy and rhabdomyolysis"]}
    ]));
    let chunks = vec![chunk(
        "c1",
        "Skeletal muscle effects such as myopathy and rhabdomyolysis were observed.",
    )];
    let report = advise_gates(&space, &[], &chunks);
    let advice = &report.rules[0];
    assert_eq!(advice.semantics_suspect, 1);
    assert!(
        advice.semantics_samples[0]
            .signals
            .iter()
            .any(|s| s.contains("co-listed")),
        "{:?}",
        advice.semantics_samples[0]
    );
}

/// The detector must stay quiet on a well-evidenced fact, and in
/// particular must NOT fire merely because the SOURCE is absent from the
/// sentence. Source absence measured ANTI-correlated with error on the
/// pilot (a label section names its drug once, then refers to it
/// implicitly), which is why it is not one of the signals.
#[test]
fn stays_quiet_on_a_well_evidenced_fact_even_without_the_source() {
    let space = clinical(serde_json::json!([
        {"source": "Metformin", "relation": "CAUSES", "target": "lactic acidosis",
         "when_any": ["may cause lactic acidosis"]}
    ]));
    let chunks = vec![chunk(
        "c1",
        "Metformin has a boxed warning. It may cause lactic acidosis in renal impairment.",
    )];
    let report = advise_gates(&space, &[], &chunks);
    let advice = &report.rules[0];
    assert_eq!(advice.grounds_ungated, 1);
    assert_eq!(
        advice.semantics_suspect, 0,
        "source absence alone must never flag: {:?}",
        advice.semantics_samples
    );
    // ...while the endpoint-presence view DOES flag it — the two
    // detectors are deliberately independent.
    assert!(advice.never_in_sentence.contains(&EndpointRef::Source));
}
