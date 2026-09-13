//! Generation capacity.

use super::*;

impl PromotionManager {
    pub(super) async fn scoped_record_keys(
        &self,
        collection: &str,
        prefix: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, CogniGraphError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let mut cursor = after.unwrap_or(prefix).to_string();
        let mut keys = Vec::with_capacity(limit);
        loop {
            let page_limit = (limit - keys.len()).min(256);
            let rows = self
                .backend
                .list_documents_after_key(collection, Some(&cursor), &["_key".into()], page_limit)
                .await?;
            if rows.is_empty() {
                break;
            }
            let mut advanced = false;
            for value in rows {
                let key = value
                    .get("_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| conflict("M26 authority record is missing `_key`"))?
                    .to_string();
                if !key.starts_with(prefix) {
                    return Ok(keys);
                }
                cursor.clone_from(&key);
                keys.push(key);
                advanced = true;
                if keys.len() == limit {
                    return Ok(keys);
                }
            }
            if !advanced {
                break;
            }
        }
        Ok(keys)
    }

    pub(super) async fn ensure_generation_capacity_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        candidate: &SemanticRepairGenerationRecord,
    ) -> Result<(), CogniGraphError> {
        let records = self
            .all_semantic_repair_generations_locked(tenant, incarnation)
            .await?;
        let capacity_result = validate_generation_capacity_entries(
            records
                .iter()
                .chain(std::iter::once(candidate))
                .map(|record| GenerationCapacityEntry {
                    space_type: &record.target.space_type,
                    idempotency_key_hash: &record.idempotency_key_hash,
                    canonical_record_bytes: record.canonical_record_bytes,
                }),
        );
        match capacity_result {
            Ok(()) => Ok(()),
            Err(GenerationCapacityViolation::TenantCount) => Err(capacity(format!(
                "M26 tenant generation limit of {MAX_M26_GENERATIONS_PER_TENANT} is exhausted"
            ))),
            Err(GenerationCapacityViolation::SpaceCount) => Err(capacity(format!(
                "M26 space generation limit of {MAX_M26_GENERATIONS_PER_SPACE} is exhausted"
            ))),
            Err(GenerationCapacityViolation::AggregateBytes) => Err(capacity(format!(
                "M26 tenant generation bytes exceed {MAX_M26_GENERATION_BYTES_PER_TENANT}"
            ))),
            Err(GenerationCapacityViolation::ByteAccountingOverflow) => Err(capacity(
                "M26 aggregate generation byte accounting overflowed",
            )),
            Err(GenerationCapacityViolation::DuplicateIdempotencyHash) => Err(conflict(
                "M26 duplicate generation idempotency-key hash is stored",
            )),
        }
    }
}
