//! Promotion.

use super::*;

impl PromotionManager {
    pub async fn promote_signed(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence_id: &str,
        actor: PromotionActor,
        idempotency_key: &str,
        authorization: crate::governance::PromotionIntentSubmission,
    ) -> Result<PromotionMutation<PromotionDecision>, CogniGraphError> {
        let payload = &authorization.statement.payload;
        if payload.requested_action != "promote" || payload.evidence_id != evidence_id {
            return Err(CogniGraphError::ValidationError(
                "signed promote intent action or evidence path mismatch".into(),
            ));
        }
        validate_reason(&payload.reason)?;
        self.decide(
            tenant,
            incarnation,
            evidence_id,
            actor,
            idempotency_key,
            PromotionAction::Promote,
            payload.reason.clone(),
            payload.expected_head_decision_id.clone(),
            payload.rollback_target_evidence_id.clone(),
            Some(authorization),
        )
        .await
    }

    pub async fn reject_signed(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence_id: &str,
        actor: PromotionActor,
        idempotency_key: &str,
        authorization: crate::governance::PromotionIntentSubmission,
    ) -> Result<PromotionMutation<PromotionDecision>, CogniGraphError> {
        let payload = &authorization.statement.payload;
        if payload.requested_action != "reject"
            || payload.evidence_id != evidence_id
            || payload.expected_head_decision_id.is_some()
            || payload.rollback_target_evidence_id.is_some()
        {
            return Err(CogniGraphError::ValidationError(
                "signed reject intent action, evidence, or head fields mismatch".into(),
            ));
        }
        validate_reason(&payload.reason)?;
        self.decide(
            tenant,
            incarnation,
            evidence_id,
            actor,
            idempotency_key,
            PromotionAction::Reject,
            payload.reason.clone(),
            None,
            None,
            Some(authorization),
        )
        .await
    }

    #[cfg(test)]
    pub async fn promote(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence_id: &str,
        actor: PromotionActor,
        idempotency_key: &str,
        request: PromoteRequest,
    ) -> Result<PromotionMutation<PromotionDecision>, CogniGraphError> {
        validate_reason(&request.reason)?;
        if let Some(id) = &request.expected_head_decision_id {
            validate_record_id("expected_head_decision_id", id)?;
        }
        self.decide(
            tenant,
            incarnation,
            evidence_id,
            actor,
            idempotency_key,
            PromotionAction::Promote,
            request.reason,
            request.expected_head_decision_id,
            None,
            None,
        )
        .await
    }
}
