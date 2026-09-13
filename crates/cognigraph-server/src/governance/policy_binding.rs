//! Policy binding.

use super::*;

impl PromotionManager {
    pub async fn validate_policy_binding_active(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        policy: &ResolvedPromotionPolicy,
        binding: &PolicyGovernanceBinding,
    ) -> Result<(), CogniGraphError> {
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.validate_policy_binding_locked(
            tenant,
            incarnation,
            target,
            policy,
            binding,
            now_millis(),
        )
        .await
    }

    pub(crate) async fn validate_policy_binding_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        policy: &ResolvedPromotionPolicy,
        binding: &PolicyGovernanceBinding,
        at_ms: u64,
    ) -> Result<(), CogniGraphError> {
        binding.validate()?;
        let root = self.root_key()?;
        if binding.root_key_id != root.key_id {
            return Err(conflict("policy binding names another governance root"));
        }
        let revision = self
            .get_policy_locked(tenant, incarnation, &binding.policy_revision_id)
            .await?;
        let approval = self
            .get_approval_locked(tenant, incarnation, &binding.approval_id)
            .await?;
        if revision.policy_revision_digest != binding.policy_revision_digest
            || revision.resolved_policy_digest != binding.resolved_policy_digest
            || revision.resolved_policy != *policy
            || revision.target != *target
            || revision.author_registration_id != binding.author_registration_id
            || revision.author_registration_digest != binding.author_registration_digest
            || revision.author_principal_id != binding.author_principal_id
            || approval.approval_digest != binding.approval_digest
            || approval.policy_revision_id != revision.policy_revision_id
            || approval.policy_revision_digest != revision.policy_revision_digest
            || approval.approver_registration_id != binding.approver_registration_id
            || approval.approver_registration_digest != binding.approver_registration_digest
            || approval.approver_principal_id != binding.approver_principal_id
        {
            return Err(conflict(
                "promotion context governance binding does not match signed authority",
            ));
        }
        self.get_active_key_locked(
            tenant,
            incarnation,
            &binding.author_registration_id,
            KeyPurpose::PolicyAuthor,
            at_ms,
        )
        .await?;
        self.get_active_key_locked(
            tenant,
            incarnation,
            &binding.approver_registration_id,
            KeyPurpose::PolicyApprover,
            at_ms,
        )
        .await?;
        Ok(())
    }

    pub async fn policy_binding(
        &self,
        tenant: &str,
        incarnation: &str,
        approval_id: &str,
    ) -> Result<PolicyGovernanceBinding, CogniGraphError> {
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        let approval = self
            .get_approval_locked(tenant, incarnation, approval_id)
            .await?;
        let revision = self
            .get_policy_locked(tenant, incarnation, &approval.policy_revision_id)
            .await?;
        let root = self.root_key()?;
        let target = revision.target.clone();
        let resolved_policy = revision.resolved_policy.clone();
        let binding = PolicyGovernanceBinding {
            root_key_id: root.key_id,
            author_registration_id: revision.author_registration_id,
            author_registration_digest: revision.author_registration_digest,
            policy_revision_id: revision.policy_revision_id,
            policy_revision_digest: revision.policy_revision_digest,
            resolved_policy_digest: revision.resolved_policy_digest,
            approval_id: approval.approval_id,
            approval_digest: approval.approval_digest,
            approver_registration_id: approval.approver_registration_id,
            approver_registration_digest: approval.approver_registration_digest,
            author_principal_id: revision.author_principal_id,
            approver_principal_id: approval.approver_principal_id,
        };
        self.validate_policy_binding_locked(
            tenant,
            incarnation,
            &target,
            &resolved_policy,
            &binding,
            now_millis(),
        )
        .await?;
        Ok(binding)
    }
}
