//! Canonical.

use super::*;

pub fn canonical_digest<T: Serialize>(value: &T) -> Result<String, CogniGraphError> {
    Ok(digest_bytes(&canonical_json_bytes(value)?))
}
pub(crate) fn canonical_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CogniGraphError> {
    let value = serde_json::to_value(value)?;
    let canonical = canonicalize(&value)?;
    serde_json::to_vec(&canonical).map_err(CogniGraphError::from)
}
pub fn digest_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex = digest
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            use std::fmt::Write;
            let _ = write!(output, "{byte:02x}");
            output
        });
    format!("sha256:{hex}")
}
pub(super) fn canonicalize(value: &Value) -> Result<Value, CogniGraphError> {
    match value {
        Value::Object(map) => {
            let mut normalized = map
                .iter()
                .map(|(key, value)| Ok((key.nfc().collect::<String>(), canonicalize(value)?)))
                .collect::<Result<Vec<_>, CogniGraphError>>()?;
            normalized.sort_by(|left, right| left.0.cmp(&right.0));
            if normalized.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(validation(
                    "canonical JSON contains object keys that collide after NFC normalization",
                ));
            }
            let mut output = serde_json::Map::new();
            for (key, value) in normalized {
                output.insert(key, value);
            }
            Ok(Value::Object(output))
        }
        Value::Array(values) => values
            .iter()
            .map(canonicalize)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::String(value) => Ok(Value::String(value.nfc().collect())),
        other => Ok(other.clone()),
    }
}
