//! Key validation.

use super::*;

impl PromotionManager {
    pub(super) fn validate_key_record(
        &self,
        record: &GovernanceKeyRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let root = self.root_key()?;
        if record.schema_version != GOVERNANCE_RECORD_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key != scoped_key(tenant, incarnation, "gk", &record.registration_id)
            || record.root_key_id != root.key_id
            || record.verification_key.key_id == record.root_key_id
            || record.registration_digest != record_digest(record, "registration_digest")?
        {
            return Err(conflict("malformed or foreign governance key record"));
        }
        validate_identifier("registration_id", &record.registration_id)?;
        validate_identifier("principal_id", &record.principal_id)?;
        validate_identifier("subject_user_key", &record.subject_user_key)?;
        record.registered_by.require_role(Role::Admin)?;
        record
            .verification_key
            .validate()
            .map_err(governance_validation_error)?;
        if record.verification_key.purpose == KeyPurpose::TrustRoot
            || record.not_before_ms == 0
            || record.registered_at_ms == 0
            || record.signed_at_ms == 0
            || record.signed_at_ms < record.not_before_ms
            || record.signed_at_ms > record.registered_at_ms.saturating_add(MAX_CLOCK_SKEW_MS)
            || record
                .not_after_ms
                .is_some_and(|until| until <= record.not_before_ms)
            || record
                .not_after_ms
                .is_some_and(|until| record.signed_at_ms >= until)
            || record
                .not_after_ms
                .is_some_and(|until| record.registered_at_ms >= until)
        {
            return Err(conflict(
                "governance key record has an invalid purpose, timestamp, or validity interval",
            ));
        }
        let registration_domain = key_registration_domain(record.verification_key.purpose);
        let statement = GovernanceStatement::new(
            registration_domain,
            tenant,
            incarnation,
            KeyRegistrationPayload {
                registration_id: record.registration_id.clone(),
                principal_id: record.principal_id.clone(),
                subject_user_key: record.subject_user_key.clone(),
                verification_key: record.verification_key.clone(),
                not_before_ms: record.not_before_ms,
                not_after_ms: record.not_after_ms,
                signed_at_ms: record.signed_at_ms,
            },
        );
        root.verify(
            &statement,
            &record.root_signature,
            registration_domain,
            tenant,
            incarnation,
            KeyPurpose::TrustRoot,
        )
        .map_err(stored_signature_error)?;
        record
            .verification_key
            .verify(
                &statement,
                &record.possession_signature,
                registration_domain,
                tenant,
                incarnation,
                record.verification_key.purpose,
            )
            .map_err(stored_signature_error)?;
        let expected_request_digest = canonical_digest(&RegisterGovernanceKeyRequest {
            statement: statement.clone(),
            root_signature: record.root_signature.clone(),
            possession_signature: record.possession_signature.clone(),
        })?;
        if record.public_key_digest
            != record
                .verification_key
                .key_id
                .strip_prefix("ed25519:")
                .unwrap_or_default()
            || record.registration_id
                != key_registration_id(tenant, incarnation, &record.verification_key.key_id)
            || !is_digest(&record.idempotency_key_hash)
            || record.request_digest != expected_request_digest
        {
            return Err(conflict("governance key derived identity mismatch"));
        }
        Ok(())
    }

    pub(super) fn validate_stored_key_record(
        &self,
        record: &GovernanceKeyRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_key_record(record, tenant, incarnation)
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(super) fn validate_revocation_record(
        &self,
        record: &GovernanceKeyRevocation,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let root = self.root_key()?;
        if record.schema_version != GOVERNANCE_RECORD_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key != scoped_key(tenant, incarnation, "gkr", &record.revocation_id)
            || record.root_key_id != root.key_id
            || record.revocation_digest != record_digest(record, "revocation_digest")?
        {
            return Err(conflict("malformed or foreign governance key revocation"));
        }
        validate_identifier("revocation_id", &record.revocation_id)?;
        validate_identifier("registration_id", &record.registration_id)?;
        validate_reason(&record.reason)?;
        record.revoked_by.require_role(Role::Admin)?;
        if record.recorded_at_ms == 0
            || record.signed_at_ms == 0
            || record.signed_at_ms > record.recorded_at_ms.saturating_add(MAX_CLOCK_SKEW_MS)
            || record.effective_at_ms < record.recorded_at_ms.saturating_sub(MAX_CLOCK_SKEW_MS)
            || record.effective_at_ms > record.recorded_at_ms.saturating_add(MAX_CLOCK_SKEW_MS)
            || record.revocation_id
                != key_revocation_id(tenant, incarnation, &record.registration_id)
        {
            return Err(conflict(
                "governance key revocation has an invalid identity or timestamp",
            ));
        }
        let statement = GovernanceStatement::new(
            KEY_REVOCATION_DOMAIN,
            tenant,
            incarnation,
            KeyRevocationPayload {
                revocation_id: record.revocation_id.clone(),
                registration_id: record.registration_id.clone(),
                key_id: record.key_id.clone(),
                public_key_digest: record.public_key_digest.clone(),
                reason: record.reason.clone(),
                signed_at_ms: record.signed_at_ms,
                effective_at_ms: record.effective_at_ms,
            },
        );
        root.verify(
            &statement,
            &record.root_signature,
            KEY_REVOCATION_DOMAIN,
            tenant,
            incarnation,
            KeyPurpose::TrustRoot,
        )
        .map_err(stored_signature_error)?;
        let expected_request_digest = canonical_digest(&RevokeGovernanceKeyRequest {
            statement,
            root_signature: record.root_signature.clone(),
        })?;
        if !is_digest(&record.idempotency_key_hash)
            || record.request_digest != expected_request_digest
        {
            return Err(conflict(
                "governance key revocation request authority mismatch",
            ));
        }
        Ok(())
    }

    pub(super) fn validate_stored_revocation_record(
        &self,
        record: &GovernanceKeyRevocation,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        self.validate_revocation_record(record, tenant, incarnation)
            .inspect_err(|error| self.record_error(tenant, error.to_string()))
    }
}
