//! Intent fixtures.

use super::*;

pub(super) fn promoter_actor(principal: &TestGovernancePrincipal) -> PromotionActor {
    PromotionActor {
        user_key: principal.actor.user_key.clone(),
        username: principal.actor.username.clone(),
        role: "promoter".into(),
    }
}
pub(super) fn signed_promote_intent(
    principal: &TestGovernancePrincipal,
    evidence: &PromotionEvidence,
    idempotency_key: &str,
    reason: &str,
) -> PromotionIntentSubmission {
    let binding = evidence.governance.as_ref().unwrap();
    let statement = GovernanceStatement::new(
        if evidence
            .artifact_consumption
            .as_ref()
            .and_then(|authority| authority.derivation.as_ref())
            .and_then(|authority| authority.preparation.as_ref())
            .is_some()
        {
            M23_PROMOTION_INTENT_DOMAIN
        } else if evidence
            .artifact_consumption
            .as_ref()
            .and_then(|authority| authority.derivation.as_ref())
            .is_some()
        {
            M22_PROMOTION_INTENT_DOMAIN
        } else if evidence.artifact_consumption.is_some() {
            M21_PROMOTION_INTENT_DOMAIN
        } else {
            PROMOTION_INTENT_DOMAIN
        },
        TENANT,
        INCARNATION,
        PromotionIntentPayload {
            requested_action: "promote".into(),
            target: evidence.target.clone(),
            evidence_id: evidence.id.clone(),
            evidence_digest: evidence.evidence_digest.clone(),
            policy_revision_id: Some(binding.policy_revision_id.clone()),
            policy_revision_digest: Some(binding.policy_revision_digest.clone()),
            approval_id: Some(binding.approval_id.clone()),
            approval_digest: Some(binding.approval_digest.clone()),
            artifact_authority_digest: evidence
                .artifact_attestations
                .as_ref()
                .map(|authority| authority.authority_digest.clone()),
            consumption_authority_digest: evidence
                .artifact_consumption
                .as_ref()
                .map(|authority| authority.authority_digest.clone()),
            derivation_authority_digest: evidence
                .artifact_consumption
                .as_ref()
                .and_then(|authority| authority.derivation.as_ref())
                .map(|authority| authority.authority_digest.clone()),
            preparation_authority_digest: evidence
                .artifact_consumption
                .as_ref()
                .and_then(|authority| authority.derivation.as_ref())
                .and_then(|authority| authority.preparation.as_ref())
                .map(|authority| authority.authority_digest.clone()),
            gate_assessment_digest: canonical_digest(&evidence.gates).unwrap(),
            expected_head_decision_id: evidence.expected_head_decision_id.clone(),
            rollback_target_evidence_id: evidence.rollback_target_evidence_id.clone(),
            reason: reason.into(),
            idempotency_key_hash: digest_bytes(idempotency_key.as_bytes()),
            promoter_registration_id: principal.record.registration_id.clone(),
            promoter_principal_id: principal.record.principal_id.clone(),
            signed_at_ms: now_millis(),
        },
    );
    PromotionIntentSubmission {
        promoter_signature: principal.signing_key.sign(&statement).unwrap(),
        statement,
    }
}
pub(super) fn signed_reject_intent(
    principal: &TestGovernancePrincipal,
    evidence: &PromotionEvidence,
    idempotency_key: &str,
    reason: &str,
) -> PromotionIntentSubmission {
    let mut submission = signed_promote_intent(principal, evidence, idempotency_key, reason);
    submission.statement.payload.requested_action = "reject".into();
    submission.statement.payload.expected_head_decision_id = None;
    submission.statement.payload.rollback_target_evidence_id = None;
    submission.promoter_signature = principal.signing_key.sign(&submission.statement).unwrap();
    submission
}
pub(super) fn signed_rollback_intent(
    principal: &TestGovernancePrincipal,
    evidence: &PromotionEvidence,
    expected_head_decision_id: &str,
    idempotency_key: &str,
    reason: &str,
) -> PromotionIntentSubmission {
    let mut submission = signed_promote_intent(principal, evidence, idempotency_key, reason);
    submission.statement.payload.requested_action = "rollback".into();
    submission.statement.payload.expected_head_decision_id = Some(expected_head_decision_id.into());
    submission.statement.payload.rollback_target_evidence_id = Some(evidence.id.clone());
    submission.promoter_signature = principal.signing_key.sign(&submission.statement).unwrap();
    submission
}
