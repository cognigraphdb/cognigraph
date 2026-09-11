//! Comparison.

use super::*;

pub(super) fn compare_pair(
    label: &str,
    original: &PromotionEvaluationSource,
    replay: &PromotionEvaluationSource,
    reasons: &mut Vec<String>,
) {
    let result_equivalent = match (&original.artifact_consumption, &replay.artifact_consumption) {
        (Some(original), Some(replay)) => original.material_digest == replay.material_digest,
        (None, None) => original.result_digest == replay.result_digest,
        _ => false,
    };
    if original.context_digest != replay.context_digest
        || original.execution_payload_digest != replay.execution_payload_digest
        || !result_equivalent
        || original.recall != replay.recall
        || original.restraint != replay.restraint
        || original.missing != replay.missing
        || original.violations != replay.violations
    {
        reasons.push(format!(
            "{label} original and independent replay do not reproduce the same verified material and result"
        ));
    }
}
pub(super) fn compare_candidate_baseline(
    label: &str,
    candidate: &PromotionEvaluationSource,
    baseline: &PromotionEvaluationSource,
    reasons: &mut Vec<String>,
) {
    let candidate_context = &candidate.context;
    let baseline_context = &baseline.context;
    let policy = &candidate_context.policy;
    let comparable = candidate_context.target == baseline_context.target
        && (candidate_context.candidate == baseline_context.candidate
            || policy
                .allowed_candidate_differences
                .contains(&CandidateDifference::CandidateIdentity))
        && (candidate_context
            .effective_configuration
            .construction_config_digest
            == baseline_context
                .effective_configuration
                .construction_config_digest
            || policy
                .allowed_candidate_differences
                .contains(&CandidateDifference::ConstructionConfiguration))
        && candidate_context
            .effective_configuration
            .evaluation_config_digest
            == baseline_context
                .effective_configuration
                .evaluation_config_digest
        && candidate_context.effective_configuration.eval_spec_digest
            == baseline_context.effective_configuration.eval_spec_digest
        && candidate_context
            .effective_configuration
            .preprocessing_digest
            == baseline_context
                .effective_configuration
                .preprocessing_digest
        && candidate_context.effective_configuration.scorer_id
            == baseline_context.effective_configuration.scorer_id
        && candidate_context.effective_configuration.scorer_version
            == baseline_context.effective_configuration.scorer_version
        && candidate_context
            .effective_configuration
            .scorer_artifact_digest
            == baseline_context
                .effective_configuration
                .scorer_artifact_digest
        && candidate_context.revisions.corpus == baseline_context.revisions.corpus
        && candidate_context.revisions.oracle == baseline_context.revisions.oracle
        && (candidate_context.revisions.graph == baseline_context.revisions.graph
            || policy
                .allowed_candidate_differences
                .contains(&CandidateDifference::GraphRevision))
        && candidate_context.revisions.graph.kind == baseline_context.revisions.graph.kind
        && candidate_context.revisions.graph.source_lineage
            == baseline_context.revisions.graph.source_lineage
        && candidate_context.revisions.graph.immutability_method
            == baseline_context.revisions.graph.immutability_method
        && candidate_context.revisions.graph.issuer == baseline_context.revisions.graph.issuer
        && candidate_context.case_manifest == baseline_context.case_manifest
        && candidate_context.policy == baseline_context.policy
        && (candidate_context.reproducibility.resolved_config_digest
            == baseline_context.reproducibility.resolved_config_digest
            || policy
                .allowed_candidate_differences
                .contains(&CandidateDifference::ResolvedConfiguration))
        && same_reproducibility_environment(
            &candidate_context.reproducibility,
            &baseline_context.reproducibility,
        )
        && candidate_context.consumption_plan == baseline_context.consumption_plan
        && comparable_artifact_attestations(candidate_context, baseline_context, policy);
    if !comparable {
        reasons.push(format!(
            "candidate and baseline {label} contexts are not comparable"
        ));
    }
}
pub(super) fn comparable_artifact_attestations(
    candidate: &PromotionContext,
    baseline: &PromotionContext,
    policy: &ResolvedPromotionPolicy,
) -> bool {
    match (
        &candidate.artifact_attestations,
        &baseline.artifact_attestations,
    ) {
        (None, None) => true,
        (Some(candidate), Some(baseline)) => {
            candidate.corpus == baseline.corpus
                && candidate.oracle == baseline.oracle
                && candidate.scorer == baseline.scorer
                && candidate.verifier == baseline.verifier
                && if candidate.graph == baseline.graph {
                    true
                } else {
                    policy
                        .allowed_candidate_differences
                        .contains(&CandidateDifference::GraphRevision)
                        && candidate.graph.subject_digest != baseline.graph.subject_digest
                }
        }
        _ => false,
    }
}
pub(super) fn same_reproducibility_environment(
    candidate: &ReproducibilityContext,
    baseline: &ReproducibilityContext,
) -> bool {
    candidate.deterministic == baseline.deterministic
        && candidate.source_commit == baseline.source_commit
        && candidate.source_tree_digest == baseline.source_tree_digest
        && candidate.dirty_tree == baseline.dirty_tree
        && candidate.executable_digest == baseline.executable_digest
        && candidate.toolchain == baseline.toolchain
        && candidate.target == baseline.target
        && candidate.backend == baseline.backend
        && candidate.backend_version == baseline.backend_version
        && candidate.command_digest == baseline.command_digest
        && candidate.seed == baseline.seed
        && candidate.provider == baseline.provider
        && candidate.model == baseline.model
}
pub(super) fn same_construction_identity(
    left: &PromotionContext,
    right: &PromotionContext,
) -> bool {
    left.candidate == right.candidate
        && left.effective_configuration.construction_config_digest
            == right.effective_configuration.construction_config_digest
        && left.revisions.graph == right.revisions.graph
        && left.revisions.corpus == right.revisions.corpus
        && left.artifact_attestations == right.artifact_attestations
        && left.consumption_plan == right.consumption_plan
}
pub(super) fn gate(reasons: Vec<String>) -> GateResult {
    GateResult {
        passed: reasons.is_empty(),
        reasons,
    }
}
pub(super) fn ratio_at_least(found: u64, total: u64, numerator: u64, denominator: u64) -> bool {
    total > 0
        && u128::from(found) * u128::from(denominator) >= u128::from(total) * u128::from(numerator)
}
