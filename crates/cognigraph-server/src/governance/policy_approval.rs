//! Policy approval.

use super::*;

impl PromotionManager {
    pub async fn approve_policy_revision(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        request: ApprovePolicyRevisionRequest,
    ) -> Result<GovernanceMutation<PolicyApprovalRecord>, CogniGraphError> {
        actor.require_role(Role::PolicyApprover)?;
        validate_idempotency_key(idempotency_key)?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        let now = now_millis();
        validate_statement_scope(
            &request.statement,
            POLICY_APPROVAL_DOMAIN,
            tenant,
            incarnation,
        )?;
        let payload = &request.statement.payload;
        validate_identifier("approval_id", &payload.approval_id)?;
        validate_identifier("policy_revision_id", &payload.policy_revision_id)?;
        validate_identifier(
            "approver_registration_id",
            &payload.approver_registration_id,
        )?;
        validate_identifier("approver_principal_id", &payload.approver_principal_id)?;
        validate_reason(&payload.reason)?;
        validate_signed_at(payload.signed_at_ms, now)?;
        if payload.decision != "approve"
            || payload.approval_id != approval_id(tenant, incarnation, &payload.policy_revision_id)
        {
            return Err(validation(
                "policy approval must be an exact approve decision with its natural identity",
            ));
        }
        let policy = self
            .get_policy_locked(tenant, incarnation, &payload.policy_revision_id)
            .await?;
        if payload.policy_revision_digest != policy.policy_revision_digest
            || payload.target != policy.target
            || payload.resolved_policy_digest != policy.resolved_policy_digest
            || payload.author_principal_id != policy.author_principal_id
        {
            return Err(validation(
                "policy approval does not bind the exact policy revision",
            ));
        }
        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let key = scoped_key(tenant, incarnation, "pa", &payload.approval_id);
        if let Some(existing) = self
            .get_authority_raw::<PolicyApprovalRecord>(tenant, POLICY_APPROVALS_COLLECTION, &key)
            .await?
        {
            self.validate_stored_approval_record(&existing, tenant, incarnation)
                .await?;
            if existing.idempotency_key_hash == key_hash
                && existing.request_digest == request_digest
                && existing.approved_by == actor
            {
                return Ok(GovernanceMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "policy revision already has a different approval authority",
            ));
        }
        let author = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &policy.author_registration_id,
                KeyPurpose::PolicyAuthor,
                now,
            )
            .await?;
        if author.registration_digest != policy.author_registration_digest
            || author.principal_id != policy.author_principal_id
        {
            return Err(conflict(
                "policy approval references different author authority",
            ));
        }
        let approver = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &payload.approver_registration_id,
                KeyPurpose::PolicyApprover,
                now,
            )
            .await?;
        require_key_actor(&approver, &actor, &payload.approver_principal_id)?;
        if approver.principal_id == policy.author_principal_id {
            return Err(CogniGraphError::Forbidden(
                "policy author and approver principals must be distinct".into(),
            ));
        }
        approver
            .verification_key
            .verify(
                &request.statement,
                &request.approval_signature,
                POLICY_APPROVAL_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyApprover,
            )
            .map_err(signature_error)?;
        self.ensure_governance_idempotency_available_locked(tenant, incarnation, &key_hash)
            .await?;
        let mut record = PolicyApprovalRecord {
            key,
            schema_version: GOVERNANCE_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            approval_id: payload.approval_id.clone(),
            policy_revision_id: policy.policy_revision_id.clone(),
            policy_revision_digest: policy.policy_revision_digest.clone(),
            target: policy.target.clone(),
            resolved_policy_digest: policy.resolved_policy_digest.clone(),
            author_principal_id: policy.author_principal_id.clone(),
            approver_registration_id: approver.registration_id.clone(),
            approver_registration_digest: approver.registration_digest.clone(),
            approver_principal_id: approver.principal_id.clone(),
            decision: "approve".into(),
            reason: payload.reason.clone(),
            signed_at_ms: payload.signed_at_ms,
            approval_signature: request.approval_signature,
            approved_by: actor,
            approved_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
            approval_digest: String::new(),
        };
        record.approval_digest = record_digest(&record, "approval_digest")?;
        self.validate_approval_record(&record, tenant, incarnation)
            .await?;
        self.insert_immutable(tenant, POLICY_APPROVALS_COLLECTION, &record.key, &record)
            .await?;
        Ok(GovernanceMutation {
            record,
            replayed: false,
        })
    }
}
