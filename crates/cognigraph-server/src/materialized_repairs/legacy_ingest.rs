//! Legacy ingest.

use super::*;

impl PromotionManager {
    pub(super) async fn materialized_repair_repository_present(
        &self,
        tenant: &str,
    ) -> Result<bool, CogniGraphError> {
        let required = BTreeSet::from(M26_PROTECTED_COLLECTIONS);
        let present = self
            .backend
            .list_collections()
            .await?
            .into_iter()
            .filter_map(|collection| {
                required
                    .contains(collection.name.as_str())
                    .then_some(collection.name)
            })
            .collect::<BTreeSet<_>>();
        if present.is_empty() {
            return Ok(false);
        }
        if present.len() != required.len() {
            // Collection creation is individually idempotent but not one
            // atomic database act. A process stop, transient failure, or a
            // fresh snapshot import (which deliberately omits derived heads)
            // can therefore leave a recoverable prefix of the repository.
            // Complete it before inspecting immutable authority.
            self.ensure_materialized_repair_repository(tenant).await?;
            let completed = self
                .backend
                .list_collections()
                .await?
                .into_iter()
                .filter(|collection| required.contains(collection.name.as_str()))
                .count();
            if completed != required.len() {
                return Err(conflict(
                    "M26 materialization repository initialization remained incomplete",
                ));
            }
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
        Ok(true)
    }

    pub(super) async fn require_non_atomic_materialized_repair_repository_absent(
        &self,
    ) -> Result<(), CogniGraphError> {
        debug_assert!(!self.backend.supports_atomic_batches());
        let key_field = ["_key".to_string()];
        for collection in M26_PROTECTED_COLLECTIONS {
            let probe = self
                .backend
                .list_documents_after_key(collection, None, &key_field, 1)
                .await;
            validate_non_atomic_repository_probe(self.backend.backend_name(), collection, probe)?;
        }
        Ok(())
    }

    /// Admission for every server-side occurrence writer. The caller must hold
    /// `transition_lock` through reconciliation, including any provider calls,
    /// so deployment and tenant retirement cannot overtake an admitted write.
    pub(crate) async fn ensure_unmaterialized_ingest_allowed_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        space_type: &str,
    ) -> Result<(), CogniGraphError> {
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        if !self.backend.supports_atomic_batches() {
            self.require_non_atomic_materialized_repair_repository_absent()
                .await?;
            return Ok(());
        }
        if !self.materialized_repair_repository_present(tenant).await? {
            return Ok(());
        }
        require_native_atomic_backend(self)?;
        let chains = self
            .validated_deployment_chains_locked(tenant, incarnation)
            .await?;
        if chains.contains_key(space_type) {
            return Err(conflict(format!(
                "space `{space_type}` is governed by an active M26 materialized generation; use a signed M26 deployment"
            )));
        }
        Ok(())
    }

    pub(crate) async fn ingest_unmaterialized_chunks(
        &self,
        backend: &dyn cognigraph_core::GraphBackend,
        scope: (&str, &str),
        space_type: &str,
        config: &SpaceType,
        chunks: &[Chunk],
        vetoes: &[VetoRule],
    ) -> Result<usize, CogniGraphError> {
        let (tenant, incarnation) = scope;
        let _guard = self.transition_lock.lock().await;
        self.ensure_unmaterialized_ingest_allowed_locked(tenant, incarnation, space_type)
            .await?;
        ingest_chunks(backend, space_type, config, chunks, vetoes).await
    }
}
