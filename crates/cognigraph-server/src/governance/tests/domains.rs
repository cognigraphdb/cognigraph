//! Domains.

use super::*;

#[test]
fn promotion_intent_domains_bind_each_authority_generation_without_rewriting_history() {
    let mut payload = PromotionIntentPayload {
        requested_action: "promote".into(),
        target: PromotionTarget {
            space_type: "medical".into(),
            channel: "stable".into(),
        },
        evidence_id: "evidence-v1".into(),
        evidence_digest: digest_bytes(b"evidence"),
        policy_revision_id: Some("policy-v1".into()),
        policy_revision_digest: Some(digest_bytes(b"policy")),
        approval_id: Some("approval-v1".into()),
        approval_digest: Some(digest_bytes(b"approval")),
        artifact_authority_digest: None,
        consumption_authority_digest: None,
        derivation_authority_digest: None,
        preparation_authority_digest: None,
        gate_assessment_digest: digest_bytes(b"gates"),
        expected_head_decision_id: None,
        rollback_target_evidence_id: None,
        reason: "generation binding".into(),
        idempotency_key_hash: digest_bytes(b"idempotency"),
        promoter_registration_id: "promoter-registration".into(),
        promoter_principal_id: "promoter-principal".into(),
        signed_at_ms: 1,
    };
    assert_eq!(promotion_intent_domain(&payload), PROMOTION_INTENT_DOMAIN);
    let historical = serde_json::to_value(&payload).unwrap();
    assert!(historical.get("preparation_authority_digest").is_none());
    assert_eq!(
        serde_json::from_value::<PromotionIntentPayload>(historical).unwrap(),
        payload
    );

    payload.artifact_authority_digest = Some(digest_bytes(b"artifact-authority"));
    assert_eq!(promotion_intent_domain(&payload), PROMOTION_INTENT_DOMAIN);
    payload.consumption_authority_digest = Some(digest_bytes(b"consumption-authority"));
    assert_eq!(
        promotion_intent_domain(&payload),
        M21_PROMOTION_INTENT_DOMAIN
    );
    payload.derivation_authority_digest = Some(digest_bytes(b"derivation-authority"));
    assert_eq!(
        promotion_intent_domain(&payload),
        M22_PROMOTION_INTENT_DOMAIN
    );
    let m22_statement = GovernanceStatement::new(
        M22_PROMOTION_INTENT_DOMAIN,
        TENANT,
        INCARNATION,
        payload.clone(),
    );
    let mut explicit_null = serde_json::to_value(m22_statement).unwrap();
    explicit_null["payload"]["preparation_authority_digest"] = Value::Null;
    assert!(
        serde_json::from_value::<GovernanceStatement<PromotionIntentPayload>>(explicit_null)
            .is_err(),
        "an M22 signed-intent wire accepted explicit null M23 authority"
    );
    payload.preparation_authority_digest = Some(digest_bytes(b"preparation-authority"));
    assert_eq!(
        promotion_intent_domain(&payload),
        M23_PROMOTION_INTENT_DOMAIN
    );
}
#[test]
fn later_revocation_preserves_history_but_blocks_later_acceptance() {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    let promoter = SigningKeyMaterial::generate(KeyPurpose::Promoter).unwrap();
    let key = certified_key(&root, &promoter, "promoter-principal", "promoter-user", 300);
    let revocation = revocation(&root, &key, 2_000);

    validate_historical_key_use(&key, KeyPurpose::Promoter, 1_000, 1_000, Some(&revocation))
        .unwrap();
    assert!(
        validate_historical_key_use(&key, KeyPurpose::Promoter, 2_000, 2_000, Some(&revocation),)
            .is_err()
    );
}
