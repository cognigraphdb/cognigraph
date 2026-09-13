//! Repository.

use super::*;

impl PromotionManager {
    pub(super) async fn authoritative_evaluation_source(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PromotionEvaluationSource, CogniGraphError> {
        let result = self
            .jobs
            .promotion_evaluation_source(tenant, incarnation, id)
            .await;
        if let Err(error @ CogniGraphError::BackendError(_)) = &result {
            self.record_error(tenant, error.to_string());
        }
        result
    }

    pub(crate) async fn ensure_repository(&self, tenant: &str) -> Result<(), CogniGraphError> {
        for collection in [EVIDENCE_COLLECTION, DECISIONS_COLLECTION, HEADS_COLLECTION] {
            if let Err(error) = self
                .backend
                .ensure_collection(collection, CollectionType::Document)
                .await
            {
                self.record_error(tenant, format!("repository initialization failed: {error}"));
                return Err(error);
            }
        }
        if let Err(error) = self
            .backend
            .ensure_index(
                DECISIONS_COLLECTION,
                &IndexDef {
                    index_type: IndexType::Persistent,
                    fields: vec![
                        "tenant".into(),
                        "tenant_incarnation".into(),
                        "target.space_type".into(),
                        "target.channel".into(),
                    ],
                    unique: false,
                    sparse: false,
                    name: Some("idx_cognigraph_promotion_target".into()),
                },
            )
            .await
        {
            self.record_error(
                tenant,
                format!("repository index initialization failed: {error}"),
            );
            return Err(error);
        }
        Ok(())
    }
}
