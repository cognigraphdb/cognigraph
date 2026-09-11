//! Status.

use super::*;

impl PromotionManager {
    pub(crate) async fn materialized_repair_status_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<MaterializedRepairStatus, CogniGraphError> {
        if !self.backend.supports_atomic_batches() {
            self.require_non_atomic_materialized_repair_repository_absent()
                .await?;
            return Ok(MaterializedRepairStatus {
                enabled: false,
                generations: 0,
                deployment_decisions: 0,
                active_heads: 0,
                repair_required: false,
                full_validation_performed: true,
                native_atomic_only: true,
            });
        }
        if !self.materialized_repair_repository_present(tenant).await? {
            return Ok(MaterializedRepairStatus {
                enabled: false,
                generations: 0,
                deployment_decisions: 0,
                active_heads: 0,
                repair_required: false,
                full_validation_performed: true,
                native_atomic_only: true,
            });
        }
        require_native_atomic_backend(self)?;
        let generations = self
            .validated_generation_set_locked(tenant, incarnation)
            .await?;
        let chains = self
            .validated_deployment_chains_locked(tenant, incarnation)
            .await?;
        if !chains.is_empty() {
            self.require_collection_types(&[
                ("entities", CollectionType::Document),
                ("chunks", CollectionType::Document),
                ("mentions", CollectionType::Edge),
                ("facts", CollectionType::Edge),
            ])
            .await?;
        }
        let heads = self
            .scoped_deployment_head_values(tenant, incarnation)
            .await?;
        let mut expected_head_keys = BTreeSet::new();
        let mut repair_required = false;
        for (space_type, chain) in &chains {
            let decision = chain
                .last()
                .expect("M26 validated deployment chain is non-empty");
            let expected = self.deployment_head_from_decision(decision)?;
            expected_head_keys.insert(expected.key.clone());
            let stored = heads.get(&expected.key).and_then(|value| {
                serde_json::from_value::<SemanticRepairDeploymentHead>(value.clone()).ok()
            });
            repair_required |= stored.as_ref() != Some(&expected);
            let generation = generations
                .get(&decision.semantic_repair_generation_id)
                .ok_or_else(|| conflict("M26 deployment references a missing generation"))?;
            repair_required |= !self
                .active_target_matches_projection(space_type, &generation.projection)
                .await?;
        }
        repair_required |= heads.keys().any(|key| !expected_head_keys.contains(key));
        self.record_m26_reconciliation(0);
        Ok(MaterializedRepairStatus {
            enabled: true,
            generations: generations.len(),
            deployment_decisions: chains.values().map(Vec::len).sum(),
            active_heads: heads.len(),
            repair_required,
            full_validation_performed: true,
            native_atomic_only: true,
        })
    }
}
