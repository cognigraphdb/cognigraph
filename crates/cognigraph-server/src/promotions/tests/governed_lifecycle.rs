//! Governed lifecycle.

use super::*;

#[test]
fn signed_governed_lifecycle_blocks_failed_promote_and_recovers() {
    lifecycle(flow);
}

async fn flow() {
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    raw.create_edge(
        "facts",
        json!({
            "_key": "governed-expected-fact",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "SUPPLIES",
            "space_id": SPACE,
            "evidence_chunk_id": "governed-chunk-1",
        }),
    )
    .await
    .unwrap();
    let state = AppState::new_shared(raw.clone());
    let manager = state.promotions.clone();
    let (binding, promoter, _root) = governed_policy_binding(&state).await;
    let target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "stable".into(),
    };

    let baseline_context = governed_context("governed-baseline", &binding);
    let candidate_a_context = governed_context("governed-a", &binding);
    let baseline_original = submit_evaluation(
        &state,
        "governed-baseline-original",
        baseline_context.clone(),
    )
    .await;
    let baseline_replay =
        submit_evaluation(&state, "governed-baseline-replay", baseline_context).await;
    let candidate_a_original =
        submit_evaluation(&state, "governed-a-original", candidate_a_context.clone()).await;
    let candidate_a_replay =
        submit_evaluation(&state, "governed-a-replay", candidate_a_context).await;
    let evidence_a = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "governed-evidence-a",
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
    let promote_a_key = "governed-promote-a";
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
                "governed candidate A passed",
            ),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(promotion_a.action, PromotionAction::Promote);

    raw.create_edge(
        "facts",
        json!({
            "_key": "governed-forbidden-fact",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "OWNS",
            "space_id": SPACE,
            "evidence_chunk_id": "governed-chunk-2",
        }),
    )
    .await
    .unwrap();
    let failed_context = governed_context("governed-failed", &binding);
    let failed_original =
        submit_evaluation(&state, "governed-failed-original", failed_context.clone()).await;
    let failed_replay = submit_evaluation(&state, "governed-failed-replay", failed_context).await;
    let failed_evidence = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "governed-failed-evidence",
            RegisterEvidenceRequest {
                candidate_original_job_id: failed_original,
                candidate_replay_job_id: failed_replay,
                baseline_original_job_id: candidate_a_original.clone(),
                baseline_replay_job_id: candidate_a_replay.clone(),
                expected_head_decision_id: Some(promotion_a.id.clone()),
            },
        )
        .await
        .unwrap()
        .record;
    assert!(!failed_evidence.gates.overall_passed);
    assert_eq!(
        failed_evidence.rollback_target_evidence_id.as_deref(),
        Some(evidence_a.id.as_str())
    );
    let blocked_key = "governed-failed-promote";
    let blocked = manager
        .promote_signed(
            TENANT,
            INCARNATION,
            &failed_evidence.id,
            promoter_actor(&promoter),
            blocked_key,
            signed_promote_intent(
                &promoter,
                &failed_evidence,
                blocked_key,
                "record failed governed attempt",
            ),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(blocked.action, PromotionAction::Blocked);
    assert_eq!(
        blocked
            .governance
            .as_ref()
            .unwrap()
            .statement
            .payload
            .rollback_target_evidence_id
            .as_deref(),
        Some(evidence_a.id.as_str())
    );
    assert_eq!(
        manager
            .current(TENANT, INCARNATION, &target)
            .await
            .unwrap()
            .unwrap()
            .applied_decision_id,
        promotion_a.id
    );
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    assert!(manager.health().is_ok());
    assert_eq!(
        manager.operator_status(TENANT, INCARNATION).await.unwrap()["healthy"],
        json!(true)
    );

    let reject_key = "governed-reject-failed";
    let rejected = manager
        .reject_signed(
            TENANT,
            INCARNATION,
            &failed_evidence.id,
            promoter_actor(&promoter),
            reject_key,
            signed_reject_intent(
                &promoter,
                &failed_evidence,
                reject_key,
                "independently reject failed candidate",
            ),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(rejected.action, PromotionAction::Reject);

    assert!(
        raw.delete_document("facts", "governed-forbidden-fact")
            .await
            .unwrap()
    );
    let candidate_b_context = governed_context("governed-b", &binding);
    let candidate_b_original =
        submit_evaluation(&state, "governed-b-original", candidate_b_context.clone()).await;
    let candidate_b_replay =
        submit_evaluation(&state, "governed-b-replay", candidate_b_context).await;
    let evidence_b = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "governed-evidence-b",
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
    let promote_b_key = "governed-promote-b";
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
                "governed candidate B passed",
            ),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(promotion_b.action, PromotionAction::Promote);
    let rollback_key = "governed-rollback-b-to-a";
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
                "restore governed candidate A",
            ),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(rollback.action, PromotionAction::Rollback);
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    assert_eq!(
        manager
            .current(TENANT, INCARNATION, &target)
            .await
            .unwrap()
            .unwrap()
            .applied_decision_id,
        rollback.id
    );

    let legacy_baseline = promotion_context("governed-a", &eval_spec());
    let legacy_candidate = promotion_context("legacy-on-governed-target", &eval_spec());
    let legacy_baseline_original = submit_evaluation(
        &state,
        "mixed-admission-baseline-original",
        legacy_baseline.clone(),
    )
    .await;
    let legacy_baseline_replay =
        submit_evaluation(&state, "mixed-admission-baseline-replay", legacy_baseline).await;
    let legacy_candidate_original = submit_evaluation(
        &state,
        "mixed-admission-candidate-original",
        legacy_candidate.clone(),
    )
    .await;
    let legacy_candidate_replay =
        submit_evaluation(&state, "mixed-admission-candidate-replay", legacy_candidate).await;
    assert!(matches!(
        manager
            .register_evidence(
                TENANT,
                INCARNATION,
                admin(),
                "mixed-generation-admission",
                RegisterEvidenceRequest {
                    candidate_original_job_id: legacy_candidate_original,
                    candidate_replay_job_id: legacy_candidate_replay,
                    baseline_original_job_id: legacy_baseline_original,
                    baseline_replay_job_id: legacy_baseline_replay,
                    expected_head_decision_id: Some(rollback.id.clone()),
                },
            )
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));

    let mut isolated_baseline = promotion_context("isolated-baseline", &eval_spec());
    isolated_baseline.target.channel = "legacy-isolated".into();
    let mut isolated_candidate = promotion_context("isolated-candidate", &eval_spec());
    isolated_candidate.target.channel = "legacy-isolated".into();
    let isolated_baseline_original = submit_evaluation(
        &state,
        "isolated-baseline-original",
        isolated_baseline.clone(),
    )
    .await;
    let isolated_baseline_replay =
        submit_evaluation(&state, "isolated-baseline-replay", isolated_baseline).await;
    let isolated_candidate_original = submit_evaluation(
        &state,
        "isolated-candidate-original",
        isolated_candidate.clone(),
    )
    .await;
    let isolated_candidate_replay =
        submit_evaluation(&state, "isolated-candidate-replay", isolated_candidate).await;
    let mut injected_legacy = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            admin(),
            "isolated-legacy-evidence",
            RegisterEvidenceRequest {
                candidate_original_job_id: isolated_candidate_original,
                candidate_replay_job_id: isolated_candidate_replay,
                baseline_original_job_id: isolated_baseline_original,
                baseline_replay_job_id: isolated_baseline_replay,
                expected_head_decision_id: None,
            },
        )
        .await
        .unwrap()
        .record;
    injected_legacy.target = target;
    for run in &mut injected_legacy.runs {
        run.source.context.target = injected_legacy.target.clone();
        run.source.context_digest = run.source.context.digest().unwrap();
    }
    injected_legacy.evidence_digest = record_digest(&injected_legacy, "evidence_digest").unwrap();
    raw.replace_document(
        EVIDENCE_COLLECTION,
        &injected_legacy.key,
        serde_json::to_value(&injected_legacy).unwrap(),
    )
    .await
    .unwrap();
    assert!(matches!(
        manager.recover_tenant(TENANT, INCARNATION).await,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    assert!(manager.health().is_err());
    state.jobs.shutdown().await;
}
