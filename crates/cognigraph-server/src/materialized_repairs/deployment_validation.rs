//! Deployment validation.

use super::*;

impl PromotionManager {
    pub(super) async fn validate_stored_deployment_decision(
        &self,
        decision: &SemanticRepairDeploymentDecision,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let result = async {
            let generation = self
                .get_semantic_repair_generation_locked(
                    tenant,
                    incarnation,
                    &decision.semantic_repair_generation_id,
                )
                .await?;
            let promotion = self
                .get_decision(tenant, incarnation, &decision.promotion_head_decision_id)
                .await?;
            let promotion_selection = promotion
                .resulting_selection
                .as_ref()
                .ok_or_else(|| conflict("M26 deployment promotion has no selection"))?;
            let promotion_head = self.head_from_decision(&promotion, promotion_selection)?;
            let evidence = self
                .get_evidence(tenant, incarnation, &generation.source_evidence_id)
                .await?;
            let payload = &decision.deployment_intent.statement.payload;
            let stored_key = self
                .get_key_locked(tenant, incarnation, &payload.promoter_registration_id)
                .await?;
            if stored_key != decision.deployment_intent.promoter_registration
                || stored_key.principal_id != payload.promoter_principal_id
                || stored_key.subject_user_key != decision.actor.user_key
            {
                return Err(conflict(
                    "stored M26 deployment signer registration is malformed",
                ));
            }
            let revocation_id =
                key_revocation_id(tenant, incarnation, &payload.promoter_registration_id);
            let revocation: Option<GovernanceKeyRevocation> = match self
                .get_governance_revocation(tenant, incarnation, &revocation_id)
                .await
            {
                Ok(record) => Some(record),
                Err(CogniGraphError::DocumentNotFound { .. }) => None,
                Err(error) => return Err(error),
            };
            let revision = self
                .get_semantic_repair_revision(
                    tenant,
                    incarnation,
                    &generation.semantic_repair_revision_id,
                )
                .await?;
            let review = self
                .get_semantic_repair_review(
                    tenant,
                    incarnation,
                    &generation.semantic_repair_review_id,
                )
                .await?;
            validate_deployment_decision_against(
                decision,
                tenant,
                incarnation,
                &generation,
                &promotion,
                &promotion_head.projection_digest,
                &evidence,
                &revision,
                &review,
                &stored_key,
                revocation.as_ref(),
            )
        }
        .await;
        result.inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(super) fn validate_deployment_head(
        &self,
        head: &SemanticRepairDeploymentHead,
        tenant: &str,
        incarnation: &str,
        space_type: &str,
    ) -> Result<(), CogniGraphError> {
        if head.schema_version != MATERIALIZATION_RECORD_SCHEMA_VERSION
            || head.digest_algorithm != DIGEST_ALGORITHM
            || head.tenant != tenant
            || head.tenant_incarnation != incarnation
            || head.space_type != space_type
            || head.key != deployment_head_key(tenant, incarnation, space_type)?
            || head.selection.generation == 0
            || head.selection.target.space_type != space_type
            || head.updated_at_ms == 0
            || head.projection_digest != record_digest(head, "projection_digest")?
        {
            return Err(conflict("M26 deployment head projection is malformed"));
        }
        validate_record_id(
            "applied_deployment_decision_id",
            &head.applied_deployment_decision_id,
        )?;
        Ok(())
    }
}
