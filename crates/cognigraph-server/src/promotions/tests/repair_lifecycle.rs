//! Repair lifecycle.

use super::*;

#[tokio::test]
async fn signed_semantic_repair_authority_tracks_promote_and_rollback_heads() {
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    raw.create_edge(
        "facts",
        json!({
            "_key": "m25-expected-fact",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "SUPPLIES",
            "space_id": SPACE,
            "evidence_chunk_id": "m25-chunk-1",
        }),
    )
    .await
    .unwrap();
    let state = AppState::new_shared(raw.clone());
    let manager = state.promotions.clone();
    let (binding, promoter, root) = governed_policy_binding(&state).await;
    let repair_author = register_test_governance_principal(
        &state,
        &root,
        KeyPurpose::PolicyAuthor,
        Role::PolicyAuthor,
        "m25-repair-author-principal",
        "m25-repair-author-user",
    )
    .await;
    let repair_approver = register_test_governance_principal(
        &state,
        &root,
        KeyPurpose::PolicyApprover,
        Role::PolicyApprover,
        "m25-repair-approver-principal",
        "m25-repair-approver-user",
    )
    .await;
    assert_ne!(
        repair_author.record.principal_id,
        repair_approver.record.principal_id
    );
    assert_ne!(
        repair_author.record.principal_id,
        promoter.record.principal_id
    );
    assert_ne!(
        repair_approver.record.principal_id,
        promoter.record.principal_id
    );

    let target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "stable".into(),
    };
    let (baseline_context, _) = governed_semantic_context("m25-baseline", &binding);
    let (candidate_a_context, candidate_a) = governed_semantic_context("m25-candidate-a", &binding);
    let revision_a = create_test_semantic_repair_revision(
        &state,
        &repair_author,
        &target,
        None,
        candidate_a,
        "m25-create-revision-a",
    )
    .await;

    let baseline_original =
        submit_evaluation(&state, "m25-baseline-original", baseline_context.clone()).await;
    let baseline_replay = submit_evaluation(&state, "m25-baseline-replay", baseline_context).await;
    let candidate_a_original = submit_evaluation(
        &state,
        "m25-candidate-a-original",
        candidate_a_context.clone(),
    )
    .await;
    let candidate_a_replay =
        submit_evaluation(&state, "m25-candidate-a-replay", candidate_a_context).await;
    let evidence_a = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "m25-evidence-a",
            RegisterEvidenceRequest {
                candidate_original_job_id: candidate_a_original.clone(),
                candidate_replay_job_id: candidate_a_replay.clone(),
                baseline_original_job_id: baseline_original,
                baseline_replay_job_id: baseline_replay,
                expected_head_decision_id: None,
            },
        )
        .await
        .unwrap()
        .record;
    let promote_a_key = "m25-promote-a";
    let promotion_a = manager
        .promote_signed(
            TENANT,
            INCARNATION,
            &evidence_a.id,
            promoter_actor(&promoter),
            promote_a_key,
            signed_promote_intent(
                &promoter,
                &evidence_a,
                promote_a_key,
                "select M25 candidate A",
            ),
        )
        .await
        .unwrap()
        .record;
    assert!(matches!(
        manager
            .resolve_current_semantic_repair_authority(TENANT, INCARNATION, &target)
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));

    let review_a = approve_test_semantic_repair_revision(
        &state,
        &repair_approver,
        &revision_a,
        "m25-approve-revision-a",
    )
    .await;
    let head_a = manager
        .current(TENANT, INCARNATION, &target)
        .await
        .unwrap()
        .unwrap();
    let resolved_a = manager
        .resolve_semantic_repair_authority(
            TENANT,
            INCARNATION,
            &head_a,
            &revision_a.candidate_digest,
        )
        .await
        .unwrap();
    assert_eq!(resolved_a.revision, revision_a);

    // A revision for a separate target is valid independent authority and
    // must not poison reconstruction of this target's promotion head.
    let other_target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "m25-other".into(),
    };
    let other_revision = create_test_semantic_repair_revision(
        &state,
        &repair_author,
        &other_target,
        None,
        semantic_repair_candidate("m25-other-candidate", &other_target),
        "m25-create-other-target-revision",
    )
    .await;
    let other_review = approve_test_semantic_repair_revision(
        &state,
        &repair_approver,
        &other_revision,
        "m25-approve-other-target-revision",
    )
    .await;
    let reconciled_a = manager
        .reconcile(TENANT, INCARNATION, &target, false)
        .await
        .unwrap();
    assert_eq!(
        reconciled_a.head.unwrap().applied_decision_id,
        promotion_a.id
    );

    let (candidate_b_context, candidate_b) = governed_semantic_context("m25-candidate-b", &binding);
    let revision_b = create_test_semantic_repair_revision(
        &state,
        &repair_author,
        &target,
        Some(&promotion_a.id),
        candidate_b,
        "m25-create-revision-b",
    )
    .await;
    approve_test_semantic_repair_revision(
        &state,
        &repair_approver,
        &revision_b,
        "m25-approve-revision-b",
    )
    .await;

    let still_a = manager
        .resolve_current_semantic_repair_authority(TENANT, INCARNATION, &target)
        .await
        .unwrap();
    assert_eq!(
        still_a.revision.semantic_repair_revision_id,
        revision_a.semantic_repair_revision_id
    );

    let candidate_b_original = submit_evaluation(
        &state,
        "m25-candidate-b-original",
        candidate_b_context.clone(),
    )
    .await;
    let candidate_b_replay =
        submit_evaluation(&state, "m25-candidate-b-replay", candidate_b_context).await;
    let evidence_b = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "m25-evidence-b",
            RegisterEvidenceRequest {
                candidate_original_job_id: candidate_b_original,
                candidate_replay_job_id: candidate_b_replay,
                baseline_original_job_id: candidate_a_original,
                baseline_replay_job_id: candidate_a_replay,
                expected_head_decision_id: Some(promotion_a.id.clone()),
            },
        )
        .await
        .unwrap()
        .record;
    let promote_b_key = "m25-promote-b";
    let promotion_b = manager
        .promote_signed(
            TENANT,
            INCARNATION,
            &evidence_b.id,
            promoter_actor(&promoter),
            promote_b_key,
            signed_promote_intent(
                &promoter,
                &evidence_b,
                promote_b_key,
                "select M25 candidate B",
            ),
        )
        .await
        .unwrap()
        .record;
    let resolved_b = manager
        .resolve_current_semantic_repair_authority(TENANT, INCARNATION, &target)
        .await
        .unwrap();
    assert_eq!(resolved_b.revision, revision_b);

    let rollback_key = "m25-rollback-b-to-a";
    let rollback = manager
        .rollback_signed(
            TENANT,
            INCARNATION,
            &target,
            promoter_actor(&promoter),
            rollback_key,
            signed_rollback_intent(
                &promoter,
                &evidence_a,
                &promotion_b.id,
                rollback_key,
                "restore approved M25 candidate A",
            ),
        )
        .await
        .unwrap()
        .record;
    let resolved_rollback = manager
        .resolve_current_semantic_repair_authority(TENANT, INCARNATION, &target)
        .await
        .unwrap();
    assert_eq!(resolved_rollback.revision, revision_a);
    assert_eq!(
        manager
            .current(TENANT, INCARNATION, &target)
            .await
            .unwrap()
            .unwrap()
            .applied_decision_id,
        rollback.id
    );

    // Stored authority is validated before compatibility filtering. Even
    // an unrelated target's review must therefore fail closed if its
    // immutable bytes are corrupted.
    let mut tampered_review = serde_json::to_value(other_review.clone()).unwrap();
    tampered_review["reason"] = json!("tampered unrelated review");
    raw.replace_document(
        SEMANTIC_REPAIR_REVIEWS_COLLECTION,
        &other_review.key,
        tampered_review,
    )
    .await
    .unwrap();
    assert!(matches!(
        manager
            .resolve_current_semantic_repair_authority(TENANT, INCARNATION, &target)
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));

    // Restore the unrelated record and clear the integrity latch before
    // exercising prospective revocation. A direct-store writer can still
    // possess a retired private key, so current resolution must validate
    // the complete revocation view instead of signatures in isolation.
    raw.replace_document(
        SEMANTIC_REPAIR_REVIEWS_COLLECTION,
        &other_review.key,
        serde_json::to_value(&other_review).unwrap(),
    )
    .await
    .unwrap();
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    tokio::time::sleep(Duration::from_millis(2)).await;
    let revoked_at = now_millis();
    let revocation_statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: key_revocation_id(
                TENANT,
                INCARNATION,
                &repair_approver.record.registration_id,
            ),
            registration_id: repair_approver.record.registration_id.clone(),
            key_id: repair_approver.record.verification_key.key_id.clone(),
            public_key_digest: repair_approver.record.public_key_digest.clone(),
            reason: "retire M25 repair approver".into(),
            signed_at_ms: revoked_at,
            effective_at_ms: revoked_at,
        },
    );
    let revocation = manager
        .revoke_governance_key(
            TENANT,
            INCARNATION,
            GovernanceActor::from_user(governance_user(Role::Admin, "governance-admin")),
            "m25-revoke-repair-approver",
            RevokeGovernanceKeyRequest {
                root_signature: root.sign(&revocation_statement).unwrap(),
                statement: revocation_statement,
            },
        )
        .await
        .unwrap()
        .record;
    let historical_after_revocation = manager
        .resolve_current_semantic_repair_authority(TENANT, INCARNATION, &target)
        .await
        .expect("later approver revocation preserves admitted review history");
    assert_eq!(historical_after_revocation.revision, revision_a);
    let accepted_at_ms = now_millis()
        .max(revocation.recorded_at_ms)
        .max(revocation.effective_at_ms);
    let post_revocation_statement = GovernanceStatement::new(
        SEMANTIC_REPAIR_REVIEW_DOMAIN,
        TENANT,
        INCARNATION,
        SemanticRepairReviewPayload {
            semantic_repair_review_id: review_a.semantic_repair_review_id.clone(),
            semantic_repair_revision_id: review_a.semantic_repair_revision_id.clone(),
            semantic_repair_revision_digest: review_a.semantic_repair_revision_digest.clone(),
            target: review_a.target.clone(),
            base_promotion_head_decision_id: review_a.base_promotion_head_decision_id.clone(),
            candidate_digest: review_a.candidate_digest.clone(),
            author_principal_id: review_a.author_principal_id.clone(),
            approver_registration_id: review_a.approver_registration_id.clone(),
            approver_principal_id: review_a.approver_principal_id.clone(),
            decision: review_a.decision,
            reason: "direct-store review accepted after approver revocation".into(),
            signed_at_ms: accepted_at_ms,
        },
    );
    let post_revocation_request = ReviewSemanticRepairRevisionRequest {
        approver_signature: repair_approver
            .signing_key
            .sign(&post_revocation_statement)
            .unwrap(),
        statement: post_revocation_statement,
    };
    let mut post_revocation_review = review_a.clone();
    post_revocation_review.reason = post_revocation_request.statement.payload.reason.clone();
    post_revocation_review.signed_at_ms = accepted_at_ms;
    post_revocation_review.reviewed_at_ms = accepted_at_ms;
    post_revocation_review.approver_signature = post_revocation_request.approver_signature.clone();
    post_revocation_review.request_digest = canonical_digest(&post_revocation_request).unwrap();
    post_revocation_review.semantic_repair_review_digest = String::new();
    post_revocation_review.semantic_repair_review_digest =
        record_digest(&post_revocation_review, "semantic_repair_review_digest").unwrap();
    let post_revocation_review_key = post_revocation_review.key.clone();
    raw.replace_document(
        SEMANTIC_REPAIR_REVIEWS_COLLECTION,
        &post_revocation_review_key,
        serde_json::to_value(post_revocation_review).unwrap(),
    )
    .await
    .unwrap();
    assert!(matches!(
        manager
            .resolve_current_semantic_repair_authority(TENANT, INCARNATION, &target)
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    state.jobs.shutdown().await;
}
