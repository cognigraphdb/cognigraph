//! Record helpers.

use super::*;

pub(super) fn fact_map(
    facts: &[VerifiedGraphFact],
) -> Result<BTreeMap<FactKey, VerifiedGraphFact>, CogniGraphError> {
    let mut result = BTreeMap::new();
    for fact in facts {
        let key = (
            fact.source.clone(),
            fact.relation.clone(),
            fact.target.clone(),
            fact.evidence_chunk_id.clone(),
        );
        if result.insert(key, fact.clone()).is_some() {
            return Err(conflict("M26 semantic facts are not unique"));
        }
    }
    Ok(result)
}
pub(super) fn require_sorted_unique_keys<T: Serialize>(rows: &[T]) -> Result<(), CogniGraphError> {
    let mut previous: Option<String> = None;
    for row in rows {
        let key = serde_json::to_value(row)?
            .get("_key")
            .and_then(Value::as_str)
            .ok_or_else(|| conflict("M26 materialized row is missing a string `_key`"))?
            .to_string();
        if previous.as_ref().is_some_and(|previous| previous >= &key) {
            return Err(conflict(
                "M26 materialized row keys are not sorted and unique",
            ));
        }
        previous = Some(key);
    }
    Ok(())
}
pub(super) fn public_value<T: Serialize>(record: &T) -> Result<Value, CogniGraphError> {
    let mut value = serde_json::to_value(record)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("_key");
        fields.remove("idempotency_key_hash");
    }
    Ok(value)
}
pub(super) fn deserialize_required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
pub(super) fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}
pub(super) fn conflict(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::DocumentConflict(message.into())
}
pub(super) fn capacity(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::CapacityExceeded(message.into())
}
pub(super) fn not_found(kind: &str, id: &str) -> CogniGraphError {
    CogniGraphError::DocumentNotFound {
        collection: kind.into(),
        key: id.into(),
    }
}
pub(super) fn validate_identifier(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.is_empty()
        || value.len() > 256
        || value.chars().any(|ch| ch.is_control())
        || !unicode_normalization::is_nfc(value)
    {
        return Err(validation(format!("{label} is invalid")));
    }
    Ok(())
}
pub(super) fn validate_record_id(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(validation(format!(
            "{label} must be 64 lowercase hex characters"
        )));
    }
    Ok(())
}
pub(super) fn validate_digest(label: &str, value: &str) -> Result<(), CogniGraphError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(validation(format!("{label} must be a sha256 digest")));
    };
    validate_record_id(label, hex)
}
pub(super) fn generation_id(identity: &Value) -> Result<String, CogniGraphError> {
    Ok(canonical_digest(identity)?
        .trim_start_matches("sha256:")
        .to_string())
}
pub(super) fn deployment_head_key(
    tenant: &str,
    incarnation: &str,
    space_type: &str,
) -> Result<String, CogniGraphError> {
    validate_identifier("space_type", space_type)?;
    let id = canonical_digest(&json!({
        "tenant": tenant,
        "tenant_incarnation": incarnation,
        "space_type": space_type,
    }))?;
    Ok(scoped_key(
        tenant,
        incarnation,
        "m26h",
        id.trim_start_matches("sha256:"),
    ))
}
