//! Key fixtures.

use super::*;

pub(super) fn governance_user(role: Role, key: &str) -> User {
    User {
        key: key.into(),
        username: format!("{key}@example.test"),
        role,
        tenant: TENANT.into(),
    }
}
pub(super) async fn register_test_governance_principal(
    state: &AppState,
    root: &SigningKeyMaterial,
    purpose: KeyPurpose,
    role: Role,
    principal_id: &str,
    user_key: &str,
) -> TestGovernancePrincipal {
    let signing_key = SigningKeyMaterial::generate(purpose).unwrap();
    let verification_key = signing_key.verification_key().unwrap();
    let now = now_millis();
    let statement = GovernanceStatement::new(
        if purpose == KeyPurpose::ArtifactAttestor {
            M20_KEY_REGISTRATION_DOMAIN
        } else {
            KEY_REGISTRATION_DOMAIN
        },
        TENANT,
        INCARNATION,
        KeyRegistrationPayload {
            registration_id: key_registration_id(TENANT, INCARNATION, &verification_key.key_id),
            principal_id: principal_id.into(),
            subject_user_key: user_key.into(),
            verification_key,
            not_before_ms: now.saturating_sub(1),
            not_after_ms: None,
            signed_at_ms: now,
        },
    );
    let request = RegisterGovernanceKeyRequest {
        root_signature: root.sign(&statement).unwrap(),
        possession_signature: signing_key.sign(&statement).unwrap(),
        statement,
    };
    let actor = GovernanceActor::from_user(governance_user(role, user_key));
    let record = state
        .promotions
        .register_governance_key(
            TENANT,
            INCARNATION,
            GovernanceActor::from_user(governance_user(Role::Admin, "governance-admin")),
            governance_user(role, user_key),
            &format!("register-{principal_id}"),
            request,
        )
        .await
        .unwrap()
        .record;
    TestGovernancePrincipal {
        signing_key,
        actor,
        record,
    }
}
pub(super) fn staged_test_governance_principal(
    root: &SigningKeyMaterial,
    purpose: KeyPurpose,
    role: Role,
    principal_id: &str,
    user_key: &str,
    label: &str,
) -> TestGovernancePrincipal {
    let signing_key = SigningKeyMaterial::generate(purpose).unwrap();
    let verification_key = signing_key.verification_key().unwrap();
    let now = now_millis();
    let statement = GovernanceStatement::new(
        if purpose == KeyPurpose::ArtifactAttestor {
            M20_KEY_REGISTRATION_DOMAIN
        } else {
            KEY_REGISTRATION_DOMAIN
        },
        TENANT,
        INCARNATION,
        KeyRegistrationPayload {
            registration_id: key_registration_id(TENANT, INCARNATION, &verification_key.key_id),
            principal_id: principal_id.into(),
            subject_user_key: user_key.into(),
            verification_key,
            not_before_ms: now.saturating_sub(1),
            not_after_ms: None,
            signed_at_ms: now,
        },
    );
    let root_signature = root.sign(&statement).unwrap();
    let possession_signature = signing_key.sign(&statement).unwrap();
    let request = RegisterGovernanceKeyRequest {
        root_signature: root_signature.clone(),
        possession_signature: possession_signature.clone(),
        statement: statement.clone(),
    };
    let registration_id = statement.payload.registration_id.clone();
    let mut record = GovernanceKeyRecord {
        key: scoped_key(TENANT, INCARNATION, "gk", &registration_id),
        schema_version: GOVERNANCE_RECORD_SCHEMA_VERSION,
        digest_algorithm: DIGEST_ALGORITHM.into(),
        tenant: TENANT.into(),
        tenant_incarnation: INCARNATION.into(),
        registration_id,
        principal_id: statement.payload.principal_id.clone(),
        subject_user_key: statement.payload.subject_user_key.clone(),
        public_key_digest: statement
            .payload
            .verification_key
            .key_id
            .strip_prefix("ed25519:")
            .unwrap()
            .into(),
        verification_key: statement.payload.verification_key.clone(),
        not_before_ms: statement.payload.not_before_ms,
        not_after_ms: statement.payload.not_after_ms,
        signed_at_ms: statement.payload.signed_at_ms,
        root_key_id: root.verification_key().unwrap().key_id,
        root_signature,
        possession_signature,
        registered_by: GovernanceActor::from_user(governance_user(Role::Admin, "governance-admin")),
        registered_at_ms: now,
        idempotency_key_hash: digest_bytes(format!("register-{label}").as_bytes()),
        request_digest: canonical_digest(&request).unwrap(),
        registration_digest: String::new(),
    };
    record.registration_digest = record_digest(&record, "registration_digest").unwrap();
    TestGovernancePrincipal {
        signing_key,
        actor: GovernanceActor::from_user(governance_user(role, user_key)),
        record,
    }
}
