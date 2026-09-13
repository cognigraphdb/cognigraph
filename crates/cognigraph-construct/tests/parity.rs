//! Parity with the research repo's evaluation results (the M14 acceptance
//! test): SOTU recall with accepted neurons, case:alpha and study:px-101 on
//! the base ontology alone, and Case Bravo's negative probe — full recall
//! with zero forbidden facts constructed.

use cognigraph_construct::*;
use cognigraph_native::NativeBackend;
use std::path::Path;

fn fixtures() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/semantic-neurons"
    ))
}

fn load_space(name: &str) -> SpaceType {
    serde_json::from_str(
        &std::fs::read_to_string(fixtures().join(format!("space_types/{name}.json"))).unwrap(),
    )
    .unwrap()
}

fn load_eval(name: &str) -> EvalSpec {
    serde_json::from_str(
        &std::fs::read_to_string(fixtures().join(format!("evaluations/{name}.json"))).unwrap(),
    )
    .unwrap()
}

fn load_chunks(name: &str) -> Vec<Chunk> {
    std::fs::read_to_string(fixtures().join(format!("chunks/{name}.chunks.jsonl")))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn load_neurons(name: &str) -> NeuronSet {
    serde_json::from_str(
        &std::fs::read_to_string(fixtures().join(format!("neurons/{name}.neurons.json"))).unwrap(),
    )
    .unwrap()
}

async fn run_space(
    space_type: &str,
    eval_name: &str,
    chunks_name: &str,
    neurons: Option<&str>,
) -> EvalOutcome {
    let space = load_space(space_type);
    let (config, vetoes) = match neurons {
        Some(name) => {
            let set = load_neurons(name);
            validate_neurons(&set, &space).unwrap();
            (
                effective_config(&space, &set.neurons),
                effective_vetoes(&set.neurons),
            )
        }
        None => (space.clone(), Vec::new()),
    };
    let spec = load_eval(eval_name);
    let chunks = load_chunks(chunks_name);
    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &config, &chunks, &vetoes)
        .await
        .unwrap();
    evaluate(&backend, &spec.space_id, &spec).await.unwrap()
}

#[tokio::test]
async fn sotu_reaches_full_recall_with_accepted_neurons() {
    let outcome = run_space(
        "demo_policy_speech",
        "demo_sotu_2024",
        "state_of_the_union_2024",
        Some("demo_policy_speech"),
    )
    .await;
    assert!(
        outcome.recall_ok(),
        "missing: {:?} ({}/{})",
        outcome.missing,
        outcome.expected_found,
        outcome.expected_total
    );
    assert!(
        outcome.restraint_ok(),
        "violations: {:?}",
        outcome.violations
    );
}

#[tokio::test]
async fn forensics_and_pharma_pass_on_base_ontology() {
    let alpha = run_space(
        "digital_forensics",
        "forensics_case_alpha",
        "digital_forensics_case_alpha",
        None,
    )
    .await;
    assert!(alpha.recall_ok(), "case:alpha missing: {:?}", alpha.missing);

    let pharma = run_space(
        "pharma_research",
        "pharma_study_px101",
        "pharma_study_px101",
        None,
    )
    .await;
    assert!(pharma.recall_ok(), "px-101 missing: {:?}", pharma.missing);
}

#[tokio::test]
async fn case_bravo_negative_probe_full_recall_zero_forbidden() {
    let outcome = run_space(
        "digital_forensics",
        "forensics_case_bravo",
        "digital_forensics_case_bravo",
        None,
    )
    .await;
    assert!(
        outcome.recall_ok(),
        "bravo recall {}/{}, missing: {:?}",
        outcome.expected_found,
        outcome.expected_total,
        outcome.missing
    );
    assert_eq!(
        outcome.forbidden_triggered, 0,
        "restraint violations: {:?}",
        outcome.violations
    );
    assert!(
        outcome.forbidden_total >= 4,
        "bravo declares 4 forbidden facts"
    );
}

#[tokio::test]
async fn crowdstrike_accepted_neurons_close_the_generalization_loop() {
    // The generalization experiment's outcome, pinned: the four human-accepted
    // repairs take the untuned CrowdStrike article from 2/6 to full recall
    // with zero restraint violations. Deterministic — no LLM involved.
    let dir = fixtures().join("generalization");
    let space: SpaceType = serde_json::from_str(
        &std::fs::read_to_string(dir.join("crowdstrike_2024.space_type.json")).unwrap(),
    )
    .unwrap();
    let set: NeuronSet = serde_json::from_str(
        &std::fs::read_to_string(dir.join("crowdstrike_2024.neurons.accepted.json")).unwrap(),
    )
    .unwrap();
    validate_neurons(&set, &space).unwrap();
    let spec: EvalSpec = serde_json::from_str(
        &std::fs::read_to_string(dir.join("crowdstrike_2024.eval.json")).unwrap(),
    )
    .unwrap();
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("crowdstrike_2024.chunks.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let backend = NativeBackend::new();
    let repaired = effective_config(&space, &set.neurons);
    ingest_chunks(
        &backend,
        &spec.space_id,
        &repaired,
        &chunks,
        &effective_vetoes(&set.neurons),
    )
    .await
    .unwrap();
    let outcome = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    assert!(
        outcome.recall_ok(),
        "recall {}/{}, missing: {:?}",
        outcome.expected_found,
        outcome.expected_total,
        outcome.missing
    );
    assert_eq!(
        outcome.forbidden_triggered, 0,
        "restraint violations: {:?}",
        outcome.violations
    );
}

#[tokio::test]
async fn sotu_ablation_flags_graduated_neurons_as_redundant() {
    // The research's redundancy finding: the three accepted SOTU neurons'
    // facts are already grounded by the (graduated) base ontology.
    let space = load_space("demo_policy_speech");
    let set = load_neurons("demo_policy_speech");
    let chunks = load_chunks("state_of_the_union_2024");
    let spec = load_eval("demo_sotu_2024");
    let expected: Vec<Fact> = spec
        .questions
        .iter()
        .flat_map(|q| q.expected_facts.iter().filter_map(|f| Fact::parse(f)))
        .collect();
    let report = ablation_report(&space, &set.neurons, &chunks, &expected);
    assert!(report.iter().all(|e| e.status != AblationStatus::Missing));
    for entry in report.iter().filter(|e| !e.attributed_to.is_empty()) {
        assert_eq!(
            entry.status,
            AblationStatus::Base,
            "graduated neuron facts ground from base: {:?}",
            entry.fact
        );
    }
}

#[tokio::test]
async fn graduation_reproduces_the_research_redundancy_finding() {
    // The research found all three accepted SOTU neurons redundant against
    // the (graduated) base ontology — the leave-one-out report must agree.
    let space = load_space("demo_policy_speech");
    let set = load_neurons("demo_policy_speech");
    let chunks = load_chunks("state_of_the_union_2024");
    let report = graduation_report(&space, &set.neurons, &chunks);
    let accepted = set
        .neurons
        .iter()
        .filter(|n| n.status == NeuronStatus::Accepted)
        .count();
    assert_eq!(
        report.len(),
        accepted,
        "every accepted SOTU neuron graduates"
    );
    assert!(
        report
            .iter()
            .all(|c| c.reason == GraduationReason::CoveredByBase)
    );

    // The CrowdStrike repairs are NOT redundant: base recall was 2/6.
    let dir = fixtures().join("generalization");
    let space: SpaceType = serde_json::from_str(
        &std::fs::read_to_string(dir.join("crowdstrike_2024.space_type.json")).unwrap(),
    )
    .unwrap();
    let set: NeuronSet = serde_json::from_str(
        &std::fs::read_to_string(dir.join("crowdstrike_2024.neurons.accepted.json")).unwrap(),
    )
    .unwrap();
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("crowdstrike_2024.chunks.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let report = graduation_report(&space, &set.neurons, &chunks);
    assert!(
        report.is_empty(),
        "live repairs must not be flagged: {report:?}"
    );
}
