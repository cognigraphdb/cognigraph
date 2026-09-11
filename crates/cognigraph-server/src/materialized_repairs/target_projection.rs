//! Target projection.

use super::*;

impl PromotionManager {
    pub(super) async fn bounded_target_rows(
        &self,
        space_type: &str,
    ) -> Result<Vec<(&'static str, Vec<Value>)>, CogniGraphError> {
        let mut rows = Vec::new();
        for collection in ["chunks", "mentions", "facts"] {
            rows.push((collection, self.target_rows(collection, space_type).await?));
        }
        let row_count = rows
            .iter()
            .try_fold(0usize, |count, (_, values)| count.checked_add(values.len()))
            .ok_or_else(|| capacity("M26 target row count overflowed"))?;
        if row_count > MAX_M26_TOTAL_ROWS {
            return Err(capacity(format!(
                "existing M26 target has {row_count} rows, exceeding {MAX_M26_TOTAL_ROWS}"
            )));
        }
        let bytes = canonical_json_bytes(&rows)?.len();
        if bytes > MAX_M26_CANONICAL_GENERATION_BYTES {
            return Err(capacity(format!(
                "existing M26 target has {bytes} canonical bytes, exceeding {MAX_M26_CANONICAL_GENERATION_BYTES}"
            )));
        }
        Ok(rows)
    }

    pub(super) async fn active_target_matches_projection(
        &self,
        space_type: &str,
        projection: &MaterializedProjection,
    ) -> Result<bool, CogniGraphError> {
        for entity in &projection.entities {
            let expected = serde_json::to_value(entity)?;
            let key = row_key(&expected)?;
            let Some(existing) = self.backend.get_document("entities", key).await? else {
                return Ok(false);
            };
            if existing.get("_key").and_then(Value::as_str) != Some(key)
                || existing.get("name").and_then(Value::as_str)
                    != expected.get("name").and_then(Value::as_str)
                || existing.get("entity_type").and_then(Value::as_str)
                    != expected.get("entity_type").and_then(Value::as_str)
            {
                return Err(conflict(format!(
                    "M26 shared entity `{key}` has a conflicting canonical identity"
                )));
            }
        }
        let actual = self.bounded_target_rows(space_type).await?;
        for (collection, mut rows) in actual {
            for row in &mut rows {
                strip_target_backend_metadata(collection, row);
            }
            rows.sort_by(|left, right| row_key(left).ok().cmp(&row_key(right).ok()));
            let expected = match collection {
                "chunks" => projection
                    .chunks
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
                "mentions" => projection
                    .mentions
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
                "facts" => projection
                    .facts
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
                _ => unreachable!("closed M26 target collection set"),
            };
            if rows != expected {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) async fn target_replacement_ops(
        &self,
        space_type: &str,
        projection: &MaterializedProjection,
    ) -> Result<Vec<BatchOp>, CogniGraphError> {
        projection.validate()?;
        if projection.space_id != space_type {
            return Err(conflict(
                "M26 projection space does not match deployment space",
            ));
        }
        let mut ops = Vec::new();
        for entity in &projection.entities {
            let value = serde_json::to_value(entity)?;
            let key = row_key(&value)?;
            match self.backend.get_document("entities", key).await? {
                Some(existing)
                    if existing.get("_key").and_then(Value::as_str) != Some(key)
                        || existing.get("name").and_then(Value::as_str)
                            != value.get("name").and_then(Value::as_str)
                        || existing.get("entity_type").and_then(Value::as_str)
                            != value.get("entity_type").and_then(Value::as_str) =>
                {
                    return Err(conflict(format!(
                        "M26 shared entity `{key}` has a conflicting canonical identity"
                    )));
                }
                Some(_) => {}
                None => ops.push(BatchOp::Insert {
                    collection: "entities".into(),
                    doc: value,
                }),
            }
        }
        let existing_rows = self.bounded_target_rows(space_type).await?;
        for (collection, rows) in existing_rows {
            for value in rows {
                ops.push(BatchOp::Delete {
                    collection: collection.into(),
                    key: row_key(&value)?.into(),
                });
            }
        }
        for (collection, rows) in [
            (
                "chunks",
                projection
                    .chunks
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            (
                "mentions",
                projection
                    .mentions
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            (
                "facts",
                projection
                    .facts
                    .iter()
                    .map(serde_json::to_value)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        ] {
            for value in rows {
                let key = row_key(&value)?;
                if let Some(existing) = self.backend.get_document(collection, key).await?
                    && existing.get("space_id").and_then(Value::as_str) != Some(space_type)
                {
                    return Err(conflict(format!(
                        "M26 `{collection}/{key}` collides with another space"
                    )));
                }
                ops.push(BatchOp::Insert {
                    collection: collection.into(),
                    doc: value,
                });
            }
        }
        if ops.len() > MAX_M26_TOTAL_ROWS * 2 + MAX_M26_ENTITIES {
            return Err(capacity(
                "M26 atomic replacement operation count is too large",
            ));
        }
        Ok(ops)
    }

    pub(super) async fn target_rows(
        &self,
        collection: &str,
        space_type: &str,
    ) -> Result<Vec<Value>, CogniGraphError> {
        if !matches!(collection, "chunks" | "mentions" | "facts") {
            return Err(validation("unsupported M26 target collection"));
        }
        let predicates = [FieldPredicate {
            path: vec!["space_id".into()],
            op: PredicateOp::Eq,
            value: json!(space_type),
        }];
        let mut rows = self
            .backend
            .list_documents_filtered(
                collection,
                &predicates,
                None,
                Some(MAX_M26_TOTAL_ROWS + 1),
                None,
            )
            .await?;
        if rows.len() > MAX_M26_TOTAL_ROWS {
            return Err(capacity(format!(
                "existing M26 `{collection}` target rows exceed {MAX_M26_TOTAL_ROWS}"
            )));
        }
        let mut bytes = 0usize;
        for value in &mut rows {
            strip_target_backend_metadata(collection, value);
            row_key(value)?;
            bytes = bytes
                .checked_add(canonical_json_bytes(value)?.len())
                .ok_or_else(|| capacity("M26 target row byte accounting overflowed"))?;
            if bytes > MAX_M26_CANONICAL_GENERATION_BYTES {
                return Err(capacity(format!(
                    "existing M26 `{collection}` target rows exceed {MAX_M26_CANONICAL_GENERATION_BYTES} canonical bytes"
                )));
            }
        }
        Ok(rows)
    }
}
