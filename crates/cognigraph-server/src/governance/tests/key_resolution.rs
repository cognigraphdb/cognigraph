//! Key resolution.

use super::*;

#[tokio::test]
async fn pre_m25_store_recovery_lazily_creates_semantic_repair_collections() {
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    for collection in [
        GOVERNANCE_KEYS_COLLECTION,
        GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
        POLICY_REVISIONS_COLLECTION,
        POLICY_APPROVALS_COLLECTION,
        ARTIFACT_ATTESTATIONS_COLLECTION,
    ] {
        raw.ensure_collection(collection, CollectionType::Document)
            .await
            .unwrap();
    }
    assert!(
        raw.list_documents(SEMANTIC_REPAIR_REVISIONS_COLLECTION, None, None)
            .await
            .is_err()
    );
    assert!(
        raw.list_documents(SEMANTIC_REPAIR_REVIEWS_COLLECTION, None, None)
            .await
            .is_err()
    );

    let state = AppState::new_shared(raw.clone());
    state
        .promotions
        .recover_governance_locked(TENANT, INCARNATION)
        .await
        .unwrap();
    assert_eq!(
        raw.list_documents(SEMANTIC_REPAIR_REVISIONS_COLLECTION, None, None)
            .await
            .unwrap(),
        Vec::<Value>::new()
    );
    assert_eq!(
        raw.list_documents(SEMANTIC_REPAIR_REVIEWS_COLLECTION, None, None)
            .await
            .unwrap(),
        Vec::<Value>::new()
    );
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn active_key_resolution_rejects_future_registration_metadata() {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    let promoter = SigningKeyMaterial::generate(KeyPurpose::Promoter).unwrap();
    let at_ms = now_millis();
    let key = certified_key(
        &root,
        &promoter,
        "future-promoter-principal",
        "future-promoter-user",
        at_ms + 1_000,
    );
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

    assert!(matches!(
        state
            .promotions
            .get_active_key_locked(
                TENANT,
                INCARNATION,
                &key.registration_id,
                KeyPurpose::Promoter,
                at_ms,
            )
            .await,
        Err(CogniGraphError::Forbidden(_))
    ));

    let active_signer = SigningKeyMaterial::generate(KeyPurpose::Promoter).unwrap();
    let active_key = certified_key(
        &root,
        &active_signer,
        "active-promoter-principal",
        "active-promoter-user",
        300,
    );
    raw.create_document(
        GOVERNANCE_KEYS_COLLECTION,
        serde_json::to_value(&active_key).unwrap(),
    )
    .await
    .unwrap();
    let future_revocation = revocation(&root, &active_key, at_ms + 1_000);
    raw.ensure_collection(
        GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
        CollectionType::Document,
    )
    .await
    .unwrap();
    raw.create_document(
        GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
        serde_json::to_value(&future_revocation).unwrap(),
    )
    .await
    .unwrap();
    assert!(
        state
            .promotions
            .get_active_key_locked(
                TENANT,
                INCARNATION,
                &active_key.registration_id,
                KeyPurpose::Promoter,
                at_ms,
            )
            .await
            .is_ok()
    );
    state.jobs.shutdown().await;
}
