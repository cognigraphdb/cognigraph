//! Key registration.

use super::*;

impl PromotionManager {
    pub async fn register_governance_key(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        subject: User,
        idempotency_key: &str,
        request: RegisterGovernanceKeyRequest,
    ) -> Result<GovernanceMutation<GovernanceKeyRecord>, CogniGraphError> {
        actor.require_role(Role::Admin)?;
        validate_idempotency_key(idempotency_key)?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        let now = now_millis();
        let payload = &request.statement.payload;
        let registration_domain = key_registration_domain(payload.verification_key.purpose);
        validate_statement_scope(&request.statement, registration_domain, tenant, incarnation)?;
        validate_key_registration_payload(payload, now)?;
        let expected_id =
            key_registration_id(tenant, incarnation, &payload.verification_key.key_id);
        if payload.registration_id != expected_id {
            return Err(validation(
                "key registration id does not match its natural identity",
            ));
        }
        if subject.tenant != tenant
            || payload.subject_user_key != subject.key
            || !role_matches_purpose(subject.role, payload.verification_key.purpose)
        {
            return Err(CogniGraphError::Forbidden(
                "governance key subject, tenant, role, and purpose do not match".into(),
            ));
        }
        let root = self.root_key()?;
        if payload.verification_key.key_id == root.key_id {
            return Err(validation(
                "the externally configured trust root cannot be registered as an operational tenant key",
            ));
        }
        root.verify(
            &request.statement,
            &request.root_signature,
            registration_domain,
            tenant,
            incarnation,
            KeyPurpose::TrustRoot,
        )
        .map_err(signature_error)?;
        payload
            .verification_key
            .verify(
                &request.statement,
                &request.possession_signature,
                registration_domain,
                tenant,
                incarnation,
                payload.verification_key.purpose,
            )
            .map_err(signature_error)?;

        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let key = scoped_key(tenant, incarnation, "gk", &payload.registration_id);
        if let Some(existing) = self
            .get_authority_raw::<GovernanceKeyRecord>(tenant, GOVERNANCE_KEYS_COLLECTION, &key)
            .await?
        {
            self.validate_stored_key_record(&existing, tenant, incarnation)?;
            if existing.idempotency_key_hash == key_hash
                && existing.request_digest == request_digest
            {
                return Ok(GovernanceMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "governance key identity already has different authority",
            ));
        }
        self.ensure_governance_idempotency_available_locked(tenant, incarnation, &key_hash)
            .await?;
        let existing_keys = self
            .scoped_records::<GovernanceKeyRecord>(
                GOVERNANCE_KEYS_COLLECTION,
                tenant,
                incarnation,
                "gk",
                MAX_KEYS_PER_TENANT + 1,
            )
            .await?;
        if existing_keys.len() >= MAX_KEYS_PER_TENANT {
            return Err(conflict(format!(
                "tenant governance key limit of {MAX_KEYS_PER_TENANT} is exhausted"
            )));
        }
        if existing_keys.iter().any(|record| {
            record.verification_key.key_id == payload.verification_key.key_id
                || (record.principal_id == payload.principal_id
                    && record.verification_key.purpose != payload.verification_key.purpose)
        }) {
            return Err(conflict(
                "governance key id is already registered or a principal is crossing duties",
            ));
        }

        let public_key_digest = payload
            .verification_key
            .key_id
            .strip_prefix("ed25519:")
            .ok_or_else(|| validation("governance key id is malformed"))?
            .to_string();
        let mut record = GovernanceKeyRecord {
            key,
            schema_version: GOVERNANCE_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            registration_id: payload.registration_id.clone(),
            principal_id: payload.principal_id.clone(),
            subject_user_key: payload.subject_user_key.clone(),
            verification_key: payload.verification_key.clone(),
            public_key_digest,
            not_before_ms: payload.not_before_ms,
            not_after_ms: payload.not_after_ms,
            signed_at_ms: payload.signed_at_ms,
            root_key_id: root.key_id,
            root_signature: request.root_signature,
            possession_signature: request.possession_signature,
            registered_by: actor,
            registered_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
            registration_digest: String::new(),
        };
        record.registration_digest = record_digest(&record, "registration_digest")?;
        self.validate_key_record(&record, tenant, incarnation)?;
        self.insert_immutable(tenant, GOVERNANCE_KEYS_COLLECTION, &record.key, &record)
            .await?;
        Ok(GovernanceMutation {
            record,
            replayed: false,
        })
    }
}
