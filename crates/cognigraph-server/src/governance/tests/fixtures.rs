//! Fixtures.

use super::*;

#[test]
fn historical_heads_include_each_reachable_same_millisecond_prefix() {
    let decisions = [
        ("head-before", None, 99),
        ("same-ms-one", Some("head-before"), 100),
        ("same-ms-two", Some("same-ms-one"), 100),
        ("head-after", Some("same-ms-two"), 101),
    ];
    let possible = possible_promotion_heads_at(&decisions, 100).unwrap();
    assert_eq!(
        possible,
        HashSet::from([
            Some("head-before".to_string()),
            Some("same-ms-one".to_string()),
            Some("same-ms-two".to_string()),
        ])
    );

    let fresh = [
        ("same-ms-one", None, 100),
        ("same-ms-two", Some("same-ms-one"), 100),
    ];
    assert_eq!(
        possible_promotion_heads_at(&fresh, 100).unwrap(),
        HashSet::from([
            None,
            Some("same-ms-one".to_string()),
            Some("same-ms-two".to_string()),
        ])
    );
}
pub(super) fn actor(role: Role, key: &str) -> GovernanceActor {
    GovernanceActor {
        user_key: key.into(),
        username: format!("{key}@example.test"),
        role,
    }
}
pub(super) fn certified_key(
    root: &SigningKeyMaterial,
    signer: &SigningKeyMaterial,
    principal_id: &str,
    subject_user_key: &str,
    registered_at_ms: u64,
) -> GovernanceKeyRecord {
    let verification_key = signer.verification_key().unwrap();
    let registration_id = key_registration_id(TENANT, INCARNATION, &verification_key.key_id);
    let statement = GovernanceStatement::new(
        KEY_REGISTRATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRegistrationPayload {
            registration_id: registration_id.clone(),
            principal_id: principal_id.into(),
            subject_user_key: subject_user_key.into(),
            verification_key: verification_key.clone(),
            not_before_ms: 100,
            not_after_ms: None,
            signed_at_ms: 200,
        },
    );
    let request = RegisterGovernanceKeyRequest {
        root_signature: root.sign(&statement).unwrap(),
        possession_signature: signer.sign(&statement).unwrap(),
        statement,
    };
    let registered_by = actor(Role::Admin, "admin-user");
    let idempotency_key_hash = digest_bytes(b"register-promoter-key");
    let request_digest = canonical_digest(&request).unwrap();
    let mut record = GovernanceKeyRecord {
        key: scoped_key(TENANT, INCARNATION, "gk", &registration_id),
        schema_version: GOVERNANCE_RECORD_SCHEMA_VERSION,
        digest_algorithm: DIGEST_ALGORITHM.into(),
        tenant: TENANT.into(),
        tenant_incarnation: INCARNATION.into(),
        registration_id,
        principal_id: principal_id.into(),
        subject_user_key: subject_user_key.into(),
        public_key_digest: verification_key
            .key_id
            .strip_prefix("ed25519:")
            .unwrap()
            .into(),
        verification_key,
        not_before_ms: 100,
        not_after_ms: None,
        signed_at_ms: 200,
        root_key_id: root.key_id.clone(),
        root_signature: request.root_signature,
        possession_signature: request.possession_signature,
        registered_by,
        registered_at_ms,
        idempotency_key_hash,
        request_digest,
        registration_digest: String::new(),
    };
    record.registration_digest = record_digest(&record, "registration_digest").unwrap();
    record
}
pub(super) fn revocation(
    root: &SigningKeyMaterial,
    key: &GovernanceKeyRecord,
    recorded_at_ms: u64,
) -> GovernanceKeyRevocation {
    let revocation_id = key_revocation_id(TENANT, INCARNATION, &key.registration_id);
    let statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: revocation_id.clone(),
            registration_id: key.registration_id.clone(),
            key_id: key.verification_key.key_id.clone(),
            public_key_digest: key.public_key_digest.clone(),
            reason: "scheduled key retirement".into(),
            signed_at_ms: recorded_at_ms,
            effective_at_ms: recorded_at_ms,
        },
    );
    let request = RevokeGovernanceKeyRequest {
        root_signature: root.sign(&statement).unwrap(),
        statement,
    };
    let revoked_by = actor(Role::Admin, "admin-user");
    let idempotency_key_hash = digest_bytes(b"revoke-promoter-key");
    let request_digest = canonical_digest(&request).unwrap();
    let mut record = GovernanceKeyRevocation {
        key: scoped_key(TENANT, INCARNATION, "gkr", &revocation_id),
        schema_version: GOVERNANCE_RECORD_SCHEMA_VERSION,
        digest_algorithm: DIGEST_ALGORITHM.into(),
        tenant: TENANT.into(),
        tenant_incarnation: INCARNATION.into(),
        revocation_id,
        registration_id: key.registration_id.clone(),
        key_id: key.verification_key.key_id.clone(),
        public_key_digest: key.public_key_digest.clone(),
        reason: "scheduled key retirement".into(),
        signed_at_ms: recorded_at_ms,
        effective_at_ms: recorded_at_ms,
        root_key_id: root.key_id.clone(),
        root_signature: request.root_signature,
        revoked_by,
        recorded_at_ms,
        idempotency_key_hash,
        request_digest,
        revocation_digest: String::new(),
    };
    record.revocation_digest = record_digest(&record, "revocation_digest").unwrap();
    record
}
