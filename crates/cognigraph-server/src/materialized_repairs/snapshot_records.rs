//! Snapshot records.

use super::*;

pub(super) fn row_key(value: &Value) -> Result<&str, CogniGraphError> {
    value
        .get("_key")
        .and_then(Value::as_str)
        .ok_or_else(|| conflict("M26 materialized row has no string `_key`"))
}
pub(super) fn strip_backend_metadata(value: &mut Value) {
    if let Some(fields) = value.as_object_mut() {
        for field in ["_id", "_rev", "created_at", "updated_at"] {
            fields.remove(field);
        }
    }
}
pub(super) fn strip_target_backend_metadata(collection: &str, value: &mut Value) {
    strip_backend_metadata(value);
    if matches!(collection, "mentions" | "facts")
        && let Some(fields) = value.as_object_mut()
    {
        fields.remove("confidence");
    }
}
pub(super) fn snapshot_collection<'a>(snapshot: &'a Value, collection: &str) -> Option<&'a Value> {
    snapshot
        .get("collections")
        .and_then(Value::as_object)
        .and_then(|collections| collections.get(collection))
}
pub(super) fn validate_snapshot_declared_collection_type(
    snapshot: &Value,
    collection: &str,
    expected: CollectionType,
) -> Result<(), CogniGraphError> {
    let Some(entry) = snapshot_collection(snapshot, collection) else {
        return Ok(());
    };
    let actual = entry
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| validation(format!("snapshot collection `{collection}` has no type")))?;
    if actual != expected.as_str() {
        return Err(conflict(format!(
            "snapshot collection `{collection}` has type `{actual}`, expected `{}`",
            expected.as_str()
        )));
    }
    Ok(())
}
pub(super) fn snapshot_documents<'a>(
    snapshot: &'a Value,
    collection: &str,
) -> Result<Option<&'a serde_json::Map<String, Value>>, CogniGraphError> {
    let Some(collections) = snapshot.get("collections") else {
        return Ok(None);
    };
    let collections = collections
        .as_object()
        .ok_or_else(|| validation("snapshot `collections` must be an object"))?;
    let Some(entry) = collections.get(collection) else {
        return Ok(None);
    };
    let documents = entry
        .get("documents")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            validation(format!(
                "snapshot collection `{collection}` must contain a documents object"
            ))
        })?;
    Ok(Some(documents))
}
pub(super) async fn snapshot_or_stored<T: DeserializeOwned>(
    manager: &PromotionManager,
    snapshot: &Value,
    collection: &str,
    key: &str,
) -> Result<Option<T>, CogniGraphError> {
    if let Some(value) = snapshot_documents(snapshot, collection)?.and_then(|docs| docs.get(key)) {
        let mut value = value.clone();
        strip_backend_metadata(&mut value);
        return serde_json::from_value(value).map(Some).map_err(Into::into);
    }
    let value = manager.backend.get_document(collection, key).await?;
    value
        .map(|mut value| {
            strip_backend_metadata(&mut value);
            serde_json::from_value(value)
        })
        .transpose()
        .map_err(Into::into)
}
pub(super) fn snapshot_promotion_principal(
    decision: &PromotionDecision,
) -> Result<&str, CogniGraphError> {
    decision
        .governance
        .as_ref()
        .map(|signed| signed.statement.payload.promoter_principal_id.as_str())
        .ok_or_else(|| conflict("snapshot M26 resolution requires signed promotion decisions"))
}
pub(super) fn same_snapshot_selected_authority(
    left: &crate::promotions::PromotionSelection,
    right: &crate::promotions::PromotionSelection,
) -> bool {
    left.evidence_id == right.evidence_id
        && left.evidence_digest == right.evidence_digest
        && left.candidate_digest == right.candidate_digest
        && left.policy_digest == right.policy_digest
}
pub(super) fn validate_snapshot_resolution_separation(
    promoter_principal_id: &str,
    author_principal_id: &str,
    approver_principal_id: &str,
) -> Result<(), CogniGraphError> {
    if promoter_principal_id == author_principal_id
        || promoter_principal_id == approver_principal_id
        || author_principal_id == approver_principal_id
    {
        return Err(CogniGraphError::Forbidden(
            "snapshot Semantic Repair author, reviewer, and promoter principals must be distinct"
                .into(),
        ));
    }
    Ok(())
}
