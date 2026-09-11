//! Assessment.

use super::*;

/// Pure evidence assessment. Structural invalidity returns an error; genuine
/// promotion-gate failures are retained as independent failed gate results.
pub fn assess_evidence_runs(
    candidate_original: &PromotionEvaluationSource,
    candidate_replay: &PromotionEvaluationSource,
    baseline_original: &PromotionEvaluationSource,
    baseline_replay: &PromotionEvaluationSource,
) -> Result<PromotionGateAssessment, CogniGraphError> {
    for run in [
        candidate_original,
        candidate_replay,
        baseline_original,
        baseline_replay,
    ] {
        run.validate()?;
    }
    let policy = &candidate_original.context.policy;
    if [candidate_replay, baseline_original, baseline_replay]
        .iter()
        .any(|run| run.context.policy != *policy)
    {
        return Err(validation(
            "all four evaluation jobs must carry the identical frozen policy",
        ));
    }
    let governance = &candidate_original.context.governance;
    if [candidate_replay, baseline_original, baseline_replay]
        .iter()
        .any(|run| run.context.governance != *governance)
    {
        return Err(validation(
            "all four evaluation jobs must carry the identical frozen policy governance binding",
        ));
    }
    if [candidate_replay, baseline_original, baseline_replay]
        .iter()
        .any(|run| run.context.schema_version != candidate_original.context.schema_version)
    {
        return Err(validation(
            "all four evaluation jobs must use the same promotion authority generation",
        ));
    }
    if candidate_original.context.artifact_attestations
        != candidate_replay.context.artifact_attestations
        || baseline_original.context.artifact_attestations
            != baseline_replay.context.artifact_attestations
    {
        return Err(validation(
            "each original/replay pair must carry identical artifact attestations",
        ));
    }

    let mut replay_reasons = Vec::new();
    compare_pair(
        "candidate",
        candidate_original,
        candidate_replay,
        &mut replay_reasons,
    );
    compare_pair(
        "baseline",
        baseline_original,
        baseline_replay,
        &mut replay_reasons,
    );

    let mut comparability_reasons = Vec::new();
    for (label, candidate, baseline) in [
        ("original", candidate_original, baseline_original),
        ("replay", candidate_replay, baseline_replay),
    ] {
        compare_candidate_baseline(label, candidate, baseline, &mut comparability_reasons);
    }
    if candidate_original.context.candidate.candidate_digest
        == baseline_original.context.candidate.candidate_digest
    {
        comparability_reasons.push("candidate digest is identical to baseline".into());
    }

    let mut denominator_reasons = Vec::new();
    for (label, run) in [
        ("candidate original", candidate_original),
        ("candidate replay", candidate_replay),
    ] {
        if run.recall.total < policy.recall.min_expected_distinct {
            denominator_reasons.push(format!(
                "{label} recall denominator {} is below {}",
                run.recall.total, policy.recall.min_expected_distinct
            ));
        }
        if run.restraint.total < policy.restraint.min_forbidden_distinct {
            denominator_reasons.push(format!(
                "{label} restraint denominator {} is below {}",
                run.restraint.total, policy.restraint.min_forbidden_distinct
            ));
        }
    }
    if candidate_original.recall.total != baseline_original.recall.total
        || candidate_replay.recall.total != baseline_replay.recall.total
        || candidate_original.restraint.total != baseline_original.restraint.total
        || candidate_replay.restraint.total != baseline_replay.restraint.total
    {
        denominator_reasons.push("candidate and baseline denominators differ".into());
    }

    let mut recall_reasons = Vec::new();
    for (label, candidate, baseline) in [
        ("original", candidate_original, baseline_original),
        ("replay", candidate_replay, baseline_replay),
    ] {
        let missing = candidate.recall.total - candidate.recall.found;
        let baseline_missing = baseline.recall.total - baseline.recall.found;
        if !ratio_at_least(
            candidate.recall.found,
            candidate.recall.total,
            policy.recall.min_ratio_numerator,
            policy.recall.min_ratio_denominator,
        ) {
            recall_reasons.push(format!("candidate {label} recall ratio is below policy"));
        }
        if missing > policy.recall.max_missing {
            recall_reasons.push(format!(
                "candidate {label} missing count {missing} exceeds {}",
                policy.recall.max_missing
            ));
        }
        if missing
            > baseline_missing.saturating_add(policy.recall.max_additional_missing_vs_baseline)
        {
            recall_reasons.push(format!(
                "candidate {label} recall regresses beyond the baseline allowance"
            ));
        }
    }

    let mut restraint_reasons = Vec::new();
    for (label, candidate, baseline) in [
        ("original", candidate_original, baseline_original),
        ("replay", candidate_replay, baseline_replay),
    ] {
        let safe = candidate
            .restraint
            .total
            .saturating_sub(candidate.restraint.violations);
        if !ratio_at_least(
            safe,
            candidate.restraint.total,
            policy.restraint.min_ratio_numerator,
            policy.restraint.min_ratio_denominator,
        ) {
            restraint_reasons.push(format!("candidate {label} restraint ratio is below policy"));
        }
        if candidate.restraint.violations > policy.restraint.max_violations {
            restraint_reasons.push(format!(
                "candidate {label} violations {} exceeds {}",
                candidate.restraint.violations, policy.restraint.max_violations
            ));
        }
        if candidate.restraint.violations
            > baseline
                .restraint
                .violations
                .saturating_add(policy.restraint.max_additional_violations_vs_baseline)
        {
            restraint_reasons.push(format!(
                "candidate {label} restraint regresses beyond the baseline allowance"
            ));
        }
    }

    let mut oracle_reasons = Vec::new();
    for (label, run) in [
        ("candidate original", candidate_original),
        ("candidate replay", candidate_replay),
        ("baseline original", baseline_original),
        ("baseline replay", baseline_replay),
    ] {
        if let Err(error) = run
            .context
            .oracle_separation
            .validate_for_policy(&run.context.policy.oracle)
        {
            oracle_reasons.push(format!("{label}: {error}"));
        }
    }

    Ok(PromotionGateAssessment {
        pair_reproducibility: gate(replay_reasons),
        baseline_comparability: gate(comparability_reasons),
        denominators: gate(denominator_reasons),
        recall: gate(recall_reasons),
        restraint: gate(restraint_reasons),
        oracle_separation: gate(oracle_reasons),
        overall_passed: false,
    }
    .finalize())
}
