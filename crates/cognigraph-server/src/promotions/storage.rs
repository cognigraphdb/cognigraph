//! Storage.

use super::*;

impl PromotionManager {
    pub(crate) async fn insert_immutable<T: Serialize>(
        &self,
        tenant: &str,
        collection: &str,
        key: &str,
        record: &T,
    ) -> Result<(), CogniGraphError> {
        if let Some(existing) = self.backend.get_document(collection, key).await? {
            if same_stored_record(&existing, &serde_json::to_value(record)?)? {
                return Ok(());
            }
            let error = conflict(format!(
                "immutable promotion record `{collection}/{key}` already differs"
            ));
            self.record_error(tenant, error.to_string());
            return Err(error);
        }
        self.backend
            .create_document(collection, serde_json::to_value(record)?)
            .await?;
        Ok(())
    }

    pub(crate) async fn get_raw<T: DeserializeOwned>(
        &self,
        collection: &str,
        key: &str,
    ) -> Result<Option<T>, CogniGraphError> {
        self.backend
            .get_document(collection, key)
            .await?
            .map(serde_json::from_value)
            .transpose()
            .map_err(CogniGraphError::from)
    }

    pub(crate) async fn get_authority_raw<T: DeserializeOwned>(
        &self,
        tenant: &str,
        collection: &str,
        key: &str,
    ) -> Result<Option<T>, CogniGraphError> {
        let value = self.backend.get_document(collection, key).await?;
        value
            .map(|value| serde_json::from_value(without_backend_metadata(&value)))
            .transpose()
            .map_err(CogniGraphError::from)
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn list_records<T, F>(
        &self,
        collection: &str,
        tenant: &str,
        incarnation: &str,
        kind: &str,
        limit: usize,
        cursor: Option<&str>,
        validate: F,
    ) -> Result<PromotionPage<T>, CogniGraphError>
    where
        T: DeserializeOwned,
        F: Fn(&T) -> Result<(), CogniGraphError>,
    {
        if !(1..=MAX_PAGE_SIZE).contains(&limit) {
            return Err(validation(format!(
                "promotion list limit must be between 1 and {MAX_PAGE_SIZE}"
            )));
        }
        let prefix = scoped_key(tenant, incarnation, kind, "");
        if let Some(cursor) = cursor
            && !cursor.starts_with(&prefix)
        {
            return Err(validation("promotion cursor belongs to another scope"));
        }
        let after = cursor.unwrap_or(&prefix);
        let fields = promotion_record_fields(collection)?;
        let rows = self
            .backend
            .list_documents_after_key(collection, Some(after), &fields, limit.saturating_add(1))
            .await?;
        let mut records = Vec::new();
        let mut keys = Vec::new();
        for value in rows {
            let Some(key) = value
                .get("_key")
                .and_then(Value::as_str)
                .map(str::to_string)
            else {
                return Err(conflict(
                    "promotion repository returned a record without `_key`",
                ));
            };
            if !key.starts_with(&prefix) {
                break;
            }
            let record: T = serde_json::from_value(value).map_err(|error| {
                let error = CogniGraphError::from(error);
                self.record_error(tenant, error.to_string());
                error
            })?;
            validate(&record)?;
            keys.push(key);
            records.push(record);
        }
        let has_more = records.len() > limit;
        if has_more {
            records.truncate(limit);
            keys.truncate(limit);
        }
        Ok(PromotionPage {
            next_cursor: has_more.then(|| keys.last().cloned()).flatten(),
            records,
        })
    }

    pub(super) async fn all_scoped_records<T: DeserializeOwned>(
        &self,
        collection: &str,
        tenant: &str,
        incarnation: &str,
        kind: &str,
    ) -> Result<Vec<T>, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, kind, "");
        let mut after = prefix.clone();
        let mut records = Vec::new();
        let fields = promotion_record_fields(collection)?;
        loop {
            let rows = self
                .backend
                .list_documents_after_key(collection, Some(&after), &fields, 256)
                .await?;
            if rows.is_empty() {
                break;
            }
            let mut advanced = false;
            for value in rows {
                let key = value
                    .get("_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| conflict("promotion repository record is missing `_key`"))?;
                if !key.starts_with(&prefix) {
                    return Ok(records);
                }
                after = key.to_string();
                let record = serde_json::from_value(value).map_err(|error| {
                    let error = CogniGraphError::from(error);
                    if collection != HEADS_COLLECTION {
                        self.record_error(tenant, error.to_string());
                    }
                    error
                })?;
                records.push(record);
                advanced = true;
            }
            if !advanced {
                break;
            }
        }
        Ok(records)
    }
}
