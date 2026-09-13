//! Deployment queries.

use super::*;

impl PromotionManager {
    pub async fn current_semantic_repair_deployment(
        &self,
        tenant: &str,
        incarnation: &str,
        space_type: &str,
    ) -> Result<Option<SemanticRepairDeploymentHead>, CogniGraphError> {
        require_native_atomic_backend(self)?;
        self.ensure_materialized_repair_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.current_semantic_repair_deployment_locked(tenant, incarnation, space_type)
            .await
    }

    pub(super) async fn current_semantic_repair_deployment_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        space_type: &str,
    ) -> Result<Option<SemanticRepairDeploymentHead>, CogniGraphError> {
        let key = deployment_head_key(tenant, incarnation, space_type)?;
        let stored = self
            .get_authority_raw::<SemanticRepairDeploymentHead>(
                tenant,
                SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION,
                &key,
            )
            .await?;
        let chains = self
            .validated_deployment_chains_locked(tenant, incarnation)
            .await?;
        let Some(decision) = chains.get(space_type).and_then(|chain| chain.last()) else {
            if stored.is_some() {
                return Err(conflict("M26 deployment head has no immutable authority"));
            }
            return Ok(None);
        };
        let expected = self.deployment_head_from_decision(decision)?;
        let head = stored.ok_or_else(|| {
            conflict("M26 deployment head is missing for the immutable decision authority")
        })?;
        self.validate_deployment_head(&head, tenant, incarnation, space_type)?;
        if head != expected {
            return Err(conflict(
                "M26 deployment head differs from immutable decision authority",
            ));
        }
        let generation = self
            .get_semantic_repair_generation_locked(
                tenant,
                incarnation,
                &head.selection.semantic_repair_generation_id,
            )
            .await?;
        if !self
            .active_target_matches_projection(space_type, &generation.projection)
            .await?
        {
            return Err(conflict(
                "M26 active target rows differ from the deployed immutable generation",
            ));
        }
        Ok(Some(head))
    }

    pub(super) async fn get_semantic_repair_generation_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<SemanticRepairGenerationRecord, CogniGraphError> {
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

    pub(super) async fn validated_generation_set_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<BTreeMap<String, SemanticRepairGenerationRecord>, CogniGraphError> {
        let records = self
            .all_semantic_repair_generations_locked(tenant, incarnation)
            .await?;
        let mut by_id = BTreeMap::new();
        for record in records {
            self.validate_stored_semantic_repair_generation(&record, tenant, incarnation)
                .await?;
            if by_id
                .insert(record.semantic_repair_generation_id.clone(), record)
                .is_some()
            {
                return Err(conflict("M26 duplicate generation identity is stored"));
            }
        }
        validate_generation_capacity_set(&by_id)?;
        Ok(by_id)
    }

    pub(super) async fn scoped_deployment_head_values(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<BTreeMap<String, Value>, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, "m26h", "");
        let keys = self
            .scoped_record_keys(
                SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION,
                &prefix,
                None,
                MAX_M26_DEPLOYMENT_SPACES_PER_TENANT + 1,
            )
            .await?;
        if keys.len() > MAX_M26_DEPLOYMENT_SPACES_PER_TENANT {
            return Err(conflict("M26 deployment head safety bound is exceeded"));
        }
        let mut values = BTreeMap::new();
        for key in keys {
            let mut value = self
                .backend
                .get_document(SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION, &key)
                .await?
                .ok_or_else(|| conflict("M26 deployment head vanished during validation"))?;
            strip_backend_metadata(&mut value);
            values.insert(key, value);
        }
        Ok(values)
    }
}
