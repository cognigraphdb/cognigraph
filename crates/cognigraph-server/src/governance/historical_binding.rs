//! Historical binding.

use super::*;

impl PromotionManager {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_binding_against_authority(
        &self,
        target: &PromotionTarget,
        resolved_policy: &ResolvedPromotionPolicy,
        binding: &PolicyGovernanceBinding,
        accepted_at_ms: u64,
        authority: &GovernanceAuthority,
    ) -> Result<(), CogniGraphError> {
        binding.validate()?;
        let root = self.root_key()?;
        let policy = authority
            .policies
            .get(&binding.policy_revision_id)
            .ok_or_else(|| conflict("promotion binding references a missing policy revision"))?;
        let approval = authority
            .approvals
            .get(&binding.approval_id)
            .ok_or_else(|| conflict("promotion binding references a missing policy approval"))?;
        let author = authority
            .keys
            .get(&binding.author_registration_id)
            .ok_or_else(|| conflict("promotion binding references a missing author key"))?;
        let approver = authority
            .keys
            .get(&binding.approver_registration_id)
            .ok_or_else(|| conflict("promotion binding references a missing approver key"))?;
        if binding.root_key_id != root.key_id
            || policy.created_at_ms > accepted_at_ms
            || approval.approved_at_ms > accepted_at_ms
            || policy.policy_revision_digest != binding.policy_revision_digest
            || policy.resolved_policy_digest != binding.resolved_policy_digest
            || policy.resolved_policy != *resolved_policy
            || policy.target != *target
            || policy.author_registration_id != binding.author_registration_id
            || policy.author_registration_digest != binding.author_registration_digest
            || policy.author_principal_id != binding.author_principal_id
            || approval.approval_digest != binding.approval_digest
            || approval.policy_revision_id != policy.policy_revision_id
            || approval.policy_revision_digest != policy.policy_revision_digest
            || approval.approver_registration_id != binding.approver_registration_id
            || approval.approver_registration_digest != binding.approver_registration_digest
            || approval.approver_principal_id != binding.approver_principal_id
        {
            return Err(conflict(
                "promotion binding does not match its historical signed policy authority",
            ));
        }
        validate_historical_key_use(
            author,
            KeyPurpose::PolicyAuthor,
            accepted_at_ms,
            accepted_at_ms,
            authority.revocations.get(&author.registration_id),
        )?;
        validate_historical_key_use(
            approver,
            KeyPurpose::PolicyApprover,
            accepted_at_ms,
            accepted_at_ms,
            authority.revocations.get(&approver.registration_id),
        )
    }
}
