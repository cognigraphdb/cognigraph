//! Validation.

use super::*;

pub fn validate_idempotency_key(key: &str) -> Result<(), CogniGraphError> {
    if key.is_empty() || key.len() > 128 || !key.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        return Err(validation(
            "Idempotency-Key must be 1-128 printable ASCII characters",
        ));
    }
    Ok(())
}
pub fn validate_reason(reason: &str) -> Result<(), CogniGraphError> {
    validate_text("reason", reason, MAX_REASON_BYTES)
}
pub(super) fn validate_path_segment(label: &str, value: &str) -> Result<(), CogniGraphError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
    {
        return Err(validation(format!(
            "{label} must match [A-Za-z0-9._-] and be 1-128 bytes"
        )));
    }
    Ok(())
}
pub(super) fn validate_text(label: &str, value: &str, max: usize) -> Result<(), CogniGraphError> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(validation(format!(
            "{label} must be non-empty, control-free, and at most {max} bytes"
        )));
    }
    Ok(())
}
pub(super) fn validate_digest(label: &str, value: &str) -> Result<(), CogniGraphError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(validation(format!(
            "{label} must use an algorithm-prefixed sha256 digest"
        )));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(validation(format!(
            "{label} must be sha256:<64 lowercase hex characters>"
        )));
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
            "{label} must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}
pub(super) fn validate_ratio(
    label: &str,
    numerator: u64,
    denominator: u64,
) -> Result<(), CogniGraphError> {
    if denominator == 0 || numerator > denominator {
        return Err(validation(format!(
            "{label} ratio must have 0 <= numerator <= positive denominator"
        )));
    }
    Ok(())
}
pub(super) fn validate_sorted_unique(
    label: &str,
    values: &[String],
) -> Result<(), CogniGraphError> {
    for value in values {
        validate_text(label, value, MAX_IDENTIFIER_BYTES)?;
    }
    if !is_sorted_unique(values) {
        return Err(validation(format!("{label} must be sorted and unique")));
    }
    Ok(())
}
pub(super) fn is_sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
pub(super) fn validation(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::ValidationError(message.into())
}
