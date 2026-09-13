//! Promotion intent.

use super::*;

impl PromotionManager {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn validate_signed_promotion_intent(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: &PromotionActor,
        expected: &PromotionIntentPayload,
        authorization: &SignedPromotionIntent,
        policy_binding: Option<&PolicyGovernanceBinding>,
        accepted_at_ms: u64,
    ) -> Result<(), CogniGraphError> {
        let intent_domain = promotion_intent_domain(expected);
        validate_statement_scope(&authorization.statement, intent_domain, tenant, incarnation)?;
        if authorization.statement.payload != *expected {
            return Err(conflict(
                "stored signed promotion intent does not match its decision",
            ));
        }
        let promoter = &authorization.promoter_registration;
        self.validate_key_record(promoter, tenant, incarnation)?;
        if promoter.verification_key.purpose != KeyPurpose::Promoter
            || promoter.subject_user_key != actor.user_key
            || actor.role != "promoter"
            || promoter.principal_id != expected.promoter_principal_id
            || promoter.registration_id != expected.promoter_registration_id
            || expected.signed_at_ms < promoter.not_before_ms
            || promoter
                .not_after_ms
                .is_some_and(|until| expected.signed_at_ms >= until)
            || expected.signed_at_ms > accepted_at_ms.saturating_add(MAX_CLOCK_SKEW_MS)
        {
            return Err(conflict(
                "stored promotion signer identity or validity interval mismatch",
            ));
        }
        if let Some(binding) = policy_binding
            && (promoter.principal_id == binding.author_principal_id
                || promoter.principal_id == binding.approver_principal_id)
        {
            return Err(conflict(
                "stored promotion violates author/approver/promoter separation",
            ));
        }
        promoter
            .verification_key
            .verify(
                &authorization.statement,
                &authorization.promoter_signature,
                intent_domain,
                tenant,
                incarnation,
                KeyPurpose::Promoter,
            )
            .map_err(stored_signature_error)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn authorize_promotion_intent_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: &PromotionActor,
        expected: &PromotionIntentPayload,
        submission: PromotionIntentSubmission,
        policy_binding: Option<&PolicyGovernanceBinding>,
        at_ms: u64,
    ) -> Result<SignedPromotionIntent, CogniGraphError> {
        let intent_domain = promotion_intent_domain(expected);
        validate_statement_scope(&submission.statement, intent_domain, tenant, incarnation)?;
        if submission.statement.payload != *expected {
            return Err(CogniGraphError::Forbidden(
                "signed promotion intent does not match the exact server-derived decision input"
                    .into(),
            ));
        }
        validate_signed_at(expected.signed_at_ms, at_ms)?;
        let promoter = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &expected.promoter_registration_id,
                KeyPurpose::Promoter,
                at_ms,
            )
            .await?;
        if actor.role != "promoter"
            || actor.user_key != promoter.subject_user_key
            || expected.promoter_principal_id != promoter.principal_id
        {
            return Err(CogniGraphError::Forbidden(
                "authenticated Promoter does not own the signed promotion principal".into(),
            ));
        }
        if let Some(binding) = policy_binding
            && (promoter.principal_id == binding.author_principal_id
                || promoter.principal_id == binding.approver_principal_id)
        {
            return Err(CogniGraphError::Forbidden(
                "promoter principal must be distinct from policy author and approver".into(),
            ));
        }
        promoter
            .verification_key
            .verify(
                &submission.statement,
                &submission.promoter_signature,
                intent_domain,
                tenant,
                incarnation,
                KeyPurpose::Promoter,
            )
            .map_err(signature_error)?;
        Ok(SignedPromotionIntent {
            statement: submission.statement,
            promoter_signature: submission.promoter_signature,
            promoter_registration: promoter,
        })
    }
}
