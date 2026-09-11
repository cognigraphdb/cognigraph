//! Contract checks.

use super::*;

#[test]
fn canonical_digest_normalizes_nfc_and_object_order() {
    let mut left = Map::new();
    left.insert("z".into(), json!("Cafe\u{301}"));
    left.insert("a".into(), json!({"second": 2, "first": "e\u{301}"}));

    let mut right = Map::new();
    right.insert("a".into(), json!({"first": "é", "second": 2}));
    right.insert("z".into(), json!("Café"));

    assert_eq!(
        canonical_digest(&Value::Object(left)).unwrap(),
        canonical_digest(&Value::Object(right)).unwrap()
    );

    let mut colliding_keys = Map::new();
    colliding_keys.insert("é".into(), json!(1));
    colliding_keys.insert("e\u{301}".into(), json!(2));
    assert!(matches!(
        canonical_digest(&Value::Object(colliding_keys)),
        Err(CogniGraphError::ValidationError(_))
    ));

    let target = PromotionTarget {
        space_type: "pharma".into(),
        channel: "production".into(),
    };
    for key in [
        scoped_key(TENANT, INCARNATION, "e", &"a".repeat(64)),
        scoped_key(TENANT, INCARNATION, "d", &"b".repeat(64)),
        head_key(TENANT, INCARNATION, &target).unwrap(),
    ] {
        assert!(key.len() <= 254, "Arango key is too long: {key}");
        assert!(
            key.chars()
                .all(|ch| ch.is_ascii_alphanumeric() || "_-.@()+,=;$!*'%:".contains(ch)),
            "Arango-unsafe key: {key}"
        );
    }
}
#[test]
fn promotion_context_validation_fails_closed() {
    let spec = eval_spec();
    let spec_digest = canonical_digest(&spec).unwrap();
    let context = promotion_context("candidate-a", &spec);
    context.validate(SPACE, &spec_digest, 1, 1).unwrap();

    let mut unknown = serde_json::to_value(&context).unwrap();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("unreviewed_extension".into(), json!(true));
    assert!(serde_json::from_value::<PromotionContext>(unknown).is_err());

    let mut invalid_ratio = context.clone();
    invalid_ratio.policy.recall.min_ratio_denominator = 0;
    assert!(matches!(
        invalid_ratio.validate(SPACE, &spec_digest, 1, 1),
        Err(CogniGraphError::ValidationError(_))
    ));

    let mut exposed_oracle = context.clone();
    exposed_oracle.oracle_separation.overlapping_document_ids = 1;
    exposed_oracle.oracle_separation.attestation_digest =
        record_digest(&exposed_oracle.oracle_separation, "attestation_digest").unwrap();
    exposed_oracle.validate(SPACE, &spec_digest, 1, 1).unwrap();

    let mut unknown_semantics = context.clone();
    unknown_semantics.policy.metric_semantics_version = "caller-defined".into();
    assert!(
        unknown_semantics
            .validate(SPACE, &spec_digest, 1, 1)
            .is_err()
    );

    let mut unapproved_exclusion = context.clone();
    unapproved_exclusion.case_manifest.exclusions = vec![CaseExclusion {
        case_id: "q1".into(),
        reason_code: "unreviewed".into(),
    }];
    unapproved_exclusion.case_manifest.exclusions_digest =
        canonical_digest(&unapproved_exclusion.case_manifest.exclusions).unwrap();
    assert!(
        unapproved_exclusion
            .validate(SPACE, &spec_digest, 1, 1)
            .is_err()
    );

    let mut unbound_candidate = context.clone();
    unbound_candidate.revisions.graph.candidate_digest = Some(digest("different-candidate"));
    assert!(matches!(
        unbound_candidate.validate(SPACE, &spec_digest, 1, 1),
        Err(CogniGraphError::ValidationError(_))
    ));

    let mut mislabeled_revision = context.clone();
    mislabeled_revision.revisions.oracle.kind = "corpus".into();
    assert!(
        mislabeled_revision
            .validate(SPACE, &spec_digest, 1, 1)
            .is_err()
    );

    let mut unrelated_candidate_digest = context.clone();
    unrelated_candidate_digest.candidate.candidate_digest = digest("unrelated-content");
    unrelated_candidate_digest.revisions.graph.candidate_digest = Some(
        unrelated_candidate_digest
            .candidate
            .candidate_digest
            .clone(),
    );
    assert!(
        unrelated_candidate_digest
            .validate(SPACE, &spec_digest, 1, 1)
            .is_err()
    );

    let mut exclusion_policy = context.clone();
    exclusion_policy.policy.exclusions.max_count = 1;
    exclusion_policy
        .policy
        .exclusions
        .allowed_reason_codes
        .push("operator_choice".into());
    assert!(
        exclusion_policy
            .validate(SPACE, &spec_digest, 1, 1)
            .is_err()
    );

    assert!(matches!(
        context.validate(SPACE, &spec_digest, 2, 1),
        Err(CogniGraphError::ValidationError(_))
    ));
}
#[test]
fn m20_context_requires_exact_five_slot_content_authority() {
    let spec = eval_spec();
    let spec_digest = canonical_digest(&spec).unwrap();
    let policy_digest = canonical_digest(&policy()).unwrap();
    let record_id = |label: &str| digest(label).trim_start_matches("sha256:").to_string();
    let binding = PolicyGovernanceBinding {
        root_key_id: "ed25519:test-root".into(),
        author_registration_id: record_id("author-registration"),
        author_registration_digest: digest("author-registration-record"),
        policy_revision_id: record_id("policy-revision"),
        policy_revision_digest: digest("policy-revision-record"),
        resolved_policy_digest: policy_digest,
        approval_id: record_id("approval"),
        approval_digest: digest("approval-record"),
        approver_registration_id: record_id("approver-registration"),
        approver_registration_digest: digest("approver-registration-record"),
        author_principal_id: "author-principal".into(),
        approver_principal_id: "approver-principal".into(),
    };
    let context = attested_context("candidate-a", &binding);
    context.validate(SPACE, &spec_digest, 1, 1).unwrap();

    let mut missing = context.clone();
    missing.artifact_attestations = None;
    assert!(missing.validate(SPACE, &spec_digest, 1, 1).is_err());

    let mut swapped = context.clone();
    let artifacts = swapped.artifact_attestations.as_mut().unwrap();
    std::mem::swap(&mut artifacts.scorer, &mut artifacts.verifier);
    artifacts.set_digest = record_digest(artifacts, "set_digest").unwrap();
    assert!(swapped.validate(SPACE, &spec_digest, 1, 1).is_err());

    let mut tampered_subject = context.clone();
    let artifacts = tampered_subject.artifact_attestations.as_mut().unwrap();
    artifacts.graph.subject_digest = digest("another-graph-subject");
    artifacts.set_digest = record_digest(artifacts, "set_digest").unwrap();
    assert!(
        tampered_subject
            .validate(SPACE, &spec_digest, 1, 1)
            .is_err()
    );

    let legacy = promotion_context("candidate-a", &spec);
    let encoded = serde_json::to_value(&legacy).unwrap();
    assert!(encoded.get("artifact_attestations").is_none());
}
#[test]
fn bootstrap_cas_fields_must_be_explicitly_null() {
    let job_id = "a".repeat(64);
    let missing = json!({
        "candidate_original_job_id": job_id,
        "candidate_replay_job_id": "b".repeat(64),
        "baseline_original_job_id": "c".repeat(64),
        "baseline_replay_job_id": "d".repeat(64),
    });
    assert!(serde_json::from_value::<RegisterEvidenceRequest>(missing.clone()).is_err());
    let mut explicit = missing;
    explicit["expected_head_decision_id"] = Value::Null;
    assert!(serde_json::from_value::<RegisterEvidenceRequest>(explicit).is_ok());

    assert!(serde_json::from_value::<PromoteRequest>(json!({"reason": "initial"})).is_err());
    assert!(
        serde_json::from_value::<PromoteRequest>(json!({
            "expected_head_decision_id": null,
            "reason": "initial",
        }))
        .is_ok()
    );
}
#[test]
fn actionable_decision_capacity_fails_before_the_stranding_insert() {
    ensure_actionable_capacity(MAX_RECONCILE_DECISIONS - 1).unwrap();
    assert!(matches!(
        ensure_actionable_capacity(MAX_RECONCILE_DECISIONS),
        Err(CogniGraphError::DocumentConflict(_))
    ));
}
#[test]
fn governance_generation_boundaries_fail_closed() {
    for generation in [
        AuthorityGeneration::M18,
        AuthorityGeneration::M19,
        AuthorityGeneration::M20,
        AuthorityGeneration::M21,
        AuthorityGeneration::M22,
        AuthorityGeneration::M23,
    ] {
        ensure_fresh_target_boundary(generation, generation).unwrap();
        ensure_target_evidence_generation(generation, []).unwrap();
        ensure_target_evidence_generation(generation, [generation, generation]).unwrap();
    }
    for (candidate, existing) in [
        (AuthorityGeneration::M19, AuthorityGeneration::M18),
        (AuthorityGeneration::M20, AuthorityGeneration::M19),
        (AuthorityGeneration::M18, AuthorityGeneration::M20),
        (AuthorityGeneration::M23, AuthorityGeneration::M22),
        (AuthorityGeneration::M22, AuthorityGeneration::M23),
    ] {
        assert!(matches!(
            ensure_fresh_target_boundary(candidate, existing),
            Err(CogniGraphError::DocumentConflict(_))
        ));
        assert!(matches!(
            ensure_target_evidence_generation(candidate, [existing]),
            Err(CogniGraphError::DocumentConflict(_))
        ));
    }

    validate_authority_generation_order([false, false]).unwrap();
    validate_authority_generation_order([true, true]).unwrap();
    validate_authority_generation_order([false, true, true]).unwrap();
    assert!(matches!(
        validate_authority_generation_order([false, true, false]),
        Err(CogniGraphError::DocumentConflict(_))
    ));
}
#[tokio::test]
async fn tenant_fence_survives_recovery_until_explicit_resume() {
    let state = AppState::new(NativeBackend::new());
    let target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "stable".into(),
    };
    state.promotions.pause_tenant(TENANT).await;
    assert!(matches!(
        state.promotions.current(TENANT, INCARNATION, &target).await,
        Err(CogniGraphError::Forbidden(_))
    ));
    state
        .promotions
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap();
    assert!(matches!(
        state.promotions.current(TENANT, INCARNATION, &target).await,
        Err(CogniGraphError::Forbidden(_))
    ));
    state.promotions.resume_tenant(TENANT);
    assert!(
        state
            .promotions
            .current(TENANT, INCARNATION, &target)
            .await
            .unwrap()
            .is_none()
    );
    state.jobs.shutdown().await;
}
