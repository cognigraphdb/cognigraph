//! Head validation.

use super::*;

pub(super) fn head_key(
    tenant: &str,
    incarnation: &str,
    target: &PromotionTarget,
) -> Result<String, CogniGraphError> {
    let target_id = canonical_digest(target)?;
    Ok(scoped_key(
        tenant,
        incarnation,
        "h",
        target_id.trim_start_matches("sha256:"),
    ))
}
pub(super) fn promotion_record_fields(collection: &str) -> Result<Vec<String>, CogniGraphError> {
    let fields = match collection {
        EVIDENCE_COLLECTION => EVIDENCE_FIELDS,
        DECISIONS_COLLECTION => DECISION_FIELDS,
        HEADS_COLLECTION => HEAD_FIELDS,
        _ => {
            return Err(CogniGraphError::BackendError(format!(
                "unsupported promotion repository collection `{collection}`"
            )));
        }
    };
    Ok(fields.iter().map(|field| (*field).to_string()).collect())
}
pub(super) fn valid_head(
    record: &PromotionHead,
    tenant: &str,
    incarnation: &str,
    target: &PromotionTarget,
    key: &str,
) -> Result<bool, CogniGraphError> {
    Ok(matches!(
        record.schema_version,
        PROMOTION_HEAD_SCHEMA_VERSION
            | M21_PROMOTION_HEAD_SCHEMA_VERSION
            | M22_PROMOTION_HEAD_SCHEMA_VERSION
            | M23_PROMOTION_HEAD_SCHEMA_VERSION
    ) && record.digest_algorithm == DIGEST_ALGORITHM
        && record.tenant == tenant
        && record.tenant_incarnation == incarnation
        && record.key == key
        && record.target == *target
        && record_digest(record, "projection_digest")? == record.projection_digest)
}
pub(crate) fn same_stored_record(left: &Value, right: &Value) -> Result<bool, CogniGraphError> {
    Ok(canonical_digest(&without_backend_metadata(left))?
        == canonical_digest(&without_backend_metadata(right))?)
}
pub(super) fn without_backend_metadata(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(fields) = value.as_object_mut() {
        fields.remove("_id");
        fields.remove("_rev");
        fields.remove("created_at");
        fields.remove("updated_at");
    }
    value
}
pub(super) fn conflict(message: impl Into<String>) -> CogniGraphError {
    CogniGraphError::DocumentConflict(message.into())
}
pub(super) fn not_found(collection: &str, id: &str) -> CogniGraphError {
    CogniGraphError::DocumentNotFound {
        collection: collection.into(),
        key: id.into(),
    }
}
