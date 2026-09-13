//! Consumption lifecycle.

use super::*;

#[tokio::test]
async fn m21_verified_artifacts_drive_evaluation_evidence_and_promotion() {
    let cas_root = TestArtifactCasRoot::new();
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    // The live graph deliberately contains the opposite result. A passing
    // evaluation therefore proves that M21 used the verified graph bytes.
    raw.create_edge(
        "facts",
        json!({
            "_key": "m21-live-forbidden-only",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "OWNS",
            "space_id": SPACE,
            "evidence_chunk_id": "m21-live-forbidden-chunk",
        }),
    )
    .await
    .unwrap();
    let mut state = AppState::new_shared(raw.clone());
    state.artifact_cas = Some(Arc::new(
        LocalArtifactCas::open(&cas_root.0, 1_u64 << 30).unwrap(),
    ));
    state.artifact_executable_digest = Some(current_executable_digest().await.unwrap());
    let manager = state.promotions.clone();
    let (policy_binding, promoter, root) = governed_policy_binding(&state).await;
    let attestor = register_test_governance_principal(
        &state,
        &root,
        KeyPurpose::ArtifactAttestor,
        Role::ArtifactAttestor,
        "m21-artifact-attestor-principal",
        "m21-artifact-attestor-user",
    )
    .await;

    let baseline_context = stored_consumed_context(
        &state,
        &cas_root.0,
        &attestor,
        "m21-baseline",
        &policy_binding,
        None,
    )
    .await;
    let baseline_artifacts = baseline_context.artifact_attestations.clone().unwrap();
    let candidate_context = stored_consumed_context(
        &state,
        &cas_root.0,
        &attestor,
        "m21-candidate",
        &policy_binding,
        Some(&baseline_artifacts),
    )
    .await;
    let candidate_artifacts = candidate_context.artifact_attestations.clone().unwrap();

    let baseline_original =
        submit_delayed_evaluation(&state, "m21-baseline-original", baseline_context.clone()).await;
    let baseline_replay = submit_evaluation(&state, "m21-baseline-replay", baseline_context).await;
    state.jobs.set_artifact_consumption_delay_ms(5_000);
    let candidate_original = state
        .jobs
        .submit(
            state.clone(),
            TENANT.into(),
            INCARNATION.into(),
            JobActor::request(None),
            "m21-candidate-original",
            JobKind::ConstructEvaluate,
            json!({
                "space_type": SPACE,
                "eval": eval_spec(),
                "promotion_context": candidate_context.clone(),
            }),
        )
        .await
        .unwrap()
        .job
        .id;
    state.jobs.wait_for_artifact_consumption_started().await;
    tokio::time::timeout(Duration::from_secs(3), state.jobs.pause_tenant(TENANT))
        .await
        .expect("tenant suspension must interrupt active M21 consumption within one poll");
    let parked = state
        .jobs
        .get(TENANT, INCARNATION, &candidate_original)
        .await
        .unwrap();
    assert_eq!(parked.status, JobStatus::Queued);
    assert!(parked.result.is_none());
    assert!(
        parked
            .events
            .iter()
            .any(|event| event.event == "interrupted"),
        "parked M21 event history: {:?}",
        parked.events
    );

    state.jobs.set_artifact_consumption_delay_ms(0);
    assert_eq!(
        state
            .jobs
            .resume_and_recover(state.clone(), TENANT.to_string(), INCARNATION.to_string(),)
            .await
            .unwrap(),
        1
    );
    let resumed = wait_terminal(&state, &candidate_original).await;
    assert_eq!(resumed.status, JobStatus::Succeeded);
    assert_eq!(resumed.attempt, 2);
    let candidate_replay =
        submit_evaluation(&state, "m21-candidate-replay", candidate_context.clone()).await;

    let candidate_job = state
        .jobs
        .get(TENANT, INCARNATION, &candidate_original)
        .await
        .unwrap();
    assert_eq!(candidate_job.schema_version, 2);
    assert_eq!(candidate_job.result.as_ref().unwrap()["recall"]["found"], 1);
    assert_eq!(
        candidate_job.result.as_ref().unwrap()["restraint"]["violations"],
        0
    );
    let receipt: crate::artifact_consumption::ArtifactConsumptionReceipt = serde_json::from_value(
        candidate_job.result.as_ref().unwrap()["artifact_consumption"].clone(),
    )
    .unwrap();
    assert_eq!(
        receipt.schema_version,
        M21_CONSUMPTION_RECEIPT_SCHEMA_VERSION
    );
    assert_eq!(receipt.job_id, candidate_original);
    assert_eq!(receipt.tenant, TENANT);
    assert_eq!(receipt.tenant_incarnation, INCARNATION);
    receipt.validate(&candidate_context).unwrap();

    // Archive all four otherwise-unreferenced successful jobs, then prove
    // Native snapshot preflight validates an M21 receipt even before any
    // evidence record names it. The marker demonstrates rejection occurs
    // before the additive backend import can apply unrelated data.
    let archived = state
        .jobs
        .archive_terminal(
            TENANT,
            INCARNATION,
            JobActor::request(None),
            Some(u64::MAX),
            Some(100),
            None,
            false,
            Some("M21 snapshot preflight regression".into()),
        )
        .await
        .unwrap();
    assert_eq!(archived.archived, 4);
    let mut tampered_import = raw.export_snapshot().await.unwrap();
    tampered_import
        .pointer_mut(&format!("/collections/{JOB_ARCHIVE_COLLECTION}/documents"))
        .and_then(Value::as_object_mut)
        .unwrap()
        .get_mut(&candidate_original)
        .unwrap()["result"]["recall"]["found"] = json!(0);
    tampered_import["collections"]["m21_unreferenced_job_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    assert!(
        manager
            .import_snapshot(TENANT, INCARNATION, &tampered_import)
            .await
            .is_err()
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m21_unreferenced_job_should_not_apply/documents/marker")
            .is_none()
    );

    let mut divergent_duplicate = raw.export_snapshot().await.unwrap();
    if divergent_duplicate
        .pointer(&format!("/collections/{JOBS_COLLECTION}"))
        .is_none()
    {
        divergent_duplicate["collections"][JOBS_COLLECTION] =
            json!({"type": "document", "documents": {}});
    }
    let mut divergent_hot = serde_json::to_value(&candidate_job).unwrap();
    divergent_hot["finished_at_ms"] =
        json!(candidate_job.finished_at_ms.unwrap().saturating_add(1));
    divergent_duplicate
        .pointer_mut(&format!("/collections/{JOBS_COLLECTION}/documents"))
        .and_then(Value::as_object_mut)
        .unwrap()
        .insert(candidate_original.clone(), divergent_hot);
    divergent_duplicate["collections"]["m21_duplicate_job_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    assert!(
        manager
            .import_snapshot(TENANT, INCARNATION, &divergent_duplicate)
            .await
            .is_err()
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m21_duplicate_job_should_not_apply/documents/marker")
            .is_none()
    );

    // The durable receipt authenticates the score projection it was
    // finalized with. A stored-result rewrite cannot be promoted while
    // retaining the original verified-consumption receipt.
    let mut tampered_snapshot = serde_json::to_value(&candidate_job).unwrap();
    tampered_snapshot["result"]["recall"]["found"] = json!(0);
    let tampered_source = state.jobs.promotion_evaluation_source_from_snapshot_value(
        TENANT,
        INCARNATION,
        tampered_snapshot,
        false,
    );
    assert!(
        tampered_source
            .unwrap_err()
            .to_string()
            .contains("does not match its evaluation result")
    );

    let evidence = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "m21-evidence",
            RegisterEvidenceRequest {
                candidate_original_job_id: candidate_original.clone(),
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
        M21_PROMOTION_EVIDENCE_SCHEMA_VERSION
    );
    assert!(evidence.gates.overall_passed);
    let consumption = evidence.artifact_consumption.as_ref().unwrap();
    assert_eq!(
        consumption.candidate_material_digest,
        evidence.runs[0]
            .source
            .artifact_consumption
            .as_ref()
            .unwrap()
            .material_digest
    );
    assert_eq!(
        consumption.candidate_material_digest,
        evidence.runs[1]
            .source
            .artifact_consumption
            .as_ref()
            .unwrap()
            .material_digest
    );

    // M24 projects the immutable M21 authority into a deterministic,
    // deduplicated recovery inventory without reading or writing the CAS.
    let snapshot_before_plan = raw.export_snapshot().await.unwrap();
    let recovery_plan = crate::artifact_custody::derive_recovery_plan(
        &manager,
        TENANT,
        INCARNATION,
        &evidence.id,
        1_u64 << 30,
    )
    .await
    .unwrap();
    let replayed_plan = crate::artifact_custody::derive_recovery_plan(
        &manager,
        TENANT,
        INCARNATION,
        &evidence.id,
        1_u64 << 30,
    )
    .await
    .unwrap();
    assert_eq!(recovery_plan, replayed_plan);
    assert_eq!(recovery_plan.evidence_id, evidence.id);
    assert_eq!(recovery_plan.evidence_digest, evidence.evidence_digest);
    assert_eq!(
        recovery_plan.evidence_schema_version,
        evidence.schema_version
    );
    let artifact_authority = evidence.artifact_attestations.as_ref().unwrap();
    assert_eq!(
        recovery_plan.artifact_authority_digest,
        artifact_authority.authority_digest
    );
    assert_eq!(
        recovery_plan.candidate_artifact_set_digest,
        artifact_authority.candidate.set_digest
    );
    assert_eq!(
        recovery_plan.baseline_artifact_set_digest,
        artifact_authority.baseline.set_digest
    );
    assert_eq!(recovery_plan.attestations.len(), 6);
    assert_eq!(recovery_plan.manifest_entry_count, 6);
    assert_eq!(recovery_plan.blobs.len(), 5);
    assert_eq!(recovery_plan.blob_count, 5);
    assert!(recovery_plan.attestations.windows(2).all(|pair| {
        (&pair[0].kind, &pair[0].attestation_id) < (&pair[1].kind, &pair[1].attestation_id)
    }));
    assert!(
        recovery_plan
            .blobs
            .windows(2)
            .all(|pair| pair[0].blob_digest < pair[1].blob_digest)
    );
    assert_eq!(
        recovery_plan.total_bytes,
        recovery_plan
            .blobs
            .iter()
            .map(|blob| blob.byte_length)
            .sum::<u64>()
    );
    let public_plan = serde_json::to_value(&recovery_plan).unwrap();
    assert!(public_plan.get("locations").is_none());
    assert_eq!(raw.export_snapshot().await.unwrap(), snapshot_before_plan);

    let decision_key = "m21-promote";
    let intent = signed_promote_intent(
        &promoter,
        &evidence,
        decision_key,
        "promote candidate evaluated from verified artifacts",
    );
    assert_eq!(intent.statement.domain, M21_PROMOTION_INTENT_DOMAIN);
    assert_eq!(
        intent
            .statement
            .payload
            .consumption_authority_digest
            .as_deref(),
        Some(consumption.authority_digest.as_str())
    );
    let decision = manager
        .promote_signed(
            TENANT,
            INCARNATION,
            &evidence.id,
            promoter_actor(&promoter),
            decision_key,
            intent,
        )
        .await
        .unwrap()
        .record;
    assert_eq!(
        decision.schema_version,
        M21_PROMOTION_DECISION_SCHEMA_VERSION
    );
    let head = manager
        .current(TENANT, INCARNATION, &evidence.target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(head.schema_version, M21_PROMOTION_HEAD_SCHEMA_VERSION);
    assert_eq!(head.applied_decision_id, decision.id);

    state
        .jobs
        .recover_tenant(state.clone(), TENANT.into(), INCARNATION.into())
        .await
        .unwrap();
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    assert_eq!(
        manager.operator_status(TENANT, INCARNATION).await.unwrap()["m21_evidence_bundles"],
        1
    );

    // Mutating a staged byte after attestation must fail before scoring.
    let graph_record = manager
        .get_artifact_attestation(
            TENANT,
            INCARNATION,
            &candidate_artifacts.graph.attestation_id,
        )
        .await
        .unwrap();
    let graph_entry = &graph_record.manifest.entries[0];
    let graph_path = staged_blob_path(&cas_root.0, &graph_entry.blob_digest);
    let mut tampered = fs::read(&graph_path).unwrap();
    tampered[0] ^= 1;
    fs::write(graph_path, tampered).unwrap();
    let failed = state
        .jobs
        .submit(
            state.clone(),
            TENANT.into(),
            INCARNATION.into(),
            JobActor::request(None),
            "m21-tampered-graph",
            JobKind::ConstructEvaluate,
            json!({
                "space_type": SPACE,
                "eval": eval_spec(),
                "promotion_context": candidate_context,
            }),
        )
        .await
        .unwrap();
    let failed = wait_terminal(&state, &failed.job.id).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(
        failed
            .error
            .as_ref()
            .unwrap()
            .to_string()
            .contains("digest mismatch"),
        "unexpected M21 failure: {:?}",
        failed.error
    );
    assert!(failed.result.is_none());

    // Recovery-plan authority is historical and address-only: neither a
    // later attestor revocation nor currently corrupted CAS bytes changes
    // the immutable inventory selected by the evidence.
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
            reason: "rotate M21 artifact attestor after evidence".into(),
            signed_at_ms: revoked_at,
            effective_at_ms: revoked_at,
        },
    );
    manager
        .revoke_governance_key(
            TENANT,
            INCARNATION,
            GovernanceActor::from_user(governance_user(Role::Admin, "governance-admin")),
            "revoke-m21-artifact-attestor",
            RevokeGovernanceKeyRequest {
                root_signature: root.sign(&revocation_statement).unwrap(),
                statement: revocation_statement,
            },
        )
        .await
        .unwrap();
    let historical_plan = crate::artifact_custody::derive_recovery_plan(
        &manager,
        TENANT,
        INCARNATION,
        &evidence.id,
        1_u64 << 30,
    )
    .await
    .unwrap();
    assert_eq!(historical_plan, recovery_plan);
    state.jobs.shutdown().await;
}
