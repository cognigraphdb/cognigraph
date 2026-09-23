use super::*;
use cognigraph_construct::Neuron;
use serde_json::json;

fn neuron(kind: &str, status: &str, relation: &str) -> Neuron {
    serde_json::from_value(json!({
        "id": format!("{kind}-{status}-{relation}"),
        "type": kind,
        "status": status,
        "confidence": 0.9,
        "evidence": ["trace review"],
        "relation": relation,
        "boost": 25.0,
    }))
    .unwrap()
}

#[test]
fn no_neurons_means_no_warning() {
    assert!(inert_rank_hint_warning(&[], "document_relations").is_none());
}

#[test]
fn accepted_rank_hint_on_a_non_facts_collection_warns_with_count() {
    let neurons = [
        neuron("relation_rank_hint", "accepted", "SUPPLIES"),
        neuron("relation_rank_hint", "accepted", "MENTIONS"),
    ];
    let warning = inert_rank_hint_warning(&neurons, "document_relations").unwrap();
    assert_eq!(warning["code"], INERT_RANK_HINTS);
    assert_eq!(warning["accepted_rank_hints"], 2);
    assert_eq!(warning["edge_collection"], "document_relations");
    assert!(warning["message"].as_str().unwrap().contains("facts"));
}

#[test]
fn traversing_facts_never_warns() {
    let neurons = [neuron("relation_rank_hint", "accepted", "SUPPLIES")];
    assert!(inert_rank_hint_warning(&neurons, "facts").is_none());
}

#[test]
fn only_accepted_rank_hints_count() {
    // Proposed, rejected and retired rank hints are inert by lifecycle;
    // accepted hints of other kinds never reweight ranking at all.
    let neurons = [
        neuron("relation_rank_hint", "proposed", "SUPPLIES"),
        neuron("relation_rank_hint", "rejected", "SUPPLIES"),
        neuron("relation_rank_hint", "retired", "SUPPLIES"),
        neuron("relation_hint", "accepted", "SUPPLIES"),
        neuron("relation_blocker", "accepted", "SUPPLIES"),
        neuron("alias", "accepted", ""),
    ];
    assert!(inert_rank_hint_warning(&neurons, "document_relations").is_none());
    let with_one = [
        &neurons[..],
        &[neuron("relation_rank_hint", "accepted", "X")],
    ]
    .concat();
    assert_eq!(
        inert_rank_hint_warning(&with_one, "document_relations").unwrap()["accepted_rank_hints"],
        1
    );
}

#[test]
fn collection_identity_is_exact() {
    // Collection names are exact identities (CG-5): "Facts", " facts" or
    // "facts/" are different collections and do not hold neuron facts.
    let neurons = [neuron("relation_rank_hint", "accepted", "SUPPLIES")];
    for name in [
        "Facts",
        "FACTS",
        " facts",
        "facts ",
        "facts/",
        "tenant/facts",
        "",
    ] {
        assert!(
            inert_rank_hint_warning(&neurons, name).is_some(),
            "{name:?} must warn"
        );
    }
}
