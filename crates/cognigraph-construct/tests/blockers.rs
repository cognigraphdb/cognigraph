//! relation_blocker acceptance: manufacture a real restraint violation
//! (allegation wording that legitimately contains a trigger), measure it,
//! veto it with an accepted blocker, and prove the violation closes WITH
//! RECALL HELD — the forbidden-fact contract and the blocker mechanism as
//! a pair, per docs/decisions/decision_relation_blocker.md.

use cognigraph_construct::*;
use cognigraph_native::NativeBackend;

/// The pharma scenario from the research direction doc: Novapharm's supply
/// is asserted outright; Meridian's only "evidence" is investigation
/// wording that contains the trigger phrase but does not assert the fact.
fn space() -> SpaceType {
    serde_json::from_value(serde_json::json!({
        "id": "pharma-allegations",
        "entities": [
            {"name": "Novapharm", "type": "organization", "aliases": []},
            {"name": "Meridian", "type": "organization", "aliases": []},
            {"name": "Compound X", "type": "compound", "aliases": ["compound x"]}
        ],
        "relation_rules": [
            {"source": "Novapharm", "relation": "SUPPLIES", "target": "Compound X",
             "when_any": ["novapharm supplies compound x"]},
            {"source": "Meridian", "relation": "SUPPLIES", "target": "Compound X",
             "when_any": ["meridian supplies compound x", "meridian supplying compound x"]}
        ]
    }))
    .unwrap()
}

fn chunks() -> Vec<Chunk> {
    vec![
        Chunk {
            id: "p-001".into(),
            title: "supply agreement".into(),
            text: "Under the 2024 agreement, Novapharm supplies Compound X to hospital \
                   networks across Europe."
                .into(),
        },
        Chunk {
            id: "p-002".into(),
            title: "regulatory probe".into(),
            text: "Regulators confirmed Meridian is under investigation over claims of \
                   Meridian supplying Compound X to unlicensed distributors."
                .into(),
        },
    ]
}

fn eval_spec() -> EvalSpec {
    serde_json::from_value(serde_json::json!({
        "space_id": "pharma-allegations",
        "questions": [{
            "id": "q1",
            "question": "Who supplies Compound X?",
            "expected_facts": ["Novapharm --SUPPLIES--> Compound X"],
            "forbidden_facts": ["Meridian --SUPPLIES--> Compound X"]
        }]
    }))
    .unwrap()
}

fn blocker(status: NeuronStatus) -> Neuron {
    serde_json::from_value::<Neuron>(serde_json::json!({
        "id": "meridian-allegation-blocker",
        "type": "relation_blocker",
        "confidence": 0.9,
        "rationale": "Investigation wording contains the trigger but asserts an allegation, not a supply relationship.",
        "evidence": ["Meridian is under investigation over claims of Meridian supplying Compound X"],
        "source": "Meridian",
        "relation": "SUPPLIES",
        "target": "Compound X",
        "when_any": ["under investigation", "over claims of"]
    }))
    .map(|mut n| {
        n.status = status;
        n
    })
    .unwrap()
}

async fn run(neurons: &[Neuron]) -> EvalOutcome {
    let space = space();
    let spec = eval_spec();
    let backend = NativeBackend::new();
    ingest_chunks(
        &backend,
        &spec.space_id,
        &effective_config(&space, neurons),
        &chunks(),
        &effective_vetoes(neurons),
    )
    .await
    .unwrap();
    evaluate(&backend, &spec.space_id, &spec).await.unwrap()
}

#[tokio::test]
async fn accepted_blocker_closes_violation_with_recall_held() {
    // 1. Measure: the allegation wording legitimately affirms the trigger
    //    (no negation cue — this is exactly what a blocker is for, and what
    //    the Case Bravo matcher fix rightly does NOT catch).
    let before = run(&[]).await;
    assert_eq!(before.forbidden_triggered, 1, "violation must be measured");
    assert!(before.recall_ok(), "Novapharm fact grounds from p-001");

    // 2. Proposed blocker: inert — the governance boundary.
    let proposed = [blocker(NeuronStatus::Proposed)];
    let inert = run(&proposed).await;
    assert_eq!(inert.forbidden_triggered, 1, "proposed blockers do nothing");

    // 3. Accepted blocker: violation closes, recall held.
    let accepted = [blocker(NeuronStatus::Accepted)];
    validate_neurons(
        &NeuronSet {
            space_type: "pharma-allegations".into(),
            neurons: accepted.to_vec(),
        },
        &space(),
    )
    .unwrap();
    let after = run(&accepted).await;
    assert_eq!(after.forbidden_triggered, 0, "veto suppresses p-002");
    assert!(after.recall_ok(), "recall held: {:?}", after.missing);
}

#[tokio::test]
async fn blocker_report_attributes_the_suppression() {
    let accepted = [blocker(NeuronStatus::Accepted)];
    let forbidden = [Fact::parse("Meridian --SUPPLIES--> Compound X").unwrap()];
    let report = blocker_report(&space(), &accepted, &chunks(), &forbidden);
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].status, BlockerStatus::Suppressed);
    assert_eq!(report[0].attributed_to, ["meridian-allegation-blocker"]);

    // And the recall side shows no collateral damage.
    let expected = [Fact::parse("Novapharm --SUPPLIES--> Compound X").unwrap()];
    let ablation = ablation_report(&space(), &accepted, &chunks(), &expected);
    assert!(ablation.iter().all(|e| e.status == AblationStatus::Base));
}

#[tokio::test]
async fn veto_is_chunk_local_not_graph_global() {
    // A veto only suppresses grounding from chunks containing its wording.
    // A clean assertion elsewhere still grounds the fact — restraint comes
    // from evidence discipline, not from erasing triples.
    let mut extended = chunks();
    extended.push(Chunk {
        id: "p-003".into(),
        title: "signed contract".into(),
        text: "The signed distribution contract states Meridian supplies Compound X.".into(),
    });
    let space = space();
    let spec = eval_spec();
    let backend = NativeBackend::new();
    ingest_chunks(
        &backend,
        &spec.space_id,
        &space,
        &extended,
        &effective_vetoes(&[blocker(NeuronStatus::Accepted)]),
    )
    .await
    .unwrap();
    let outcome = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    assert_eq!(
        outcome.forbidden_triggered, 1,
        "the fact grounds from the clean chunk despite the veto on p-002"
    );
}

#[tokio::test]
async fn validation_rejects_accepted_hint_blocker_pair_on_same_triple() {
    let mut hint: Neuron = serde_json::from_value(serde_json::json!({
        "id": "meridian-supply-hint",
        "type": "relation_hint",
        "confidence": 0.9,
        "evidence": ["contract text"],
        "source": "Meridian",
        "relation": "SUPPLIES",
        "target": "Compound X",
        "triggers": ["meridian supplies compound x"]
    }))
    .unwrap();
    hint.status = NeuronStatus::Accepted;
    let set = NeuronSet {
        space_type: "pharma-allegations".into(),
        neurons: vec![hint.clone(), blocker(NeuronStatus::Accepted)],
    };
    let err = validate_neurons(&set, &space()).unwrap_err();
    assert!(matches!(
        err,
        validate::NeuronError::HintBlockerConflict(_, _)
    ));

    // The pair is legal while the blocker is merely proposed — review
    // resolves it before both can be accepted.
    let set = NeuronSet {
        space_type: "pharma-allegations".into(),
        neurons: vec![hint, blocker(NeuronStatus::Proposed)],
    };
    validate_neurons(&set, &space()).unwrap();
}

#[tokio::test]
async fn blocker_validation_is_ontology_bounded() {
    let make = |mutate: fn(&mut Neuron)| {
        let mut n = blocker(NeuronStatus::Proposed);
        mutate(&mut n);
        NeuronSet {
            space_type: "pharma-allegations".into(),
            neurons: vec![n],
        }
    };
    let space = space();
    // Unknown relation, unknown entity, and empty vetoes are all rejected.
    assert!(validate_neurons(&make(|n| n.relation = "INVENTED".into()), &space).is_err());
    assert!(validate_neurons(&make(|n| n.source = "Ghost Corp".into()), &space).is_err());
    assert!(validate_neurons(&make(|n| n.triggers.clear()), &space).is_err());
}
