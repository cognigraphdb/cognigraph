//! Repository.

use super::*;

impl PromotionManager {
    pub(super) async fn require_collection_types(
        &self,
        expected: &[(&str, CollectionType)],
    ) -> Result<(), CogniGraphError> {
        let catalog = self
            .backend
            .list_collections()
            .await?
            .into_iter()
            .map(|collection| (collection.name, collection.collection_type))
            .collect::<BTreeMap<_, _>>();
        for (name, collection_type) in expected {
            match catalog.get(*name).map(String::as_str) {
                Some(actual) if actual == collection_type.as_str() => {}
                Some(actual) => {
                    return Err(conflict(format!(
                        "M26 collection `{name}` has type `{actual}`, expected `{}`",
                        collection_type.as_str()
                    )));
                }
                None => {
                    return Err(conflict(format!(
                        "M26 required collection `{name}` is missing"
                    )));
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn ensure_materialized_repair_repository(
        &self,
        tenant: &str,
    ) -> Result<(), CogniGraphError> {
        require_native_atomic_backend(self)?;
        for collection in [
            SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
            SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
            SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION,
        ] {
            if let Err(error) = self
                .backend
                .ensure_collection(collection, CollectionType::Document)
                .await
            {
                self.record_error(
                    tenant,
                    format!("M26 materialization repository initialization failed: {error}"),
                );
                return Err(error);
            }
        }
        if let Err(error) = self
            .backend
            .ensure_index(
                SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
                &IndexDef {
                    index_type: IndexType::Persistent,
                    fields: vec![
                        "tenant".into(),
                        "tenant_incarnation".into(),
                        "space_type".into(),
                    ],
                    unique: false,
                    sparse: false,
                    name: Some("idx_cognigraph_m26_deployment_space".into()),
                },
            )
            .await
        {
            self.record_error(
                tenant,
                format!("M26 materialization repository index initialization failed: {error}"),
            );
            return Err(error);
        }
        self.require_collection_types(&[
            (
                SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
                CollectionType::Document,
            ),
            (
                SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
                CollectionType::Document,
            ),
            (
                SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION,
                CollectionType::Document,
            ),
        ])
        .await?;
        Ok(())
    }

    pub(super) async fn ensure_materialization_target_collections(
        &self,
        tenant: &str,
    ) -> Result<(), CogniGraphError> {
        let expected = [
            ("entities", CollectionType::Document),
            ("chunks", CollectionType::Document),
            ("mentions", CollectionType::Edge),
            ("facts", CollectionType::Edge),
        ];
        for (collection, kind) in expected {
            if let Err(error) = self.backend.ensure_collection(collection, kind).await {
                self.record_error(
                    tenant,
                    format!("M26 target collection initialization failed: {error}"),
                );
                return Err(error);
            }
        }
        for collection in ["chunks", "mentions", "facts"] {
            if let Err(error) = self
                .backend
                .ensure_index(
                    collection,
                    &IndexDef {
                        index_type: IndexType::Persistent,
                        fields: vec!["space_id".into()],
                        unique: false,
                        sparse: false,
                        name: Some(format!("idx_cognigraph_m26_{collection}_space")),
                    },
                )
                .await
            {
                self.record_error(
                    tenant,
                    format!("M26 target index initialization failed: {error}"),
                );
                return Err(error);
            }
        }
        self.require_collection_types(&expected).await
    }
}
