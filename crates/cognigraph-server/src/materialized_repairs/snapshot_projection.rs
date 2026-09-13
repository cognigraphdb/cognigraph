//! Snapshot projection.

use super::*;

impl PromotionManager {
    pub(super) async fn validate_snapshot_target_projection(
        &self,
        snapshot: &Value,
        space_type: &str,
        projection: &MaterializedProjection,
    ) -> Result<(), CogniGraphError> {
        let current_types = self
            .backend
            .list_collections()
            .await?
            .into_iter()
            .map(|collection| (collection.name, collection.collection_type))
            .collect::<BTreeMap<_, _>>();
        for (collection, expected) in [
            ("entities", CollectionType::Document),
            ("chunks", CollectionType::Document),
            ("mentions", CollectionType::Edge),
            ("facts", CollectionType::Edge),
        ] {
            let current_type = current_types.get(collection).map(String::as_str);
            if current_type.is_some_and(|actual| actual != expected.as_str()) {
                return Err(conflict(format!(
                    "snapshot M26 active target requires existing `{collection}` to be a `{}` collection",
                    expected.as_str()
                )));
            }
            validate_snapshot_declared_collection_type(snapshot, collection, expected)?;
            if current_type.is_none() && snapshot_collection(snapshot, collection).is_none() {
                return Err(conflict(format!(
                    "snapshot M26 active target requires `{collection}` to be a `{}` collection",
                    expected.as_str()
                )));
            }
            if let Some(documents) = snapshot_documents(snapshot, collection)? {
                for (key, value) in documents {
                    if value.get("_key").and_then(Value::as_str) != Some(key) {
                        return Err(conflict(format!(
                            "snapshot `{collection}` map key and embedded _key differ"
                        )));
                    }
                }
            }
        }
        for entity in &projection.entities {
            let expected = serde_json::to_value(entity)?;
            let key = row_key(&expected)?;
            let mut actual = self.backend.get_document("entities", key).await?;
            if let Some(value) =
                snapshot_documents(snapshot, "entities")?.and_then(|documents| documents.get(key))
            {
                actual = Some(value.clone());
            }
            let actual = actual.ok_or_else(|| {
                conflict(format!(
                    "snapshot M26 active target is missing required entity `{key}`"
                ))
            })?;
            if actual.get("_key").and_then(Value::as_str) != Some(key)
                || actual.get("name").and_then(Value::as_str)
                    != expected.get("name").and_then(Value::as_str)
                || actual.get("entity_type").and_then(Value::as_str)
                    != expected.get("entity_type").and_then(Value::as_str)
            {
                return Err(conflict(format!(
                    "snapshot M26 entity `{key}` conflicts with generation identity"
                )));
            }
        }
        for (collection, expected) in [
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
            let mut union = if current_types.contains_key(collection) {
                self.target_rows(collection, space_type)
                    .await?
                    .into_iter()
                    .map(|mut value| {
                        strip_target_backend_metadata(collection, &mut value);
                        row_key(&value).map(str::to_string).map(|key| (key, value))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?
            } else {
                BTreeMap::new()
            };
            if let Some(documents) = snapshot_documents(snapshot, collection)? {
                for (key, value) in documents {
                    if union.contains_key(key)
                        && value.get("space_id").and_then(Value::as_str) != Some(space_type)
                    {
                        union.remove(key);
                    } else if value.get("space_id").and_then(Value::as_str) == Some(space_type) {
                        let mut value = value.clone();
                        strip_target_backend_metadata(collection, &mut value);
                        union.insert(key.clone(), value);
                    }
                }
            }
            if union.into_values().collect::<Vec<_>>() != expected {
                return Err(conflict(format!(
                    "snapshot M26 active `{collection}` rows do not equal the selected generation"
                )));
            }
        }
        Ok(())
    }
}
