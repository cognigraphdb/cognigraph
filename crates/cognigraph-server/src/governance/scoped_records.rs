//! Scoped records.

use super::*;

impl PromotionManager {
    pub(crate) async fn scoped_records<T: DeserializeOwned>(
        &self,
        collection: &str,
        tenant: &str,
        incarnation: &str,
        kind: &str,
        limit: usize,
    ) -> Result<Vec<T>, CogniGraphError> {
        if is_semantic_repair_collection(collection) {
            return self
                .scoped_semantic_repair_records(collection, tenant, incarnation, kind, limit)
                .await;
        }
        let prefix = scoped_key(tenant, incarnation, kind, "");
        let mut rows = self.backend.list_documents(collection, None, None).await?;
        rows.sort_by(|left, right| {
            left.get("_key")
                .and_then(Value::as_str)
                .cmp(&right.get("_key").and_then(Value::as_str))
        });
        rows.into_iter()
            .filter(|value| {
                value
                    .get("_key")
                    .and_then(Value::as_str)
                    .is_some_and(|key| key.starts_with(&prefix))
            })
            .take(limit)
            .map(|value| {
                deserialize_stored(value)
                    .inspect_err(|error| self.record_error(tenant, error.to_string()))
            })
            .collect()
    }

    pub(super) async fn scoped_semantic_repair_records<T: DeserializeOwned>(
        &self,
        collection: &str,
        tenant: &str,
        incarnation: &str,
        kind: &str,
        limit: usize,
    ) -> Result<Vec<T>, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, kind, "");
        let mut after = prefix.clone();
        let mut records = Vec::new();
        let mut candidate_bytes = 0_usize;
        let projected = vec!["_key".into()];
        loop {
            let rows = self
                .backend
                .list_documents_after_key(
                    collection,
                    Some(&after),
                    &projected,
                    GOVERNANCE_SCAN_PAGE_SIZE,
                )
                .await?;
            if rows.is_empty() {
                break;
            }
            let mut advanced = false;
            for projected_value in rows {
                let key = projected_value
                    .get("_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| conflict("semantic repair record is missing `_key`"))?
                    .to_string();
                if !key.starts_with(&prefix) {
                    return Ok(records);
                }
                after.clone_from(&key);
                let record = self
                    .load_semantic_repair_record(collection, tenant, &key, &mut candidate_bytes)
                    .await?;
                records.push(record);
                advanced = true;
                if records.len() >= limit {
                    return Ok(records);
                }
            }
            if !advanced {
                break;
            }
        }
        Ok(records)
    }

    /// Load one full M25 record after a key-only keyset page. Keeping the
    /// projected page tiny prevents a tenant from materializing every repair
    /// candidate in one backend response. Full records are still fetched so
    /// `deny_unknown_fields` continues to detect direct-store tampering.
    pub(super) async fn load_semantic_repair_record<T: DeserializeOwned>(
        &self,
        collection: &str,
        tenant: &str,
        key: &str,
        candidate_bytes: &mut usize,
    ) -> Result<T, CogniGraphError> {
        let value = self
            .backend
            .get_document(collection, key)
            .await?
            .ok_or_else(|| conflict("semantic repair record disappeared during authority scan"))?;
        if collection == SEMANTIC_REPAIR_REVISIONS_COLLECTION {
            let candidate = value
                .get("candidate")
                .ok_or_else(|| conflict("semantic repair revision is missing its candidate"))?;
            *candidate_bytes = candidate_bytes
                .checked_add(canonical_json_bytes(candidate)?.len())
                .ok_or_else(|| {
                    conflict("semantic repair candidate storage accounting overflowed")
                })?;
            if *candidate_bytes > MAX_SEMANTIC_REPAIR_CANDIDATE_BYTES_PER_TENANT {
                return Err(conflict(
                    "semantic repair candidate storage capacity exceeded",
                ));
            }
        }
        deserialize_stored(value).inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(crate) async fn ensure_governance_idempotency_available_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        idempotency_key_hash: &str,
    ) -> Result<(), CogniGraphError> {
        for collection in [
            GOVERNANCE_KEYS_COLLECTION,
            GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
            POLICY_REVISIONS_COLLECTION,
            POLICY_APPROVALS_COLLECTION,
            ARTIFACT_ATTESTATIONS_COLLECTION,
        ] {
            if self
                .backend
                .list_documents(collection, None, None)
                .await?
                .iter()
                .any(|record| {
                    record.get("tenant").and_then(Value::as_str) == Some(tenant)
                        && record.get("tenant_incarnation").and_then(Value::as_str)
                            == Some(incarnation)
                        && record.get("idempotency_key_hash").and_then(Value::as_str)
                            == Some(idempotency_key_hash)
                })
            {
                return Err(conflict(
                    "Idempotency-Key was already used for different governance authority",
                ));
            }
        }
        Ok(())
    }
}
