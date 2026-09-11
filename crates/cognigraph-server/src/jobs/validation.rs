//! Validation.

use super::*;

/// Promotion-grade evaluation is deliberately stricter than the legacy
/// diagnostic evaluator. The latter keeps accepting old empty/malformed
/// fixtures for compatibility; an M18 context may only freeze a non-empty,
/// unambiguous distinct-fact union.
pub(super) fn validate_promotion_eval_spec(
    eval: &EvalSpec,
) -> Result<(usize, usize), CogniGraphError> {
    if eval.space_id.trim().is_empty() {
        return Err(CogniGraphError::ValidationError(
            "promotion evaluation space_id is empty".into(),
        ));
    }
    if eval.questions.is_empty() {
        return Err(CogniGraphError::ValidationError(
            "promotion evaluation requires at least one question".into(),
        ));
    }

    let mut question_ids = std::collections::HashSet::new();
    let mut expected = std::collections::HashSet::new();
    let mut forbidden = std::collections::HashSet::new();
    for question in &eval.questions {
        let id = question.id.trim();
        if id.is_empty() || !question_ids.insert(id.to_string()) {
            return Err(CogniGraphError::ValidationError(
                "promotion evaluation question ids must be non-empty and unique".into(),
            ));
        }
        for (kind, raw, output) in [
            ("expected", &question.expected_facts, &mut expected),
            ("forbidden", &question.forbidden_facts, &mut forbidden),
        ] {
            for fact in raw {
                let parsed = cognigraph_construct::Fact::parse(fact).filter(|parsed| {
                    !parsed.source.trim().is_empty()
                        && !parsed.relation.trim().is_empty()
                        && !parsed.target.trim().is_empty()
                });
                let Some(parsed) = parsed else {
                    return Err(CogniGraphError::ValidationError(format!(
                        "promotion evaluation question `{id}` has malformed {kind} fact `{fact}`"
                    )));
                };
                output.insert(parsed);
            }
        }
    }
    if expected.is_empty() || forbidden.is_empty() {
        return Err(CogniGraphError::ValidationError(
            "promotion evaluation requires positive expected and forbidden denominators".into(),
        ));
    }
    if let Some(overlap) = expected.intersection(&forbidden).next() {
        return Err(CogniGraphError::ValidationError(format!(
            "promotion evaluation fact appears in both expected and forbidden sets: {} --{}--> {}",
            overlap.source, overlap.relation, overlap.target
        )));
    }
    Ok((expected.len(), forbidden.len()))
}
pub(super) fn fact_lines(facts: &[cognigraph_construct::Fact]) -> Vec<String> {
    facts
        .iter()
        .map(|fact| format!("{} --{}--> {}", fact.source, fact.relation, fact.target))
        .collect()
}
pub(super) fn validate_idempotency_key(key: &str) -> Result<(), CogniGraphError> {
    if key.is_empty() || key.len() > 128 || !key.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(CogniGraphError::ValidationError(
            "Idempotency-Key must be 1-128 printable ASCII characters".into(),
        ));
    }
    Ok(())
}
pub(super) fn validate_reason(reason: Option<&str>) -> Result<(), CogniGraphError> {
    if reason.is_some_and(|reason| reason.len() > MAX_OPERATOR_REASON_BYTES) {
        return Err(CogniGraphError::ValidationError(format!(
            "operator reason exceeds {MAX_OPERATOR_REASON_BYTES} bytes"
        )));
    }
    Ok(())
}
pub(super) fn digest_json(value: &Value) -> Result<String, CogniGraphError> {
    let canonical = canonical_json(value);
    Ok(digest_bytes(&serde_json::to_vec(&canonical)?))
}
pub(super) fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut fields = map.iter().collect::<Vec<_>>();
            fields.sort_by_key(|(left, _)| *left);
            let mut result = serde_json::Map::new();
            for (key, value) in fields {
                result.insert(key.clone(), canonical_json(value));
            }
            Value::Object(result)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}
pub(super) fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        })
}
pub(super) fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
