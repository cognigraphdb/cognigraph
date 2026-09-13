//! Policy validation.

use super::*;

impl PromotionManager {
    pub(super) async fn validate_policy_record(
        &self,
        record: &PolicyRevisionRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let author = self
            .get_key_locked(tenant, incarnation, &record.author_registration_id)
            .await?;
        self.validate_policy_record_against(record, &author, tenant, incarnation)
    }

    pub(super) async fn validate_stored_policy_record(
        &self,
        record: &PolicyRevisionRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_policy_record(record, tenant, incarnation)
            .await
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(super) fn validate_policy_record_against(
        &self,
        record: &PolicyRevisionRecord,
        author: &GovernanceKeyRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        if record.schema_version != GOVERNANCE_RECORD_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key != scoped_key(tenant, incarnation, "pr", &record.policy_revision_id)
            || record.policy_revision_digest != record_digest(record, "policy_revision_digest")?
            || record.resolved_policy_digest != canonical_digest(&record.resolved_policy)?
            || record.policy_revision_id
                != policy_revision_id(tenant, incarnation, &record.target, &record.resolved_policy)?
        {
            return Err(conflict("malformed or foreign signed policy revision"));
        }
        record.resolved_policy.validate()?;
        record.created_by.require_role(Role::PolicyAuthor)?;
        validate_historical_key_use(
            author,
            KeyPurpose::PolicyAuthor,
            record.signed_at_ms,
            record.created_at_ms,
            None,
        )?;
        if author.registration_digest != record.author_registration_digest
            || author.principal_id != record.author_principal_id
            || author.subject_user_key != record.created_by.user_key
            || author.verification_key.purpose != KeyPurpose::PolicyAuthor
        {
            return Err(conflict("policy revision author authority mismatch"));
        }
        let statement = GovernanceStatement::new(
            POLICY_REVISION_DOMAIN,
            tenant,
            incarnation,
            PolicyRevisionPayload {
                policy_revision_id: record.policy_revision_id.clone(),
                target: record.target.clone(),
                resolved_policy: record.resolved_policy.clone(),
                resolved_policy_digest: record.resolved_policy_digest.clone(),
                author_registration_id: record.author_registration_id.clone(),
                author_principal_id: record.author_principal_id.clone(),
                signed_at_ms: record.signed_at_ms,
            },
        );
        author
            .verification_key
            .verify(
                &statement,
                &record.author_signature,
                POLICY_REVISION_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyAuthor,
            )
            .map_err(stored_signature_error)?;
        let expected_request_digest = canonical_digest(&CreatePolicyRevisionRequest {
            statement,
            author_signature: record.author_signature.clone(),
        })?;
        if !is_digest(&record.idempotency_key_hash)
            || record.request_digest != expected_request_digest
        {
            return Err(conflict("signed policy request authority mismatch"));
        }
        Ok(())
    }

    pub(super) async fn validate_approval_record(
        &self,
        record: &PolicyApprovalRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let policy = self
            .get_policy_locked(tenant, incarnation, &record.policy_revision_id)
            .await?;
        let approver = self
            .get_key_locked(tenant, incarnation, &record.approver_registration_id)
            .await?;
        self.validate_approval_record_against(record, &policy, &approver, tenant, incarnation)
    }

    pub(super) async fn validate_stored_approval_record(
        &self,
        record: &PolicyApprovalRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_approval_record(record, tenant, incarnation)
            .await
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(super) fn validate_approval_record_against(
        &self,
        record: &PolicyApprovalRecord,
        policy: &PolicyRevisionRecord,
        approver: &GovernanceKeyRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        if record.schema_version != GOVERNANCE_RECORD_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key != scoped_key(tenant, incarnation, "pa", &record.approval_id)
            || record.approval_digest != record_digest(record, "approval_digest")?
            || record.decision != "approve"
            || record.approval_id != approval_id(tenant, incarnation, &record.policy_revision_id)
        {
            return Err(conflict("malformed or foreign signed policy approval"));
        }
        record.approved_by.require_role(Role::PolicyApprover)?;
        validate_reason(&record.reason)?;
        validate_historical_key_use(
            approver,
            KeyPurpose::PolicyApprover,
            record.signed_at_ms,
            record.approved_at_ms,
            None,
        )?;
        if policy.policy_revision_digest != record.policy_revision_digest
            || policy.target != record.target
            || policy.resolved_policy_digest != record.resolved_policy_digest
            || policy.author_principal_id != record.author_principal_id
            || approver.registration_digest != record.approver_registration_digest
            || approver.principal_id != record.approver_principal_id
            || approver.subject_user_key != record.approved_by.user_key
            || approver.verification_key.purpose != KeyPurpose::PolicyApprover
            || approver.principal_id == policy.author_principal_id
        {
            return Err(conflict("policy approval authority mismatch"));
        }
        let statement = GovernanceStatement::new(
            POLICY_APPROVAL_DOMAIN,
            tenant,
            incarnation,
            PolicyApprovalPayload {
                approval_id: record.approval_id.clone(),
                policy_revision_id: record.policy_revision_id.clone(),
                policy_revision_digest: record.policy_revision_digest.clone(),
                target: record.target.clone(),
                resolved_policy_digest: record.resolved_policy_digest.clone(),
                author_principal_id: record.author_principal_id.clone(),
                approver_registration_id: record.approver_registration_id.clone(),
                approver_principal_id: record.approver_principal_id.clone(),
                decision: record.decision.clone(),
                reason: record.reason.clone(),
                signed_at_ms: record.signed_at_ms,
            },
        );
        approver
            .verification_key
            .verify(
                &statement,
                &record.approval_signature,
                POLICY_APPROVAL_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyApprover,
            )
            .map_err(stored_signature_error)?;
        let expected_request_digest = canonical_digest(&ApprovePolicyRevisionRequest {
            statement,
            approval_signature: record.approval_signature.clone(),
        })?;
        if !is_digest(&record.idempotency_key_hash)
            || record.request_digest != expected_request_digest
        {
            return Err(conflict("signed approval request authority mismatch"));
        }
        Ok(())
    }
}
