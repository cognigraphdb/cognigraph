//! Fact-level review attribution: fact -> rule -> reviewer as a STORED LINK.
//!
//! Before this, a fact edge carried its evidence (chunk + trigger span) but not
//! WHO licensed it. Answering "who approved this edge?" meant joining
//! `fact.trigger` against `neuron.triggers` — an inference on a string, ambiguous
//! when two rules share a trigger. Likely a hard requirement for GxP validation.

use cognigraph_construct::*;
use serde_json::json;

fn space() -> SpaceType {
    serde_json::from_value(json!({
        "id": "attrib",
        "entities": [
            {"name": "Acme", "type": "company", "aliases": []},
            {"name": "DataCloud", "type": "product", "aliases": []},
        ],
        // An AUTHORED rule with an authored trigger.
        "relation_rules": [{
            "source": "Acme", "relation": "OPERATES", "target": "DataCloud",
            "when_any": ["operates"],
        }],
    }))
    .unwrap()
}

fn hint(id: &str, trigger: &str, reviewer: Option<&str>) -> Neuron {
    Neuron {
        id: id.into(),
        kind: NeuronKind::RelationHint,
        status: NeuronStatus::Accepted,
        reviewed_by: reviewer.map(String::from),
        source: "Acme".into(),
        relation: "OPERATES".into(),
        target: "DataCloud".into(),
        triggers: vec![trigger.into()],
        ..Neuron::default()
    }
}

/// An AUTHORED trigger licenses the fact: no neuron, no reviewer. Absence means
/// "the ontology author wrote this rule" — never "nobody approved it".
#[test]
fn a_fact_from_an_authored_trigger_carries_no_neuron() {
    let config = effective_config(&space(), &[]);
    let facts = ground_chunk("c1", "Acme operates DataCloud.", &config, &[]);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].licensed_by_neuron, None);
    assert_eq!(facts[0].reviewed_by, None);
}

/// THE CASE THAT MOTIVATED PER-TRIGGER ATTRIBUTION: a hint EXTENDS the authored
/// rule, so one rule's `when_any` now mixes an authored trigger with a
/// neuron-added one. Only the trigger that actually matched licenses the fact —
/// so attribution must follow the trigger, not the rule.
#[test]
fn attribution_follows_the_trigger_not_the_rule() {
    let neurons = vec![hint("n-42", "runs", Some("alice@corp"))];
    let config = effective_config(&space(), &neurons);

    // Matches the AUTHORED trigger -> unattributed.
    let authored = ground_chunk("c1", "Acme operates DataCloud.", &config, &[]);
    assert_eq!(authored[0].licensed_by_neuron, None, "authored trigger");
    assert_eq!(authored[0].reviewed_by, None);

    // Matches the NEURON's trigger, on the very same rule -> attributed.
    let extended = ground_chunk("c2", "Acme runs DataCloud.", &config, &[]);
    assert_eq!(extended[0].licensed_by_neuron.as_deref(), Some("n-42"));
    assert_eq!(extended[0].reviewed_by.as_deref(), Some("alice@corp"));
}

/// A hint that CREATES a rule attributes every fact it licenses.
#[test]
fn a_rule_created_by_a_neuron_attributes_its_facts() {
    let mut neuron = hint("n-7", "powers", Some("bob@corp"));
    neuron.target = "DataCloud".into();
    neuron.relation = "SUPPLIES".into();
    let config = effective_config(&space(), &[neuron]);
    let facts = ground_chunk("c1", "Acme powers DataCloud.", &config, &[]);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].licensed_by_neuron.as_deref(), Some("n-7"));
    assert_eq!(facts[0].reviewed_by.as_deref(), Some("bob@corp"));
}

/// A neuron accepted OUTSIDE the server's review route has no recorded reviewer.
/// The neuron is still attributed; `reviewed_by` is None. Absence means "not
/// recorded", never "nobody".
#[test]
fn an_unreviewed_acceptance_still_attributes_the_neuron() {
    let config = effective_config(&space(), &[hint("n-9", "runs", None)]);
    let facts = ground_chunk("c1", "Acme runs DataCloud.", &config, &[]);
    assert_eq!(facts[0].licensed_by_neuron.as_deref(), Some("n-9"));
    assert_eq!(facts[0].reviewed_by, None);
}

/// A REJECTED neuron licenses nothing — attribution cannot resurrect an
/// unaccepted rule.
#[test]
fn a_rejected_neuron_licenses_nothing() {
    let mut rejected = hint("n-bad", "runs", Some("carol@corp"));
    rejected.status = NeuronStatus::Rejected;
    let config = effective_config(&space(), &[rejected]);
    assert!(ground_chunk("c1", "Acme runs DataCloud.", &config, &[]).is_empty());
}
