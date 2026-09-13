//! Authority recovery.

use super::*;

#[tokio::test]
async fn recovery_and_snapshot_preflight_reject_tampered_root_authority() {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    let promoter = SigningKeyMaterial::generate(KeyPurpose::Promoter).unwrap();
    let key = certified_key(&root, &promoter, "promoter-principal", "promoter-user", 300);
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let state = AppState::new_shared(raw.clone());
    state
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();
    raw.ensure_collection(GOVERNANCE_KEYS_COLLECTION, CollectionType::Document)
        .await
        .unwrap();
    raw.create_document(
        GOVERNANCE_KEYS_COLLECTION,
        serde_json::to_value(&key).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        state
            .promotions
            .get_governance_key(TENANT, INCARNATION, &key.registration_id)
            .await
            .unwrap(),
        key
    );
    state
        .promotions
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap();

    let valid_snapshot = raw.export_snapshot().await.unwrap();
    let restored_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let restored = AppState::new_shared(restored_raw);
    restored
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();
    restored
        .promotions
        .import_snapshot(TENANT, INCARNATION, &valid_snapshot)
        .await
        .unwrap();
    assert_eq!(
        restored
            .promotions
            .get_governance_key(TENANT, INCARNATION, &key.registration_id)
            .await
            .unwrap(),
        key
    );

    let mut tampered = key.clone();
    let replacement = if tampered.root_signature.signature.starts_with('A') {
        "B"
    } else {
        "A"
    };
    tampered
        .root_signature
        .signature
        .replace_range(..1, replacement);
    tampered.registration_digest = record_digest(&tampered, "registration_digest").unwrap();
    let marker_collection = "snapshot_must_not_apply";
    let snapshot = json!({
        "collections": {
            (GOVERNANCE_KEYS_COLLECTION): {
                "type": "document",
                "documents": { tampered.key.clone(): tampered }
            },
            (marker_collection): {
                "type": "document",
                "documents": {"marker": {"_key": "marker", "value": 1}}
            }
        }
    });
    let target_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let target = AppState::new_shared(target_raw.clone());
    target
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();
    assert!(
        target
            .promotions
            .import_snapshot(TENANT, INCARNATION, &snapshot)
            .await
            .is_err()
    );
    assert_eq!(
        target_raw
            .get_document(marker_collection, "marker")
            .await
            .unwrap(),
        None
    );

    let mut corrupt_stored = key.clone();
    corrupt_stored
        .root_signature
        .signature
        .replace_range(..1, "A");
    if corrupt_stored.root_signature.signature == key.root_signature.signature {
        corrupt_stored
            .root_signature
            .signature
            .replace_range(..1, "B");
    }
    corrupt_stored.registration_digest =
        record_digest(&corrupt_stored, "registration_digest").unwrap();
    raw.replace_document(
        GOVERNANCE_KEYS_COLLECTION,
        &corrupt_stored.key,
        serde_json::to_value(&corrupt_stored).unwrap(),
    )
    .await
    .unwrap();
    assert!(
        state
            .promotions
            .get_governance_key(TENANT, INCARNATION, &key.registration_id)
            .await
            .is_err()
    );
    assert!(state.promotions.health().is_err());

    state.jobs.shutdown().await;
    restored.jobs.shutdown().await;
    target.jobs.shutdown().await;
}
#[tokio::test]
async fn recovery_keeps_policy_accepted_before_revocation_and_new_use_is_blocked() {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    let author = SigningKeyMaterial::generate(KeyPurpose::PolicyAuthor).unwrap();
    let key = certified_key(&root, &author, "author-principal", "author-user", 300);
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let state = AppState::new_shared(raw.clone());
    state
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();
    raw.ensure_collection(GOVERNANCE_KEYS_COLLECTION, CollectionType::Document)
        .await
        .unwrap();
    raw.create_document(
        GOVERNANCE_KEYS_COLLECTION,
        serde_json::to_value(&key).unwrap(),
    )
    .await
    .unwrap();

    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/m18/promotion-evaluation.json"
    ))
    .unwrap();
    let mut policy: ResolvedPromotionPolicy = serde_json::from_value(
        fixture
            .pointer("/input/promotion_context/policy")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let target: PromotionTarget = serde_json::from_value(
        fixture
            .pointer("/input/promotion_context/target")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let signed_at_ms = now_millis();
    let revision_id = policy_revision_id(TENANT, INCARNATION, &target, &policy).unwrap();
    let statement = GovernanceStatement::new(
        POLICY_REVISION_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyRevisionPayload {
            policy_revision_id: revision_id,
            target: target.clone(),
            resolved_policy_digest: canonical_digest(&policy).unwrap(),
            resolved_policy: policy.clone(),
            author_registration_id: key.registration_id.clone(),
            author_principal_id: key.principal_id.clone(),
            signed_at_ms,
        },
    );
    let accepted = state
        .promotions
        .create_policy_revision(
            TENANT,
            INCARNATION,
            actor(Role::PolicyAuthor, "author-user"),
            "policy-before-revocation",
            CreatePolicyRevisionRequest {
                author_signature: author.sign(&statement).unwrap(),
                statement,
            },
        )
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    let revoked_at_ms = now_millis();
    let revocation_statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: key_revocation_id(TENANT, INCARNATION, &key.registration_id),
            registration_id: key.registration_id.clone(),
            key_id: key.verification_key.key_id.clone(),
            public_key_digest: key.public_key_digest.clone(),
            reason: "rotate author key".into(),
            signed_at_ms: revoked_at_ms,
            effective_at_ms: revoked_at_ms,
        },
    );
    state
        .promotions
        .revoke_governance_key(
            TENANT,
            INCARNATION,
            actor(Role::Admin, "admin-user"),
            "revoke-author-key",
            RevokeGovernanceKeyRequest {
                root_signature: root.sign(&revocation_statement).unwrap(),
                statement: revocation_statement,
            },
        )
        .await
        .unwrap();

    state
        .promotions
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap();
    assert_eq!(
        state
            .promotions
            .get_policy_revision(TENANT, INCARNATION, &accepted.record.policy_revision_id)
            .await
            .unwrap(),
        accepted.record
    );

    policy.policy_revision = "2".into();
    let signed_at_ms = now_millis();
    let statement = GovernanceStatement::new(
        POLICY_REVISION_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyRevisionPayload {
            policy_revision_id: policy_revision_id(TENANT, INCARNATION, &target, &policy).unwrap(),
            target,
            resolved_policy_digest: canonical_digest(&policy).unwrap(),
            resolved_policy: policy,
            author_registration_id: key.registration_id,
            author_principal_id: key.principal_id,
            signed_at_ms,
        },
    );
    assert!(matches!(
        state
            .promotions
            .create_policy_revision(
                TENANT,
                INCARNATION,
                actor(Role::PolicyAuthor, "author-user"),
                "policy-after-revocation",
                CreatePolicyRevisionRequest {
                    author_signature: author.sign(&statement).unwrap(),
                    statement,
                },
            )
            .await,
        Err(CogniGraphError::Forbidden(_))
    ));
    state.jobs.shutdown().await;
}
#[test]
fn public_natural_ids_are_domain_separated() {
    assert_ne!(
        approval_id(TENANT, INCARNATION, "revision-a"),
        key_revocation_id(TENANT, INCARNATION, "revision-a")
    );
}
