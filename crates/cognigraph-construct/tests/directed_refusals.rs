//! CG-90: every directed gate refusal is a structured record, not only a
//! string, so it can be stored and traced. The human-readable reason text is
//! unchanged from the pre-existing `skips` strings.

use cognigraph_construct::directed::{DirectedProposal, gate_directed_proposals};
use cognigraph_construct::refusals::{DirectedGate, Refusal};
use cognigraph_construct::{Chunk, DirectedRelation};

fn taxonomy() -> Vec<DirectedRelation> {
    vec![DirectedRelation {
        relation: "OWNS".into(),
        description: "The source owns the target.".into(),
        require_in_sentence: vec!["owns".into()],
    }]
}

fn chunk(id: &str, text: &str) -> Chunk {
    Chunk {
        id: id.into(),
        title: "test".into(),
        text: text.into(),
    }
}

fn proposal(
    source: &str,
    relation: &str,
    target: &str,
    evidence: &str,
    chunk_id: &str,
) -> DirectedProposal {
    DirectedProposal {
        source: source.into(),
        source_type: "entity".into(),
        target: target.into(),
        target_type: "entity".into(),
        relation: relation.into(),
        evidence: evidence.into(),
        chunk_id: chunk_id.into(),
    }
}

fn refuse(text: &str, p: DirectedProposal) -> Vec<Refusal> {
    let (facts, _, refusals) =
        gate_directed_proposals(&[chunk("c1", text)], &taxonomy(), &[p], "test");
    assert!(facts.is_empty(), "gate must refuse: {refusals:?}");
    refusals
}

#[test]
fn each_directed_gate_yields_exactly_one_refusal_with_its_code() {
    let text = "Ann owns Acme.";
    let cases: Vec<(DirectedGate, DirectedProposal)> = vec![
        (
            DirectedGate::RelationNotInTaxonomy,
            proposal("Ann", "SELLS", "Acme", text, "c1"),
        ),
        (
            DirectedGate::ChunkNotInRequest,
            proposal("Ann", "OWNS", "Acme", text, "missing"),
        ),
        (
            DirectedGate::EmptyEndpoint,
            proposal("", "OWNS", "Acme", text, "c1"),
        ),
        (
            DirectedGate::UnusableEndpointIdentity,
            proposal("[***]", "OWNS", "Acme", text, "c1"),
        ),
        (
            DirectedGate::EvidenceNotVerbatim,
            proposal("Ann", "OWNS", "Acme", "Ann sells Acme.", "c1"),
        ),
        (
            DirectedGate::SourceNotInSentence,
            proposal("Bob", "OWNS", "Acme", text, "c1"),
        ),
        (
            DirectedGate::TargetNotInSentence,
            proposal("Ann", "OWNS", "Zed", text, "c1"),
        ),
    ];
    for (gate, p) in cases {
        let refusals = refuse(text, p);
        assert_eq!(refusals.len(), 1, "{gate:?}: {refusals:?}");
        assert_eq!(refusals[0].gate, gate, "{:?}", refusals[0]);
        assert_eq!(refusals[0].gate.code(), gate.code());
        assert!(!refusals[0].reason.is_empty());
    }
    // Vocabulary gate: everything else passes, but "owns" is never affirmed.
    let text = "Ann acquires Acme.";
    let refusals = refuse(text, proposal("Ann", "OWNS", "Acme", text, "c1"));
    assert_eq!(refusals.len(), 1);
    assert_eq!(refusals[0].gate, DirectedGate::VocabularyNotAffirmed);
}

#[test]
fn gate_codes_are_stable_snake_case_identifiers() {
    for gate in [
        DirectedGate::RelationNotInTaxonomy,
        DirectedGate::ChunkNotInRequest,
        DirectedGate::EmptyEndpoint,
        DirectedGate::UnusableEndpointIdentity,
        DirectedGate::EvidenceNotVerbatim,
        DirectedGate::SourceNotInSentence,
        DirectedGate::TargetNotInSentence,
        DirectedGate::VocabularyNotAffirmed,
    ] {
        let code = gate.code();
        assert!(
            !code.is_empty() && code.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "{code}"
        );
    }
    assert_eq!(
        DirectedGate::RelationNotInTaxonomy.code(),
        "relation_not_in_taxonomy"
    );
}

#[test]
fn reason_text_is_unchanged_from_the_legacy_skip_strings() {
    let text = "Ann owns Acme.";
    let r = refuse(text, proposal("Ann", "SELLS", "Acme", text, "c1"));
    assert_eq!(
        r[0].reason,
        "`Ann --SELLS--> Acme`: relation not in the taxonomy — dropped"
    );
    let r = refuse(text, proposal("Ann", "OWNS", "Acme", text, "missing"));
    assert_eq!(
        r[0].reason,
        "`Ann --OWNS--> Acme`: cites chunk `missing` which is not in this request — dropped"
    );
}

#[test]
fn refusal_carries_the_proposal_as_submitted_in_canonical_form() {
    // Decomposed é in the proposal; the chunk holds the composed form. The
    // refusal records NFC text, the same representation the writer uses.
    let text = "Ann owns Acm\u{e9}.";
    let r = refuse(
        text,
        proposal("Bob", "OWNS", "Acme\u{301}", "Ann owns Acme\u{301}.", "c1"),
    );
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].gate, DirectedGate::SourceNotInSentence);
    assert_eq!(r[0].source, "Bob");
    assert_eq!(r[0].relation, "OWNS");
    assert_eq!(r[0].target, "Acm\u{e9}");
    assert_eq!(r[0].evidence, "Ann owns Acm\u{e9}.");
    assert_eq!(r[0].chunk_id, "c1");
}

#[test]
fn empty_and_whitespace_only_fields_are_recorded_verbatim() {
    let text = "Ann owns Acme.";
    let r = refuse(text, proposal("   ", "OWNS", "Acme", text, "c1"));
    assert_eq!(r[0].gate, DirectedGate::EmptyEndpoint);
    assert_eq!(r[0].source, "   ");
    let r = refuse(text, proposal("Ann", "", "Acme", text, "c1"));
    assert_eq!(r[0].gate, DirectedGate::RelationNotInTaxonomy);
    assert_eq!(r[0].relation, "");
    let r = refuse(text, proposal("Ann", "OWNS", "Acme", "", "c1"));
    assert_eq!(r[0].gate, DirectedGate::EvidenceNotVerbatim, "{r:?}");
}

#[test]
fn duplicate_grounded_proposals_produce_one_fact_and_no_refusal() {
    let text = "Ann owns Acme.";
    let p = proposal("Ann", "OWNS", "Acme", text, "c1");
    let (facts, _, refusals) =
        gate_directed_proposals(&[chunk("c1", text)], &taxonomy(), &[p.clone(), p], "test");
    assert_eq!(facts.len(), 1);
    assert!(refusals.is_empty(), "{refusals:?}");
}

#[test]
fn refusals_preserve_proposal_order_and_repeat_identical_refusals() {
    let text = "Ann owns Acme.";
    let bad = proposal("Ann", "SELLS", "Acme", text, "c1");
    let worse = proposal("Ann", "OWNS", "Zed", text, "c1");
    let (_, _, refusals) = gate_directed_proposals(
        &[chunk("c1", text)],
        &taxonomy(),
        &[bad.clone(), worse, bad],
        "test",
    );
    let gates: Vec<_> = refusals.iter().map(|r| r.gate).collect();
    assert_eq!(
        gates,
        [
            DirectedGate::RelationNotInTaxonomy,
            DirectedGate::TargetNotInSentence,
            DirectedGate::RelationNotInTaxonomy
        ]
    );
}
