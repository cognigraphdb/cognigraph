//! Key revocation.

use super::*;

impl PromotionManager {
    pub async fn revoke_governance_key(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        request: RevokeGovernanceKeyRequest,
    ) -> Result<GovernanceMutation<GovernanceKeyRevocation>, CogniGraphError> {
        actor.require_role(Role::Admin)?;
        validate_idempotency_key(idempotency_key)?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        let now = now_millis();
        validate_statement_scope(
            &request.statement,
            KEY_REVOCATION_DOMAIN,
            tenant,
            incarnation,
        )?;
        let payload = &request.statement.payload;
        validate_identifier("revocation_id", &payload.revocation_id)?;
        validate_identifier("registration_id", &payload.registration_id)?;
        validate_reason(&payload.reason)?;
        validate_signed_at(payload.signed_at_ms, now)?;
        if payload.effective_at_ms < now.saturating_sub(MAX_CLOCK_SKEW_MS)
            || payload.effective_at_ms > now.saturating_add(MAX_CLOCK_SKEW_MS)
        {
            return Err(validation(
                "key revocation effective_at_ms must be within five minutes of server time",
            ));
        }
        let registration = self
            .get_key_locked(tenant, incarnation, &payload.registration_id)
            .await?;
        if registration.verification_key.key_id != payload.key_id
            || registration.public_key_digest != payload.public_key_digest
            || payload.revocation_id
                != key_revocation_id(tenant, incarnation, &payload.registration_id)
        {
            return Err(validation(
                "key revocation does not match the registered key identity",
            ));
        }
        let root = self.root_key()?;
        root.verify(
            &request.statement,
            &request.root_signature,
            KEY_REVOCATION_DOMAIN,
            tenant,
            incarnation,
            KeyPurpose::TrustRoot,
        )
        .map_err(signature_error)?;
        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let key = scoped_key(tenant, incarnation, "gkr", &payload.revocation_id);
        if let Some(existing) = self
            .get_authority_raw::<GovernanceKeyRevocation>(
                tenant,
                GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
                &key,
            )
            .await?
        {
            self.validate_stored_revocation_record(&existing, tenant, incarnation)?;
            if existing.idempotency_key_hash == key_hash
                && existing.request_digest == request_digest
            {
                return Ok(GovernanceMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "governance key already has a different revocation",
            ));
        }
        self.ensure_governance_idempotency_available_locked(tenant, incarnation, &key_hash)
            .await?;
        let mut record = GovernanceKeyRevocation {
            key,
            schema_version: GOVERNANCE_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            revocation_id: payload.revocation_id.clone(),
            registration_id: payload.registration_id.clone(),
            key_id: payload.key_id.clone(),
            public_key_digest: payload.public_key_digest.clone(),
            reason: payload.reason.clone(),
            signed_at_ms: payload.signed_at_ms,
            effective_at_ms: payload.effective_at_ms,
            root_key_id: root.key_id,
            root_signature: request.root_signature,
            revoked_by: actor,
            recorded_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
            revocation_digest: String::new(),
        };
        record.revocation_digest = record_digest(&record, "revocation_digest")?;
        self.validate_revocation_record(&record, tenant, incarnation)?;
        self.insert_immutable(
            tenant,
            GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
            &record.key,
            &record,
        )
        .await?;
        Ok(GovernanceMutation {
            record,
            replayed: false,
        })
    }
}
