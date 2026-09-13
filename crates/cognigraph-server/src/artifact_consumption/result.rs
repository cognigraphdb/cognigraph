//! Result.

use super::*;

pub(super) fn evaluation_result_value(outcome: &EvalOutcome) -> serde_json::Value {
    let fact_lines = |facts: &[Fact]| {
        let mut lines = facts
            .iter()
            .map(|fact| format!("{} --{}--> {}", fact.source, fact.relation, fact.target))
            .collect::<Vec<_>>();
        lines.sort();
        lines
    };
    json!({
        "space_type": outcome.space_id,
        "recall": { "found": outcome.expected_found, "total": outcome.expected_total },
        "restraint": { "violations": outcome.forbidden_triggered, "total": outcome.forbidden_total },
        "recall_ok": outcome.recall_ok(),
        "restraint_ok": outcome.restraint_ok(),
        "missing": fact_lines(&outcome.missing),
        "violations": fact_lines(&outcome.violations),
    })
}
