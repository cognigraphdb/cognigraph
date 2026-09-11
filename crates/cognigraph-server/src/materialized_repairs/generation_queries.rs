//! Generation queries.

use super::*;

impl PromotionManager {
    pub async fn get_semantic_repair_generation(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<SemanticRepairGenerationRecord, CogniGraphError> {
        require_native_atomic_backend(self)?;
        validate_record_id("semantic_repair_generation_id", id)?;
        self.ensure_materialized_repair_repository(tenant).await?;
        let key = scoped_key(tenant, incarnation, "m26g", id);
        let record = self
            .get_authority_raw::<SemanticRepairGenerationRecord>(
                tenant,
                SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
                &key,
            )
            .await?
            .ok_or_else(|| not_found("semantic repair generations", id))?;
        self.validate_stored_semantic_repair_generation(&record, tenant, incarnation)
            .await?;
        Ok(record)
    }

    pub async fn list_semantic_repair_generations(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<(Vec<SemanticRepairGenerationRecord>, Option<String>), CogniGraphError> {
        require_native_atomic_backend(self)?;
        if !(1..=MAX_M26_LIST_PAGE_SIZE).contains(&limit) {
            return Err(validation(format!(
                "M26 generation page limit must be between 1 and {MAX_M26_LIST_PAGE_SIZE}"
            )));
        }
        self.ensure_materialized_repair_repository(tenant).await?;
        let prefix = scoped_key(tenant, incarnation, "m26g", "");
        if let Some(cursor) = cursor
            && !cursor.starts_with(&prefix)
        {
            return Err(validation("M26 generation cursor belongs to another scope"));
        }
        let keys = self
            .scoped_record_keys(
                SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
                &prefix,
                cursor,
                limit + 1,
            )
            .await?;
        let next_cursor = (keys.len() > limit).then(|| keys[limit - 1].clone());
        let mut records = Vec::with_capacity(keys.len().min(limit));
        for key in keys.into_iter().take(limit) {
            let record = self
                .get_authority_raw::<SemanticRepairGenerationRecord>(
                    tenant,
                    SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
                    &key,
                )
                .await?
                .ok_or_else(|| conflict("M26 generation vanished during listing"))?;
            self.validate_stored_semantic_repair_generation(&record, tenant, incarnation)
                .await?;
            records.push(record);
        }
        Ok((records, next_cursor))
    }

    pub(super) async fn generation_by_idempotency_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        key_hash: &str,
    ) -> Result<Option<SemanticRepairGenerationRecord>, CogniGraphError> {
        let records = self
            .all_semantic_repair_generations_locked(tenant, incarnation)
            .await?;
        let mut matching = records
            .into_iter()
            .filter(|record| record.idempotency_key_hash == key_hash);
        let first = matching.next();
        if matching.next().is_some() {
            return Err(conflict(
                "M26 generation idempotency-key hash has duplicate immutable authority",
            ));
        }
        Ok(first)
    }

    pub(super) async fn all_semantic_repair_generations_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<Vec<SemanticRepairGenerationRecord>, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, "m26g", "");
        let keys = self
            .scoped_record_keys(
                SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
                &prefix,
                None,
                MAX_M26_GENERATIONS_PER_TENANT + 1,
            )
            .await?;
        if keys.len() > MAX_M26_GENERATIONS_PER_TENANT {
            return Err(conflict("M26 tenant generation quota is already exceeded"));
        }
        let mut records = Vec::with_capacity(keys.len());
        for key in keys {
            records.push(
                self.get_authority_raw::<SemanticRepairGenerationRecord>(
                    tenant,
                    SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
                    &key,
                )
                .await?
                .ok_or_else(|| conflict("M26 generation vanished during validation"))?,
            );
        }
        Ok(records)
    }
}
