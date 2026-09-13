//! Record identity.

use super::*;

pub fn record_digest<T: Serialize>(
    record: &T,
    digest_field: &str,
) -> Result<String, CogniGraphError> {
    let mut value = serde_json::to_value(record)?;
    let fields = value
        .as_object_mut()
        .ok_or_else(|| validation("promotion record must serialize as an object"))?;
    fields.remove("_key");
    fields.remove(digest_field);
    canonical_digest(&value)
}
pub(super) fn public_record_value<T: Serialize>(record: &T) -> Result<Value, CogniGraphError> {
    let mut value = serde_json::to_value(record)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("_key");
        fields.remove("idempotency_key_hash");
    }
    Ok(value)
}
pub fn scoped_key(tenant: &str, incarnation: &str, kind: &str, id: &str) -> String {
    let scope = digest_bytes(format!("{tenant}\0{incarnation}").as_bytes());
    // `:` is accepted by Native document keys.
    // Keep the prefix lexicographically sortable for cursor scans.
    format!("{}:{kind}:{id}", scope.trim_start_matches("sha256:"))
}
pub fn new_record_id(
    tenant: &str,
    incarnation: &str,
    operation: &str,
    idempotency_key_hash: &str,
) -> String {
    digest_bytes(format!("{tenant}\0{incarnation}\0{operation}\0{idempotency_key_hash}").as_bytes())
        .trim_start_matches("sha256:")
        .to_string()
}
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
pub fn sorted_fact_lines(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}
pub fn promotion_status_value(healthy: bool, error: Option<&str>) -> Value {
    json!({
        "status": if healthy { "ok" } else { "error" },
        "promotions": if healthy { "ready" } else { "unavailable" },
        "error": error,
    })
}
