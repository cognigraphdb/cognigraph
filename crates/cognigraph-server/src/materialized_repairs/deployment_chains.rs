//! Deployment chains.

use super::*;

impl PromotionManager {
    pub(super) async fn all_deployment_decisions_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<Vec<SemanticRepairDeploymentDecision>, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, "m26d", "");
        let keys = self
            .scoped_record_keys(
                SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
                &prefix,
                None,
                MAX_M26_DEPLOYMENT_DECISIONS_PER_TENANT + 1,
            )
            .await?;
        if keys.len() > MAX_M26_DEPLOYMENT_DECISIONS_PER_TENANT {
            return Err(conflict(
                "M26 tenant deployment-decision safety bound is exceeded",
            ));
        }
        let mut records = Vec::with_capacity(keys.len());
        for key in keys {
            records.push(
                self.get_authority_raw::<SemanticRepairDeploymentDecision>(
                    tenant,
                    SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
                    &key,
                )
                .await?
                .ok_or_else(|| conflict("M26 deployment decision vanished during validation"))?,
            );
        }
        Ok(records)
    }

    pub(super) async fn validated_deployment_chains_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<BTreeMap<String, Vec<SemanticRepairDeploymentDecision>>, CogniGraphError> {
        let records = self
            .all_deployment_decisions_locked(tenant, incarnation)
            .await?;
        let mut chains = BTreeMap::<String, Vec<SemanticRepairDeploymentDecision>>::new();
        for record in records {
            self.validate_stored_deployment_decision(&record, tenant, incarnation)
                .await?;
            chains
                .entry(record.space_type.clone())
                .or_default()
                .push(record);
        }
        validate_deployment_chain_capacities(
            chains
                .iter()
                .map(|(space_type, chain)| (space_type.as_str(), chain.len())),
        )
        .map_err(|violation| match violation {
            DeploymentChainCapacityViolation::SpaceCount => {
                conflict("M26 deployment space safety bound is exceeded")
            }
            DeploymentChainCapacityViolation::DecisionCount => {
                conflict("M26 deployment decision quota is exceeded")
            }
        })?;
        for chain in chains.values_mut() {
            validate_deployment_chain(chain)?;
        }
        Ok(chains)
    }

    pub(super) fn deployment_head_from_decision(
        &self,
        decision: &SemanticRepairDeploymentDecision,
    ) -> Result<SemanticRepairDeploymentHead, CogniGraphError> {
        let mut head = SemanticRepairDeploymentHead {
            key: deployment_head_key(
                &decision.tenant,
                &decision.tenant_incarnation,
                &decision.space_type,
            )?,
            schema_version: MATERIALIZATION_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: decision.tenant.clone(),
            tenant_incarnation: decision.tenant_incarnation.clone(),
            space_type: decision.space_type.clone(),
            applied_deployment_decision_id: decision.deployment_decision_id.clone(),
            selection: decision.resulting_selection.clone(),
            updated_at_ms: decision.created_at_ms,
            projection_digest: String::new(),
        };
        head.projection_digest = record_digest(&head, "projection_digest")?;
        Ok(head)
    }
}
