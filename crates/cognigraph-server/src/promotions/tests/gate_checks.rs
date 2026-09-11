//! Gate checks.

use super::*;

#[test]
fn checked_operator_fixture_is_a_valid_promotion_evaluation_job() {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Fixture {
        kind: JobKind,
        input: FixtureInput,
    }

    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct FixtureInput {
        space_type: String,
        eval: EvalSpec,
        promotion_context: PromotionContext,
    }

    let fixture: Fixture = serde_json::from_str(include_str!(
        "../../../../../fixtures/m18/promotion-evaluation.json"
    ))
    .expect("M18 operator fixture must remain valid JSON and match the authoring contract");
    assert_eq!(fixture.kind, JobKind::ConstructEvaluate);

    let expected = fixture
        .input
        .eval
        .questions
        .iter()
        .flat_map(|question| &question.expected_facts)
        .map(|fact| cognigraph_construct::Fact::parse(fact).expect("valid expected fact"))
        .collect::<std::collections::HashSet<_>>();
    let forbidden = fixture
        .input
        .eval
        .questions
        .iter()
        .flat_map(|question| &question.forbidden_facts)
        .map(|fact| cognigraph_construct::Fact::parse(fact).expect("valid forbidden fact"))
        .collect::<std::collections::HashSet<_>>();
    assert!(expected.is_disjoint(&forbidden));

    let eval_digest = canonical_digest(&fixture.input.eval).expect("EvalSpec digest");
    assert_eq!(
        eval_digest,
        "sha256:178129f9f1694d9dd2ba31835c5059a4778446a0512f2e6f3147bd5814728bbd"
    );
    fixture
        .input
        .promotion_context
        .validate(
            &fixture.input.space_type,
            &eval_digest,
            expected.len(),
            forbidden.len(),
        )
        .expect("checked PromotionContext");
    fixture
        .input
        .promotion_context
        .validate_runtime_backend("native-redb")
        .expect("fixture backend matches the native runtime family");
    fixture
        .input
        .promotion_context
        .oracle_separation
        .validate_for_policy(&fixture.input.promotion_context.policy.oracle)
        .expect("checked oracle attestation must pass the pinned policy");
}
#[test]
fn recall_and_restraint_gates_are_independent() {
    let baseline_original = source("baseline", "baseline-original", 1, 0);
    let baseline_replay = source("baseline", "baseline-replay", 1, 0);

    let recall_original = source("candidate-recall", "candidate-original", 0, 0);
    let recall_replay = source("candidate-recall", "candidate-replay", 0, 0);
    let recall = assess_evidence_runs(
        &recall_original,
        &recall_replay,
        &baseline_original,
        &baseline_replay,
    )
    .unwrap();
    assert!(!recall.recall.passed);
    assert!(recall.restraint.passed);
    assert!(!recall.overall_passed);

    let restraint_original = source("candidate-restraint", "candidate-original", 1, 1);
    let restraint_replay = source("candidate-restraint", "candidate-replay", 1, 1);
    let restraint = assess_evidence_runs(
        &restraint_original,
        &restraint_replay,
        &baseline_original,
        &baseline_replay,
    )
    .unwrap();
    assert!(restraint.recall.passed);
    assert!(!restraint.restraint.passed);
    assert!(!restraint.overall_passed);

    let mut oracle_original = source("candidate-oracle", "oracle-original", 1, 0);
    let mut oracle_replay = source("candidate-oracle", "oracle-replay", 1, 0);
    for run in [&mut oracle_original, &mut oracle_replay] {
        run.context.oracle_separation.status = OracleAttestationStatus::Unverifiable;
        run.context.oracle_separation.attestation_digest =
            record_digest(&run.context.oracle_separation, "attestation_digest").unwrap();
        run.context_digest = run.context.digest().unwrap();
    }
    let oracle = assess_evidence_runs(
        &oracle_original,
        &oracle_replay,
        &baseline_original,
        &baseline_replay,
    )
    .unwrap();
    assert!(!oracle.oracle_separation.passed);
    assert!(!oracle.overall_passed);
}
#[test]
fn exact_replay_and_baseline_comparability_gate_independently() {
    let candidate_original = source("candidate", "candidate-original", 1, 0);
    let candidate_replay = source("candidate", "candidate-replay", 1, 0);
    let baseline_original = source("baseline", "baseline-original", 1, 0);
    let baseline_replay = source("baseline", "baseline-replay", 1, 0);
    let passed = assess_evidence_runs(
        &candidate_original,
        &candidate_replay,
        &baseline_original,
        &baseline_replay,
    )
    .unwrap();
    assert!(passed.overall_passed);

    let mut changed_replay = candidate_replay.clone();
    changed_replay.result_digest = digest("different-result");
    let replay_failure = assess_evidence_runs(
        &candidate_original,
        &changed_replay,
        &baseline_original,
        &baseline_replay,
    )
    .unwrap();
    assert!(!replay_failure.pair_reproducibility.passed);
    assert!(replay_failure.baseline_comparability.passed);

    let mut incomparable_original = baseline_original.clone();
    let mut incomparable_replay = baseline_replay.clone();
    for run in [&mut incomparable_original, &mut incomparable_replay] {
        run.context.effective_configuration.evaluation_config_digest =
            digest("incomparable-evaluation-config");
        run.context_digest = run.context.digest().unwrap();
    }
    let comparability_failure = assess_evidence_runs(
        &candidate_original,
        &candidate_replay,
        &incomparable_original,
        &incomparable_replay,
    )
    .unwrap();
    assert!(comparability_failure.pair_reproducibility.passed);
    assert!(!comparability_failure.baseline_comparability.passed);

    let mut unauthorized_original = candidate_original.clone();
    let mut unauthorized_replay = candidate_replay.clone();
    for run in [&mut unauthorized_original, &mut unauthorized_replay] {
        run.context
            .policy
            .allowed_candidate_differences
            .retain(|difference| *difference != CandidateDifference::ConstructionConfiguration);
        run.context_digest = run.context.digest().unwrap();
    }
    let mut policy_matched_baseline_original = baseline_original.clone();
    let mut policy_matched_baseline_replay = baseline_replay.clone();
    for run in [
        &mut policy_matched_baseline_original,
        &mut policy_matched_baseline_replay,
    ] {
        run.context.policy = unauthorized_original.context.policy.clone();
        run.context_digest = run.context.digest().unwrap();
    }
    let unauthorized = assess_evidence_runs(
        &unauthorized_original,
        &unauthorized_replay,
        &policy_matched_baseline_original,
        &policy_matched_baseline_replay,
    )
    .unwrap();
    assert!(!unauthorized.baseline_comparability.passed);
}
