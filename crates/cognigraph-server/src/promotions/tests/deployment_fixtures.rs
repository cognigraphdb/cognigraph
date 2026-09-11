//! Deployment fixtures.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn signed_semantic_repair_deployment_intent(
    principal: &TestGovernancePrincipal,
    generation: &SemanticRepairGenerationRecord,
    promotion_head: &PromotionHead,
    action: SemanticRepairDeploymentAction,
    expected_deployment_head_decision_id: Option<&str>,
    rollback_target_generation_id: Option<&str>,
    idempotency_key: &str,
    reason: &str,
) -> SemanticRepairDeploymentIntentSubmission {
    let statement = GovernanceStatement::new(
        SEMANTIC_REPAIR_DEPLOYMENT_INTENT_DOMAIN,
        TENANT,
        INCARNATION,
        SemanticRepairDeploymentIntentPayload {
            requested_action: action,
            target: generation.target.clone(),
            semantic_repair_generation_id: generation.semantic_repair_generation_id.clone(),
            semantic_repair_generation_digest: generation.semantic_repair_generation_digest.clone(),
            impact_digest: generation.impact.impact_digest.clone(),
            promotion_head_decision_id: promotion_head.applied_decision_id.clone(),
            promotion_head_projection_digest: promotion_head.projection_digest.clone(),
            candidate_digest: generation.candidate_digest.clone(),
            semantic_repair_revision_id: generation.semantic_repair_revision_id.clone(),
            semantic_repair_revision_digest: generation.semantic_repair_revision_digest.clone(),
            semantic_repair_review_id: generation.semantic_repair_review_id.clone(),
            semantic_repair_review_digest: generation.semantic_repair_review_digest.clone(),
            expected_deployment_head_decision_id: expected_deployment_head_decision_id
                .map(str::to_owned),
            rollback_target_generation_id: rollback_target_generation_id.map(str::to_owned),
            reason: reason.into(),
            idempotency_key_hash: digest_bytes(idempotency_key.as_bytes()),
            promoter_registration_id: principal.record.registration_id.clone(),
            promoter_principal_id: principal.record.principal_id.clone(),
            signed_at_ms: now_millis(),
        },
    );
    SemanticRepairDeploymentIntentSubmission {
        promoter_signature: principal.signing_key.sign(&statement).unwrap(),
        statement,
    }
}
