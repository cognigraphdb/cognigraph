//! Artifact lifecycle.

use super::*;

#[tokio::test]
async fn signed_m20_artifact_authority_drives_full_promotion_lifecycle() {
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    raw.create_edge(
        "facts",
        json!({
            "_key": "m20-expected-fact",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "SUPPLIES",
            "space_id": SPACE,
            "evidence_chunk_id": "m20-chunk-1",
        }),
    )
    .await
    .unwrap();
    let state = AppState::new_shared(raw.clone());
    let manager = state.promotions.clone();
    let (policy_binding, promoter, root) = governed_policy_binding(&state).await;
    let attestor = register_test_governance_principal(
        &state,
        &root,
        KeyPurpose::ArtifactAttestor,
        Role::ArtifactAttestor,
        "test-artifact-attestor-principal",
        "test-artifact-attestor-user",
    )
    .await;

    let baseline_context =
        stored_attested_context(&state, &attestor, "m20-baseline", &policy_binding, None).await;
    let baseline_artifacts = baseline_context.artifact_attestations.clone().unwrap();
    let candidate_context = stored_attested_context(
        &state,
        &attestor,
        "m20-candidate",
        &policy_binding,
        Some(&baseline_artifacts),
    )
    .await;
    let candidate_artifacts = candidate_context.artifact_attestations.clone().unwrap();
    assert_eq!(
        candidate_artifacts.corpus.attestation_id,
        baseline_artifacts.corpus.attestation_id
    );
    assert_ne!(
        candidate_artifacts.graph.attestation_id,
        baseline_artifacts.graph.attestation_id
    );
    assert_eq!(
        candidate_artifacts.oracle.attestation_id,
        baseline_artifacts.oracle.attestation_id
    );
    assert_eq!(
        candidate_artifacts.scorer.attestation_id,
        baseline_artifacts.scorer.attestation_id
    );
    assert_eq!(
        candidate_artifacts.verifier.attestation_id,
        baseline_artifacts.verifier.attestation_id
    );

    let baseline_original =
        submit_evaluation(&state, "m20-baseline-original", baseline_context.clone()).await;
    let baseline_replay = submit_evaluation(&state, "m20-baseline-replay", baseline_context).await;
    let candidate_original =
        submit_evaluation(&state, "m20-candidate-original", candidate_context.clone()).await;
    let candidate_replay =
        submit_evaluation(&state, "m20-candidate-replay", candidate_context).await;

    let evidence = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "m20-evidence",
            RegisterEvidenceRequest {
                candidate_original_job_id: candidate_original,
                candidate_replay_job_id: candidate_replay,
                baseline_original_job_id: baseline_original,
                baseline_replay_job_id: baseline_replay,
                expected_head_decision_id: None,
            },
        )
        .await
        .unwrap()
        .record;
    assert_eq!(
        evidence.schema_version,
        M20_PROMOTION_EVIDENCE_SCHEMA_VERSION
    );
    assert!(evidence.gates.overall_passed);
    assert_eq!(evidence.governance.as_ref(), Some(&policy_binding));
    assert!(
        evidence
            .runs
            .iter()
            .all(|run| run.source.context.schema_version == M20_PROMOTION_CONTEXT_SCHEMA_VERSION)
    );
    let evidence_artifacts = evidence.artifact_attestations.as_ref().unwrap();
    assert_eq!(evidence_artifacts.candidate, candidate_artifacts);
    assert_eq!(evidence_artifacts.baseline, baseline_artifacts);
    assert!(!artifact_authority_conflicts_with_policy(
        evidence_artifacts,
        &policy_binding
    ));
    let mut baseline_only_conflict = evidence_artifacts.clone();
    baseline_only_conflict.baseline.graph.attestor_principal_id =
        policy_binding.author_principal_id.clone();
    baseline_only_conflict.baseline.set_digest =
        record_digest(&baseline_only_conflict.baseline, "set_digest").unwrap();
    baseline_only_conflict.authority_digest =
        record_digest(&baseline_only_conflict, "authority_digest").unwrap();
    assert!(artifact_authority_conflicts_with_policy(
        &baseline_only_conflict,
        &policy_binding
    ));
    let unsupported = crate::artifact_custody::derive_recovery_plan(
        &manager,
        TENANT,
        INCARNATION,
        &evidence.id,
        1_u64 << 30,
    )
    .await
    .unwrap_err();
    assert!(matches!(unsupported, CogniGraphError::ValidationError(_)));

    let decision_key = "m20-promote";
    let decision_intent = signed_promote_intent(
        &promoter,
        &evidence,
        decision_key,
        "promote fully attested M20 candidate",
    );
    let decision = manager
        .promote_signed(
            TENANT,
            INCARNATION,
            &evidence.id,
            promoter_actor(&promoter),
            decision_key,
            decision_intent.clone(),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(
        decision.schema_version,
        M20_PROMOTION_DECISION_SCHEMA_VERSION
    );
    assert_eq!(decision.action, PromotionAction::Promote);
    assert_eq!(
        decision
            .governance
            .as_ref()
            .unwrap()
            .statement
            .payload
            .artifact_authority_digest
            .as_ref(),
        Some(&evidence_artifacts.authority_digest)
    );
    assert_eq!(
        manager
            .current(TENANT, INCARNATION, &evidence.target)
            .await
            .unwrap()
            .unwrap()
            .applied_decision_id,
        decision.id
    );

    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    assert_eq!(
        manager.operator_status(TENANT, INCARNATION).await.unwrap()["healthy"],
        json!(true)
    );

    tokio::time::sleep(Duration::from_millis(2)).await;
    let revoked_at = now_millis();
    let revocation_statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: key_revocation_id(TENANT, INCARNATION, &attestor.record.registration_id),
            registration_id: attestor.record.registration_id.clone(),
            key_id: attestor.record.verification_key.key_id.clone(),
            public_key_digest: attestor.record.public_key_digest.clone(),
            reason: "rotate M20 artifact attestor".into(),
            signed_at_ms: revoked_at,
            effective_at_ms: revoked_at,
        },
    );
    manager
        .revoke_governance_key(
            TENANT,
            INCARNATION,
            GovernanceActor::from_user(governance_user(Role::Admin, "governance-admin")),
            "revoke-m20-artifact-attestor",
            RevokeGovernanceKeyRequest {
                root_signature: root.sign(&revocation_statement).unwrap(),
                statement: revocation_statement,
            },
        )
        .await
        .unwrap();

    let historical_replay = manager
        .promote_signed(
            TENANT,
            INCARNATION,
            &evidence.id,
            promoter_actor(&promoter),
            decision_key,
            decision_intent,
        )
        .await
        .unwrap();
    assert!(historical_replay.replayed);

    let rejected = manager
        .reject_signed(
            TENANT,
            INCARNATION,
            &evidence.id,
            promoter_actor(&promoter),
            "m20-reject-after-attestor-revocation",
            signed_reject_intent(
                &promoter,
                &evidence,
                "m20-reject-after-attestor-revocation",
                "revoked artifact authority cannot authorize a new decision",
            ),
        )
        .await;
    assert!(matches!(rejected, Err(CogniGraphError::Forbidden(_))));
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    state.jobs.shutdown().await;
}
