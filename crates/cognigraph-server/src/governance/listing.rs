//! Listing.

use super::*;

impl PromotionManager {
    pub(crate) async fn list_governance_records<T>(
        &self,
        collection: &str,
        tenant: &str,
        incarnation: &str,
        kind: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<T>, CogniGraphError>
    where
        T: DeserializeOwned,
    {
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(validation(format!(
                "governance list limit must be between 1 and {MAX_PAGE_SIZE}"
            )));
        }
        self.ensure_governance_repository(tenant).await?;
        if is_semantic_repair_collection(collection) {
            return self
                .list_semantic_repair_records(collection, tenant, incarnation, kind, limit, cursor)
                .await;
        }
        let prefix = scoped_key(tenant, incarnation, kind, "");
        let mut after = cursor.unwrap_or(&prefix).to_string();
        if !after.starts_with(&prefix) {
            return Err(validation("governance cursor belongs to another scope"));
        }
        let mut rows = self.backend.list_documents(collection, None, None).await?;
        rows.sort_by(|left, right| {
            left.get("_key")
                .and_then(Value::as_str)
                .cmp(&right.get("_key").and_then(Value::as_str))
        });
        let mut records = Vec::new();
        let mut keys = Vec::new();
        for value in rows {
            let Some(key) = value.get("_key").and_then(Value::as_str) else {
                let error = conflict("governance record is missing `_key`");
                self.record_error(tenant, error.to_string());
                return Err(error);
            };
            let key = key.to_string();
            if !key.starts_with(&prefix) || key <= after {
                continue;
            }
            after.clone_from(&key);
            let record: T = deserialize_stored(value)
                .inspect_err(|error| self.record_error(tenant, error.to_string()))?;
            keys.push(key);
            records.push(record);
            if records.len() > limit {
                break;
            }
        }
        let has_more = records.len() > limit;
        if has_more {
            records.truncate(limit);
            keys.truncate(limit);
        }
        Ok(PromotionPage {
            records,
            next_cursor: has_more.then(|| keys.last().cloned()).flatten(),
        })
    }

    pub(super) async fn list_semantic_repair_records<T: DeserializeOwned>(
        &self,
        collection: &str,
        tenant: &str,
        incarnation: &str,
        kind: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<T>, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, kind, "");
        let requested_after = cursor.unwrap_or(&prefix).to_string();
        if !requested_after.starts_with(&prefix) {
            return Err(validation("governance cursor belongs to another scope"));
        }
        // Public pagination is a true keyset window: one small key-only query
        // and at most `limit` full-record fetches. Global count/aggregate
        // validation remains mandatory at admission, recovery, snapshot
        // preflight, and current-authority resolution.
        let projected = vec!["_key".into()];
        let rows = self
            .backend
            .list_documents_after_key(
                collection,
                Some(&requested_after),
                &projected,
                limit.saturating_add(1),
            )
            .await?;
        let mut keys = Vec::with_capacity(limit.saturating_add(1));
        for projected_value in rows {
            let key = projected_value
                .get("_key")
                .and_then(Value::as_str)
                .ok_or_else(|| conflict("semantic repair record is missing `_key`"))?;
            if !key.starts_with(&prefix) {
                break;
            }
            keys.push(key.to_string());
        }
        let has_more = keys.len() > limit;
        if has_more {
            keys.truncate(limit);
        }

        let mut records = Vec::with_capacity(keys.len());
        let mut candidate_bytes = 0_usize;
        for key in &keys {
            records.push(
                self.load_semantic_repair_record(collection, tenant, key, &mut candidate_bytes)
                    .await?,
            );
        }
        Ok(PromotionPage {
            records,
            next_cursor: has_more.then(|| keys.last().cloned()).flatten(),
        })
    }
}
