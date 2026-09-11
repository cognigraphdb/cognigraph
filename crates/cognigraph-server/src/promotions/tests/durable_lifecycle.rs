//! Durable lifecycle.

use super::*;

#[test]
fn durable_promotion_lifecycle_replays_conflicts_and_repairs_only_the_head() {
    // The restore branches retain several complete snapshot fixtures across
    // awaits, so poll this test on a bounded stack larger than libtest's
    // default rather than weakening its end-to-end coverage.
    std::thread::Builder::new()
        .name("durable-promotion-lifecycle".into())
        .stack_size(4 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(durable_promotion_lifecycle_inner());
        })
        .unwrap()
        .join()
        .unwrap();
}
async fn durable_promotion_lifecycle_inner() {
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    raw.create_edge(
        "facts",
        json!({
            "_key": "promotion-expected-fact",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "SUPPLIES",
            "space_id": SPACE,
            "evidence_chunk_id": "promotion-chunk-1",
        }),
    )
    .await
    .unwrap();
    let state = AppState::new_shared(raw.clone());
    let manager = state.promotions.clone();
    let target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "stable".into(),
    };

    let baseline_context = promotion_context("baseline-0", &eval_spec());
    let candidate_a_context = promotion_context("candidate-a", &eval_spec());
    let candidate_b_context = promotion_context("candidate-b", &eval_spec());
    let baseline_original =
        submit_evaluation(&state, "job-baseline-0-original", baseline_context.clone()).await;
    let baseline_replay =
        submit_evaluation(&state, "job-baseline-0-replay", baseline_context).await;
    let candidate_a_original = submit_evaluation(
        &state,
        "job-candidate-a-original",
        candidate_a_context.clone(),
    )
    .await;
    let candidate_a_replay =
        submit_evaluation(&state, "job-candidate-a-replay", candidate_a_context).await;

    let evidence_a_request = RegisterEvidenceRequest {
        candidate_original_job_id: candidate_a_original.clone(),
        candidate_replay_job_id: candidate_a_replay.clone(),
        baseline_original_job_id: baseline_original,
        baseline_replay_job_id: baseline_replay,
        expected_head_decision_id: None,
    };
    let evidence_a = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            admin(),
            "evidence-a",
            evidence_a_request.clone(),
        )
        .await
        .unwrap();
    assert!(!evidence_a.replayed);
    assert!(evidence_a.record.gates.overall_passed);
    let evidence_a_replay = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            admin(),
            "evidence-a",
            evidence_a_request.clone(),
        )
        .await
        .unwrap();
    assert!(evidence_a_replay.replayed);
    assert_eq!(evidence_a_replay.record, evidence_a.record);

    let mut changed_evidence_request = evidence_a_request;
    changed_evidence_request.expected_head_decision_id = Some("0".repeat(64));
    assert!(matches!(
        manager
            .register_evidence(
                TENANT,
                INCARNATION,
                admin(),
                "evidence-a",
                changed_evidence_request,
            )
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));

    let promotion_a = manager
        .promote(
            TENANT,
            INCARNATION,
            &evidence_a.record.id,
            admin(),
            "promote-a",
            PromoteRequest {
                expected_head_decision_id: None,
                reason: "candidate A passed the frozen gates".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(promotion_a.record.action, PromotionAction::Promote);
    assert_eq!(
        manager
            .current(TENANT, INCARNATION, &target)
            .await
            .unwrap()
            .unwrap()
            .selection
            .evidence_id,
        evidence_a.record.id
    );

    let candidate_b_original = submit_evaluation(
        &state,
        "job-candidate-b-original",
        candidate_b_context.clone(),
    )
    .await;
    let candidate_b_replay =
        submit_evaluation(&state, "job-candidate-b-replay", candidate_b_context).await;
    let evidence_b = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            admin(),
            "evidence-b",
            RegisterEvidenceRequest {
                candidate_original_job_id: candidate_b_original,
                candidate_replay_job_id: candidate_b_replay,
                baseline_original_job_id: candidate_a_original,
                baseline_replay_job_id: candidate_a_replay,
                expected_head_decision_id: Some(promotion_a.record.id.clone()),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        evidence_b.record.baseline_candidate_digest,
        evidence_a.record.candidate_digest
    );
    assert_eq!(
        evidence_b.record.rollback_target_evidence_id.as_deref(),
        Some(evidence_a.record.id.as_str())
    );

    let promotion_b = manager
        .promote(
            TENANT,
            INCARNATION,
            &evidence_b.record.id,
            admin(),
            "promote-b",
            PromoteRequest {
                expected_head_decision_id: Some(promotion_a.record.id.clone()),
                reason: "candidate B passed against candidate A".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(promotion_b.record.action, PromotionAction::Promote);
    assert_eq!(
        promotion_b
            .record
            .resulting_selection
            .as_ref()
            .unwrap()
            .prior_evidence_id
            .as_deref(),
        Some(evidence_a.record.id.as_str())
    );

    assert!(matches!(
        manager
            .promote(
                TENANT,
                INCARNATION,
                &evidence_a.record.id,
                admin(),
                "stale-promotion",
                PromoteRequest {
                    expected_head_decision_id: Some(promotion_a.record.id.clone()),
                    reason: "stale comparison-and-swap".into(),
                },
            )
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));

    let rollback_request = RollbackRequest {
        expected_head_decision_id: promotion_b.record.id.clone(),
        to_evidence_id: evidence_a.record.id.clone(),
        reason: "restore candidate A".into(),
    };
    let rollback = manager
        .rollback(
            TENANT,
            INCARNATION,
            &target,
            admin(),
            "rollback-b-to-a",
            rollback_request.clone(),
        )
        .await
        .unwrap();
    assert!(!rollback.replayed);
    assert_eq!(rollback.record.action, PromotionAction::Rollback);
    let rollback_replay = manager
        .rollback(
            TENANT,
            INCARNATION,
            &target,
            admin(),
            "rollback-b-to-a",
            rollback_request,
        )
        .await
        .unwrap();
    assert!(rollback_replay.replayed);
    assert_eq!(rollback_replay.record, rollback.record);

    let rejection = manager
        .decide(
            TENANT,
            INCARNATION,
            &evidence_b.record.id,
            admin(),
            "reject-b-after-rollback",
            PromotionAction::Reject,
            "retain rejection as non-actionable audit history".into(),
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(rejection.record.action, PromotionAction::Reject);
    assert!(rejection.record.resulting_selection.is_none());

    let head_key = head_key(TENANT, INCARNATION, &target).unwrap();
    assert!(
        raw.delete_document(HEADS_COLLECTION, &head_key)
            .await
            .unwrap()
    );
    let deleted_repair = manager
        .reconcile(TENANT, INCARNATION, &target, false)
        .await
        .unwrap();
    assert!(deleted_repair.head_changed);
    assert_eq!(
        deleted_repair.head.as_ref().unwrap().applied_decision_id,
        rollback.record.id
    );

    let mut corrupt_head = serde_json::to_value(deleted_repair.head.unwrap()).unwrap();
    corrupt_head["projection_digest"] = json!(digest("corrupt-head"));
    raw.replace_document(HEADS_COLLECTION, &head_key, corrupt_head)
        .await
        .unwrap();
    let corrupt_repair = manager
        .reconcile(TENANT, INCARNATION, &target, false)
        .await
        .unwrap();
    assert!(corrupt_repair.head_changed);
    assert_eq!(
        corrupt_repair.head.unwrap().selection.evidence_id,
        evidence_a.record.id
    );

    raw.replace_document(
        HEADS_COLLECTION,
        &head_key,
        json!({"_key": head_key, "forged": true}),
    )
    .await
    .unwrap();
    assert!(manager.recover_tenant(TENANT, INCARNATION).await.unwrap() >= 1);
    assert_eq!(
        manager
            .current(TENANT, INCARNATION, &target)
            .await
            .unwrap()
            .unwrap()
            .applied_decision_id,
        rollback.record.id
    );

    manager.record_error(TENANT, "simulated committed-head projection failure".into());
    assert!(manager.health().is_err());
    assert!(matches!(
        manager.ensure_mutations_healthy(TENANT),
        Err(CogniGraphError::ConnectionError(_))
    ));
    let degraded_status = manager.operator_status(TENANT, INCARNATION).await.unwrap();
    assert_eq!(degraded_status["healthy"], json!(false));
    assert_eq!(degraded_status["full_validation_performed"], json!(true));
    let degraded_replay = manager
        .rollback(
            TENANT,
            INCARNATION,
            &target,
            admin(),
            "rollback-b-to-a",
            RollbackRequest {
                expected_head_decision_id: promotion_b.record.id.clone(),
                to_evidence_id: evidence_a.record.id.clone(),
                reason: "restore candidate A".into(),
            },
        )
        .await
        .unwrap();
    assert!(degraded_replay.replayed);
    assert!(manager.health().is_err());
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    assert!(manager.health().is_ok());

    assert!(matches!(
        manager
            .get_evidence(TENANT, "replaced-incarnation", &evidence_a.record.id)
            .await,
        Err(CogniGraphError::DocumentNotFound { .. })
    ));
    assert!(matches!(
        manager
            .get_decision(TENANT, "replaced-incarnation", &promotion_a.record.id)
            .await,
        Err(CogniGraphError::DocumentNotFound { .. })
    ));
    assert!(matches!(
        state
            .backend
            .get_document(EVIDENCE_COLLECTION, &evidence_a.record.key)
            .await,
        Err(CogniGraphError::Forbidden(_))
    ));
    assert!(matches!(
        state
            .backend
            .get_document(DECISIONS_COLLECTION, &promotion_a.record.key)
            .await,
        Err(CogniGraphError::Forbidden(_))
    ));

    assert_eq!(
        manager
            .get_evidence(TENANT, INCARNATION, &evidence_a.record.id)
            .await
            .unwrap(),
        evidence_a.record
    );
    assert_eq!(
        manager
            .get_evidence(TENANT, INCARNATION, &evidence_b.record.id)
            .await
            .unwrap(),
        evidence_b.record
    );
    assert_eq!(
        manager
            .get_decision(TENANT, INCARNATION, &promotion_a.record.id)
            .await
            .unwrap(),
        promotion_a.record
    );
    assert_eq!(
        manager
            .get_decision(TENANT, INCARNATION, &promotion_b.record.id)
            .await
            .unwrap(),
        promotion_b.record
    );
    assert_eq!(
        manager
            .get_decision(TENANT, INCARNATION, &rollback.record.id)
            .await
            .unwrap(),
        rollback.record
    );

    let snapshot = raw.export_snapshot().await.unwrap();
    let mut untrusted_head_snapshot = snapshot.clone();
    let head_documents = untrusted_head_snapshot
        .pointer_mut(&format!("/collections/{HEADS_COLLECTION}/documents"))
        .and_then(Value::as_object_mut)
        .unwrap();
    head_documents.insert(head_key.clone(), json!({"_key": head_key, "forged": true}));

    let restored_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let restored_state = AppState::new_shared(restored_raw.clone());
    restored_state
        .promotions
        .import_snapshot(TENANT, INCARNATION, &untrusted_head_snapshot)
        .await
        .unwrap();
    let restored_head = restored_state
        .promotions
        .current(TENANT, INCARNATION, &target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored_head.applied_decision_id, rollback.record.id);
    assert_eq!(restored_head.selection.evidence_id, evidence_a.record.id);

    let mut missing_job = snapshot.clone();
    let referenced_job_id = &evidence_a.record.runs[0].source.job_id;
    for collection in [JOBS_COLLECTION, JOB_ARCHIVE_COLLECTION] {
        if let Some(documents) = missing_job
            .pointer_mut(&format!("/collections/{collection}/documents"))
            .and_then(Value::as_object_mut)
        {
            documents.remove(referenced_job_id);
        }
    }
    missing_job["collections"]["missing_job_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    let missing_job_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let missing_job_state = AppState::new_shared(missing_job_raw.clone());
    assert!(
        missing_job_state
            .promotions
            .import_snapshot(TENANT, INCARNATION, &missing_job)
            .await
            .is_err()
    );
    assert_eq!(
        missing_job_raw
            .get_document("missing_job_snapshot_should_not_apply", "marker")
            .await
            .unwrap(),
        None
    );

    let mut collision_collection = None;
    let mut collision_job = None;
    for collection in [JOBS_COLLECTION, JOB_ARCHIVE_COLLECTION] {
        if let Some(job) = snapshot
            .pointer(&format!(
                "/collections/{collection}/documents/{referenced_job_id}"
            ))
            .cloned()
        {
            collision_collection = Some(collection);
            collision_job = Some(job);
            break;
        }
    }
    let collision_collection = collision_collection.expect("snapshot source collection");
    let mut collision_job = collision_job.expect("snapshot source job");
    let original_finished_at = collision_job["finished_at_ms"].as_u64().unwrap();
    collision_job["finished_at_ms"] = json!(original_finished_at + 1);
    let collision_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    collision_raw
        .ensure_collection(collision_collection, CollectionType::Document)
        .await
        .unwrap();
    collision_raw
        .create_document(collision_collection, collision_job.clone())
        .await
        .unwrap();
    let collision_state = AppState::new_shared(collision_raw.clone());
    let mut collision_snapshot = snapshot.clone();
    collision_snapshot["collections"]["collision_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    assert!(matches!(
        collision_state
            .promotions
            .import_snapshot(TENANT, INCARNATION, &collision_snapshot)
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    assert_eq!(
        collision_raw
            .get_document("collision_snapshot_should_not_apply", "marker")
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        collision_raw
            .get_document(collision_collection, referenced_job_id)
            .await
            .unwrap()
            .unwrap()["finished_at_ms"],
        json!(original_finished_at + 1)
    );

    let mut divergent = snapshot.clone();
    let mut divergent_evidence = evidence_a.record.clone();
    divergent_evidence.created_at_ms += 1;
    divergent_evidence.evidence_digest =
        record_digest(&divergent_evidence, "evidence_digest").unwrap();
    divergent
        .pointer_mut(&format!("/collections/{EVIDENCE_COLLECTION}/documents"))
        .and_then(Value::as_object_mut)
        .unwrap()
        .insert(
            divergent_evidence.key.clone(),
            serde_json::to_value(divergent_evidence).unwrap(),
        );
    divergent["collections"]["snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    assert!(matches!(
        restored_state
            .promotions
            .import_snapshot(TENANT, INCARNATION, &divergent)
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    assert_eq!(
        restored_raw
            .get_document("snapshot_should_not_apply", "marker")
            .await
            .unwrap(),
        None
    );

    let divergent_job_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let divergent_job_state = AppState::new_shared(divergent_job_raw.clone());
    divergent_job_state
        .promotions
        .import_snapshot(TENANT, INCARNATION, &snapshot)
        .await
        .unwrap();
    let mut source_collection = None;
    let mut divergent_job = None;
    for collection in [JOBS_COLLECTION, JOB_ARCHIVE_COLLECTION] {
        if let Some(job) = divergent_job_raw
            .get_document(collection, referenced_job_id)
            .await
            .unwrap()
        {
            source_collection = Some(collection);
            divergent_job = Some(job);
            break;
        }
    }
    let mut divergent_job = divergent_job.expect("referenced source job in snapshot");
    let finished_at_ms = divergent_job["finished_at_ms"].as_u64().unwrap();
    divergent_job["finished_at_ms"] = json!(finished_at_ms + 1);
    divergent_job_raw
        .replace_document(
            source_collection.expect("source job collection"),
            referenced_job_id,
            divergent_job,
        )
        .await
        .unwrap();
    assert!(
        divergent_job_state
            .promotions
            .recover_tenant(TENANT, INCARNATION)
            .await
            .is_err()
    );
    let divergent_status = divergent_job_state
        .promotions
        .operator_status(TENANT, INCARNATION)
        .await
        .unwrap();
    assert_eq!(divergent_status["healthy"], json!(false));
    assert!(divergent_job_state.promotions.health().is_err());

    let mut forked = snapshot;
    let mut forked_decision = promotion_b.record.clone();
    let forked_selection = forked_decision.resulting_selection.as_mut().unwrap();
    forked_selection.generation = 1;
    forked_selection.prior_evidence_id = None;
    forked_decision.decision_digest = record_digest(&forked_decision, "decision_digest").unwrap();
    forked
        .pointer_mut(&format!("/collections/{DECISIONS_COLLECTION}/documents"))
        .and_then(Value::as_object_mut)
        .unwrap()
        .insert(
            forked_decision.key.clone(),
            serde_json::to_value(forked_decision).unwrap(),
        );
    forked["collections"]["forked_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    let fork_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let fork_state = AppState::new_shared(fork_raw.clone());
    assert!(matches!(
        fork_state
            .promotions
            .import_snapshot(TENANT, INCARNATION, &forked)
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    assert_eq!(
        fork_raw
            .get_document("forked_snapshot_should_not_apply", "marker")
            .await
            .unwrap(),
        None
    );

    for collection in [JOBS_COLLECTION, JOB_ARCHIVE_COLLECTION] {
        restored_raw
            .delete_document(collection, referenced_job_id)
            .await
            .unwrap();
    }
    assert!(
        restored_state
            .promotions
            .recover_tenant(TENANT, INCARNATION)
            .await
            .is_err()
    );
    assert!(restored_state.promotions.health().is_err());

    let mut tampered_decision = serde_json::to_value(&rejection.record).unwrap();
    tampered_decision["reason"] = json!("tampered after commit");
    raw.replace_document(
        DECISIONS_COLLECTION,
        &rejection.record.key,
        tampered_decision,
    )
    .await
    .unwrap();
    assert!(matches!(
        manager.current(TENANT, INCARNATION, &target).await,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    assert!(manager.health().is_err());

    state.jobs.shutdown().await;
    restored_state.jobs.shutdown().await;
    missing_job_state.jobs.shutdown().await;
    collision_state.jobs.shutdown().await;
    divergent_job_state.jobs.shutdown().await;
    fork_state.jobs.shutdown().await;
}
