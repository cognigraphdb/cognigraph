//! Graph validation.

use super::*;

pub(super) fn validate_graph_facts(facts: &[VerifiedGraphFact]) -> Result<(), CogniGraphError> {
    let mut previous: Option<(&str, &str, &str, &str)> = None;
    let mut unique = HashSet::new();
    for fact in facts {
        for (label, value) in [
            ("source", &fact.source),
            ("relation", &fact.relation),
            ("target", &fact.target),
            ("evidence_chunk_id", &fact.evidence_chunk_id),
        ] {
            validate_text(&format!("graph.fact.{label}"), value)?;
        }
        let key = (
            fact.source.as_str(),
            fact.relation.as_str(),
            fact.target.as_str(),
            fact.evidence_chunk_id.as_str(),
        );
        if previous.is_some_and(|previous| previous >= key) || !unique.insert(key) {
            return Err(validation(
                "evaluation graph facts must be sorted and unique",
            ));
        }
        previous = Some(key);
    }
    Ok(())
}
pub(super) fn require_canonical_json<T: Serialize>(
    label: &str,
    raw: &[u8],
    value: &T,
) -> Result<Vec<u8>, CogniGraphError> {
    let canonical = cognigraph_governance::canonical_json_bytes(value)
        .map_err(|error| validation(format!("{label} cannot be canonicalized: {error}")))?;
    if raw != canonical {
        return Err(validation(format!(
            "{label} bytes are not exact canonical JSON"
        )));
    }
    Ok(canonical)
}
pub(super) fn validate_eval_spec_text(eval: &EvalSpec) -> Result<(), CogniGraphError> {
    validate_text("oracle.eval_spec.space_id", &eval.space_id)?;
    for question in &eval.questions {
        validate_text("oracle.eval_spec.question.id", &question.id)?;
        validate_text("oracle.eval_spec.question.question", &question.question)?;
        for fact in question
            .expected_facts
            .iter()
            .chain(&question.forbidden_facts)
        {
            validate_text("oracle.eval_spec.question.fact", fact)?;
        }
    }
    Ok(())
}
pub(super) fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}
