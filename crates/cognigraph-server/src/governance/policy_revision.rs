//! Policy revision.

use super::*;

impl PromotionManager {
    pub async fn create_policy_revision(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        request: CreatePolicyRevisionRequest,
    ) -> Result<GovernanceMutation<PolicyRevisionRecord>, CogniGraphError> {
        actor.require_role(Role::PolicyAuthor)?;
        validate_idempotency_key(idempotency_key)?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        let now = now_millis();
        validate_statement_scope(
            &request.statement,
            POLICY_REVISION_DOMAIN,
            tenant,
            incarnation,
        )?;
        let payload = &request.statement.payload;
        payload.target.validate()?;
        payload.resolved_policy.validate()?;
        validate_identifier("policy_revision_id", &payload.policy_revision_id)?;
        validate_identifier("author_registration_id", &payload.author_registration_id)?;
        validate_identifier("author_principal_id", &payload.author_principal_id)?;
        validate_signed_at(payload.signed_at_ms, now)?;
        let expected_policy_digest = canonical_digest(&payload.resolved_policy)?;
        if payload.resolved_policy_digest != expected_policy_digest
            || payload.policy_revision_id
                != policy_revision_id(
                    tenant,
                    incarnation,
                    &payload.target,
                    &payload.resolved_policy,
                )?
        {
            return Err(validation(
                "policy revision identity or resolved policy digest mismatch",
            ));
        }
        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let key = scoped_key(tenant, incarnation, "pr", &payload.policy_revision_id);
        if let Some(existing) = self
            .get_authority_raw::<PolicyRevisionRecord>(tenant, POLICY_REVISIONS_COLLECTION, &key)
            .await?
        {
            self.validate_stored_policy_record(&existing, tenant, incarnation)
                .await?;
            if existing.idempotency_key_hash == key_hash
                && existing.request_digest == request_digest
                && existing.created_by == actor
            {
                return Ok(GovernanceMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "policy id/revision/target identity already has different authority",
            ));
        }
        let author = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &payload.author_registration_id,
                KeyPurpose::PolicyAuthor,
                now,
            )
            .await?;
        require_key_actor(&author, &actor, &payload.author_principal_id)?;
        author
            .verification_key
            .verify(
                &request.statement,
                &request.author_signature,
                POLICY_REVISION_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::PolicyAuthor,
            )
            .map_err(signature_error)?;
        self.ensure_governance_idempotency_available_locked(tenant, incarnation, &key_hash)
            .await?;
        let policies = self
            .scoped_records::<PolicyRevisionRecord>(
                POLICY_REVISIONS_COLLECTION,
                tenant,
                incarnation,
                "pr",
                MAX_POLICY_REVISIONS_PER_TENANT + 1,
            )
            .await?;
        if policies.len() >= MAX_POLICY_REVISIONS_PER_TENANT {
            return Err(conflict(format!(
                "tenant policy revision limit of {MAX_POLICY_REVISIONS_PER_TENANT} is exhausted"
            )));
        }
        let mut record = PolicyRevisionRecord {
            key,
            schema_version: GOVERNANCE_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            policy_revision_id: payload.policy_revision_id.clone(),
            target: payload.target.clone(),
            resolved_policy: payload.resolved_policy.clone(),
            resolved_policy_digest: payload.resolved_policy_digest.clone(),
            author_registration_id: author.registration_id.clone(),
            author_registration_digest: author.registration_digest.clone(),
            author_principal_id: author.principal_id.clone(),
            signed_at_ms: payload.signed_at_ms,
            author_signature: request.author_signature,
            created_by: actor,
            created_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
            policy_revision_digest: String::new(),
        };
        record.policy_revision_digest = record_digest(&record, "policy_revision_digest")?;
        self.validate_policy_record(&record, tenant, incarnation)
            .await?;
        self.insert_immutable(tenant, POLICY_REVISIONS_COLLECTION, &record.key, &record)
            .await?;
        Ok(GovernanceMutation {
            record,
            replayed: false,
        })
    }
}
