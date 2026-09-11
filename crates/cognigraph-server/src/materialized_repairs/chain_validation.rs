//! Chain validation.

use super::*;

pub(super) fn validate_deployment_chain(
    chain: &mut [SemanticRepairDeploymentDecision],
) -> Result<(), CogniGraphError> {
    chain.sort_by_key(|decision| {
        (
            decision.resulting_selection.generation,
            decision.deployment_decision_id.clone(),
        )
    });
    let mut expected_generation = 1u64;
    let mut previous: Option<&SemanticRepairDeploymentDecision> = None;
    for decision in chain.iter() {
        let predecessor = previous.map(|record| record.deployment_decision_id.as_str());
        let prior_generation = previous.map(|record| record.semantic_repair_generation_id.as_str());
        if decision.resulting_selection.generation != expected_generation
            || decision.predecessor_deployment_decision_id.as_deref() != predecessor
            || decision.expected_deployment_head_decision_id.as_deref() != predecessor
            || decision
                .resulting_selection
                .prior_semantic_repair_generation_id
                .as_deref()
                != prior_generation
        {
            return Err(conflict(
                "M26 deployment decision chain is forked or non-contiguous",
            ));
        }
        match decision.action {
            SemanticRepairDeploymentAction::Activate => {
                if previous.is_none()
                    && decision
                        .deployment_intent
                        .statement
                        .payload
                        .rollback_target_generation_id
                        .is_some()
                {
                    return Err(conflict("the first M26 deployment must be an activation"));
                }
            }
            SemanticRepairDeploymentAction::Rollback => {
                let prior = previous
                    .ok_or_else(|| conflict("the first M26 deployment cannot be a rollback"))?;
                if prior
                    .resulting_selection
                    .prior_semantic_repair_generation_id
                    .as_deref()
                    != Some(decision.semantic_repair_generation_id.as_str())
                {
                    return Err(conflict(
                        "M26 rollback does not restore the retained one-step-prior generation",
                    ));
                }
            }
        }
        previous = Some(decision);
        expected_generation = expected_generation
            .checked_add(1)
            .ok_or_else(|| conflict("M26 deployment generation is exhausted"))?;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(super) fn validate_deployment_decision_against(
    decision: &SemanticRepairDeploymentDecision,
    tenant: &str,
    incarnation: &str,
    generation: &SemanticRepairGenerationRecord,
    promotion: &PromotionDecision,
    promotion_head_projection_digest: &str,
    evidence: &PromotionEvidence,
    revision: &SemanticRepairRevisionRecord,
    review: &SemanticRepairReviewRecord,
    stored_key: &GovernanceKeyRecord,
    revocation: Option<&GovernanceKeyRevocation>,
) -> Result<(), CogniGraphError> {
    let payload = &decision.deployment_intent.statement.payload;
    validate_deployment_intent_payload(payload, tenant, incarnation, decision.created_at_ms)?;
    let expected_id = new_record_id(
        tenant,
        incarnation,
        "semantic-repair-deployment",
        &decision.idempotency_key_hash,
    );
    if decision.schema_version != MATERIALIZATION_RECORD_SCHEMA_VERSION
        || decision.digest_algorithm != DIGEST_ALGORITHM
        || decision.tenant != tenant
        || decision.tenant_incarnation != incarnation
        || decision.space_type != decision.target.space_type
        || decision.target != generation.target
        || decision.deployment_decision_id != expected_id
        || decision.key != scoped_key(tenant, incarnation, "m26d", &expected_id)
        || decision.deployment_decision_digest
            != record_digest(decision, "deployment_decision_digest")?
        || decision.semantic_repair_generation_digest
            != generation.semantic_repair_generation_digest
        || decision.impact_digest != generation.impact.impact_digest
        || decision.candidate_digest != generation.candidate_digest
        || decision.promotion_head_projection_digest != payload.promotion_head_projection_digest
        || decision.promotion_head_projection_digest != promotion_head_projection_digest
        || decision.promotion_head_decision_id != payload.promotion_head_decision_id
        || decision.action != payload.requested_action
        || decision.expected_deployment_head_decision_id
            != payload.expected_deployment_head_decision_id
        || decision.predecessor_deployment_decision_id
            != decision.expected_deployment_head_decision_id
        || decision.actor.role != Role::Promoter
        || decision.reason != payload.reason
        || decision.created_at_ms == 0
        || decision.idempotency_key_hash != payload.idempotency_key_hash
        || decision.resulting_selection.semantic_repair_generation_id
            != generation.semantic_repair_generation_id
        || decision
            .resulting_selection
            .semantic_repair_generation_digest
            != generation.semantic_repair_generation_digest
        || decision.resulting_selection.target != generation.target
        || decision.resulting_selection.candidate_digest != generation.candidate_digest
        || decision.resulting_selection.generation == 0
        || promotion.target != generation.target
        || promotion.evidence_id != evidence.id
        || promotion.evidence_digest != evidence.evidence_digest
        || promotion
            .resulting_selection
            .as_ref()
            .is_none_or(|selection| {
                selection.evidence_id != evidence.id
                    || selection.candidate_digest != generation.candidate_digest
            })
        || revision.semantic_repair_revision_id != generation.semantic_repair_revision_id
        || revision.semantic_repair_revision_digest != generation.semantic_repair_revision_digest
        || review.semantic_repair_review_id != generation.semantic_repair_review_id
        || review.semantic_repair_review_digest != generation.semantic_repair_review_digest
        || payload.target != generation.target
        || payload.semantic_repair_generation_id != generation.semantic_repair_generation_id
        || payload.semantic_repair_generation_digest != generation.semantic_repair_generation_digest
        || payload.impact_digest != generation.impact.impact_digest
        || payload.candidate_digest != generation.candidate_digest
        || payload.semantic_repair_revision_id != generation.semantic_repair_revision_id
        || payload.semantic_repair_revision_digest != generation.semantic_repair_revision_digest
        || payload.semantic_repair_review_id != generation.semantic_repair_review_id
        || payload.semantic_repair_review_digest != generation.semantic_repair_review_digest
        || decision.deployment_intent.statement.schema_version
            != cognigraph_governance::GOVERNANCE_SCHEMA_VERSION
        || decision.deployment_intent.statement.domain != SEMANTIC_REPAIR_DEPLOYMENT_INTENT_DOMAIN
        || decision.deployment_intent.statement.tenant != tenant
        || decision.deployment_intent.statement.tenant_incarnation != incarnation
        || stored_key != &decision.deployment_intent.promoter_registration
        || stored_key.principal_id != payload.promoter_principal_id
        || stored_key.subject_user_key != decision.actor.user_key
    {
        return Err(conflict("stored M26 deployment decision is malformed"));
    }
    match decision.action {
        SemanticRepairDeploymentAction::Activate => {
            if payload.rollback_target_generation_id.is_some()
                || (decision.predecessor_deployment_decision_id.is_some()
                    && promotion.action == PromotionAction::Rollback)
            {
                return Err(conflict(
                    "stored M26 activation conflicts with rollback authority",
                ));
            }
        }
        SemanticRepairDeploymentAction::Rollback => {
            if payload.rollback_target_generation_id.as_deref()
                != Some(generation.semantic_repair_generation_id.as_str())
                || decision.predecessor_deployment_decision_id.is_none()
                || promotion.action != PromotionAction::Rollback
            {
                return Err(conflict(
                    "stored M26 rollback does not match signed promotion rollback authority",
                ));
            }
        }
    }
    validate_historical_key_use(
        stored_key,
        KeyPurpose::Promoter,
        payload.signed_at_ms,
        decision.created_at_ms,
        revocation,
    )?;
    stored_key
        .verification_key
        .verify(
            &decision.deployment_intent.statement,
            &decision.deployment_intent.promoter_signature,
            SEMANTIC_REPAIR_DEPLOYMENT_INTENT_DOMAIN,
            tenant,
            incarnation,
            KeyPurpose::Promoter,
        )
        .map_err(|error| {
            conflict(format!(
                "stored M26 deployment signature is invalid: {error}"
            ))
        })?;
    if stored_key.principal_id == revision.author_principal_id
        || stored_key.principal_id == review.approver_principal_id
        || evidence
            .artifact_attestations
            .as_ref()
            .is_some_and(|authority| {
                authority
                    .candidate
                    .attestor_principals()
                    .contains(stored_key.principal_id.as_str())
                    || authority
                        .baseline
                        .attestor_principals()
                        .contains(stored_key.principal_id.as_str())
            })
    {
        return Err(conflict(
            "stored M26 deployment violates principal separation",
        ));
    }
    let expected_request_digest = canonical_digest(&json!({
        "actor": decision.actor,
        "authorization": SemanticRepairDeploymentIntentSubmission {
            statement: decision.deployment_intent.statement.clone(),
            promoter_signature: decision.deployment_intent.promoter_signature.clone(),
        },
    }))?;
    if decision.request_digest != expected_request_digest {
        return Err(conflict(
            "stored M26 deployment request binding is malformed",
        ));
    }
    Ok(())
}
