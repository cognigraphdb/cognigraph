//! Raw document lifecycle.

use super::*;

#[tokio::test]
async fn m23_raw_documents_drive_preparation_promotion_and_fail_closed_on_tamper() {
    let cas_root = TestArtifactCasRoot::new();
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    raw.create_edge(
        "facts",
        json!({
            "_key": "m23-live-forbidden-only",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "OWNS",
            "space_id": SPACE,
            "evidence_chunk_id": "m23-live-forbidden-chunk",
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
        "m23-artifact-attestor-principal",
        "m23-artifact-attestor-user",
    )
    .await;

    let baseline_context = stored_prepared_context(
        &state,
        &cas_root.0,
        &attestor,
        "m23-baseline",
        &policy_binding,
        None,
        false,
    )
    .await;
    let baseline_artifacts = baseline_context.artifact_attestations.clone().unwrap();
    let candidate_context = stored_prepared_context(
        &state,
        &cas_root.0,
        &attestor,
        "m23-candidate",
        &policy_binding,
        Some(&baseline_artifacts),
        false,
    )
    .await;

    let baseline_original =
        submit_evaluation(&state, "m23-baseline-original", baseline_context.clone()).await;
    let baseline_replay = submit_evaluation(&state, "m23-baseline-replay", baseline_context).await;
    let candidate_original =
        submit_evaluation(&state, "m23-candidate-original", candidate_context.clone()).await;
    let candidate_replay =
        submit_evaluation(&state, "m23-candidate-replay", candidate_context.clone()).await;
    let unreferenced_job = submit_evaluation(
        &state,
        "m23-unreferenced-valid-job",
        candidate_context.clone(),
    )
    .await;
    let candidate_job = state
        .jobs
        .get(TENANT, INCARNATION, &candidate_original)
        .await
        .unwrap();
    assert_eq!(candidate_job.schema_version, 4);
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
        M23_CONSUMPTION_RECEIPT_SCHEMA_VERSION
    );
    let derivation = receipt.derivation.as_ref().unwrap();
    assert_eq!(
        derivation.schema_version,
        M23_DERIVATION_RECEIPT_SCHEMA_VERSION
    );
    assert_eq!(derivation.chunk_count, None);
    assert_eq!(derivation.fact_count, 1);
    assert_eq!(
        derivation.facts[0].evidence_chunk_id,
        "d-bdc2c7ea55672198256298634354c8cb1672975db3639c8cd8ba50433c2060de-c00000000"
    );
    let preparation = derivation.preparation.as_ref().unwrap();
    assert_eq!(
        preparation.schema_version,
        M23_PREPARATION_RECEIPT_SCHEMA_VERSION
    );
    assert_eq!(
        preparation.documents_blob_digest,
        preparation.documents_semantic_digest
    );
    assert_eq!(
        preparation.raw_document_set_digest,
        preparation.documents_semantic_digest
    );
    assert_eq!(
        preparation.prepared_corpus_blob_digest,
        preparation.prepared_corpus_semantic_digest
    );
    assert_eq!(
        preparation.preparation_plan_digest,
        candidate_context
            .consumption_plan
            .as_ref()
            .unwrap()
            .preparation
            .as_ref()
            .unwrap()
            .plan_digest
    );
    receipt.validate(&candidate_context).unwrap();

    let evidence = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "m23-evidence",
            RegisterEvidenceRequest {
                candidate_original_job_id: candidate_original.clone(),
                candidate_replay_job_id: candidate_replay.clone(),
                baseline_original_job_id: baseline_original.clone(),
                baseline_replay_job_id: baseline_replay.clone(),
                expected_head_decision_id: None,
            },
        )
        .await
        .unwrap()
        .record;
    assert_eq!(
        evidence.schema_version,
        M23_PROMOTION_EVIDENCE_SCHEMA_VERSION
    );
    assert!(evidence.gates.overall_passed);
    let recovery_plan = crate::artifact_custody::derive_recovery_plan(
        &manager,
        TENANT,
        INCARNATION,
        &evidence.id,
        1_u64 << 30,
    )
    .await
    .unwrap();
    recovery_plan.validate().unwrap();
    assert_eq!(
        recovery_plan.evidence_schema_version,
        M23_PROMOTION_EVIDENCE_SCHEMA_VERSION
    );
    let preparation_authority = evidence
        .artifact_consumption
        .as_ref()
        .unwrap()
        .derivation
        .as_ref()
        .unwrap()
        .preparation
        .as_ref()
        .unwrap();
    assert_eq!(
        preparation_authority.candidate_original_preparation_digest,
        preparation_authority.candidate_replay_preparation_digest
    );
    assert_eq!(
        preparation_authority.candidate_original_preparation_digest,
        preparation_authority.baseline_original_preparation_digest
    );
    assert_eq!(
        preparation_authority.candidate_original_preparation_digest,
        preparation_authority.baseline_replay_preparation_digest
    );
    assert_eq!(
        preparation_authority.raw_document_set_digest,
        preparation.raw_document_set_digest
    );
    assert_eq!(
        preparation_authority.prepared_corpus_digest,
        preparation.prepared_corpus_blob_digest
    );

    let decision_key = "m23-promote";
    let intent = signed_promote_intent(
        &promoter,
        &evidence,
        decision_key,
        "promote the exactly reproduced M23 prepared corpus",
    );
    assert_eq!(intent.statement.domain, M23_PROMOTION_INTENT_DOMAIN);
    assert_eq!(
        intent
            .statement
            .payload
            .preparation_authority_digest
            .as_deref(),
        Some(preparation_authority.authority_digest.as_str())
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
        M23_PROMOTION_DECISION_SCHEMA_VERSION
    );
    let head = manager
        .current(TENANT, INCARNATION, &evidence.target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(head.schema_version, M23_PROMOTION_HEAD_SCHEMA_VERSION);
    assert_eq!(head.applied_decision_id, decision.id);

    let candidate_artifacts = candidate_context.artifact_attestations.clone().unwrap();
    let second_candidate_context = stored_prepared_context(
        &state,
        &cas_root.0,
        &attestor,
        "m23-candidate-second-selection",
        &policy_binding,
        Some(&candidate_artifacts),
        false,
    )
    .await;
    let second_candidate_original = submit_evaluation(
        &state,
        "m23-second-candidate-original",
        second_candidate_context.clone(),
    )
    .await;
    let second_candidate_replay = submit_evaluation(
        &state,
        "m23-second-candidate-replay",
        second_candidate_context,
    )
    .await;
    let second_evidence = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "m23-evidence-second-selection",
            RegisterEvidenceRequest {
                candidate_original_job_id: second_candidate_original,
                candidate_replay_job_id: second_candidate_replay,
                baseline_original_job_id: candidate_original,
                baseline_replay_job_id: candidate_replay,
                expected_head_decision_id: Some(decision.id.clone()),
            },
        )
        .await
        .unwrap()
        .record;
    let second_decision_key = "m23-promote-second-selection";
    let second_decision = manager
        .promote_signed(
            TENANT,
            INCARNATION,
            &second_evidence.id,
            promoter_actor(&promoter),
            second_decision_key,
            signed_promote_intent(
                &promoter,
                &second_evidence,
                second_decision_key,
                "exercise the M23 rollback predecessor chain",
            ),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(
        second_decision.schema_version,
        M23_PROMOTION_DECISION_SCHEMA_VERSION
    );
    assert_eq!(second_decision.action, PromotionAction::Promote);
    assert_eq!(
        manager
            .current(TENANT, INCARNATION, &evidence.target)
            .await
            .unwrap()
            .unwrap()
            .applied_decision_id,
        second_decision.id
    );

    let missing_authority_key = "m23-rollback-missing-preparation-authority";
    let mut missing_authority = signed_rollback_intent(
        &promoter,
        &evidence,
        &second_decision.id,
        missing_authority_key,
        "missing preparation authority must fail closed",
    );
    missing_authority
        .statement
        .payload
        .preparation_authority_digest = None;
    missing_authority.promoter_signature = promoter
        .signing_key
        .sign(&missing_authority.statement)
        .unwrap();
    let missing_authority_error = manager
        .rollback_signed(
            TENANT,
            INCARNATION,
            &evidence.target,
            promoter_actor(&promoter),
            missing_authority_key,
            missing_authority,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(missing_authority_error, CogniGraphError::Forbidden(_)),
        "unexpected missing M23 preparation-authority failure: {missing_authority_error}"
    );

    let rollback_key = "m23-rollback-second-to-first";
    let rollback = manager
        .rollback_signed(
            TENANT,
            INCARNATION,
            &evidence.target,
            promoter_actor(&promoter),
            rollback_key,
            signed_rollback_intent(
                &promoter,
                &evidence,
                &second_decision.id,
                rollback_key,
                "restore the first M23 evidence selection",
            ),
        )
        .await
        .unwrap()
        .record;
    assert_eq!(
        rollback.schema_version,
        M23_PROMOTION_DECISION_SCHEMA_VERSION
    );
    assert_eq!(rollback.action, PromotionAction::Rollback);
    let rollback_intent = rollback.governance.as_ref().unwrap();
    assert_eq!(
        rollback_intent.statement.domain,
        M23_PROMOTION_INTENT_DOMAIN
    );
    assert_eq!(
        rollback_intent
            .statement
            .payload
            .preparation_authority_digest
            .as_deref(),
        Some(preparation_authority.authority_digest.as_str())
    );
    let rolled_back_head = manager
        .current(TENANT, INCARNATION, &evidence.target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(rolled_back_head.applied_decision_id, rollback.id);
    assert_eq!(rolled_back_head.selection.evidence_id, evidence.id);

    state
        .jobs
        .recover_tenant(state.clone(), TENANT.into(), INCARNATION.into())
        .await
        .unwrap();
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    assert_eq!(
        manager.operator_status(TENANT, INCARNATION).await.unwrap()["m23_evidence_bundles"],
        2
    );

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
            Some("M23 snapshot preflight regression".into()),
        )
        .await
        .unwrap();
    assert_eq!(archived.archived, 7);

    let mut null_count_import = raw.export_snapshot().await.unwrap();
    let null_count_derivation = null_count_import
        .pointer_mut(&format!(
            "/collections/{JOB_ARCHIVE_COLLECTION}/documents/{unreferenced_job}/result/artifact_consumption/derivation"
        ))
        .and_then(Value::as_object_mut)
        .unwrap();
    null_count_derivation.insert("chunk_count".into(), Value::Null);
    null_count_import["collections"]["m23_null_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    assert!(
        manager
            .import_snapshot(TENANT, INCARNATION, &null_count_import)
            .await
            .is_err(),
        "M23 snapshot import accepted an explicit null forbidden chunk_count"
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m23_null_snapshot_should_not_apply/documents/marker")
            .is_none()
    );

    let mut count_tampered_import = raw.export_snapshot().await.unwrap();
    let count_tampered_job = count_tampered_import
        .pointer_mut(&format!("/collections/{JOB_ARCHIVE_COLLECTION}/documents"))
        .and_then(Value::as_object_mut)
        .unwrap()
        .get_mut(&unreferenced_job)
        .unwrap();
    let count_result = count_tampered_job["result"].as_object_mut().unwrap();
    let mut count_forged_receipt: crate::artifact_consumption::ArtifactConsumptionReceipt =
        serde_json::from_value(count_result["artifact_consumption"].clone()).unwrap();
    let count_forged_derivation = count_forged_receipt.derivation.as_mut().unwrap();
    count_forged_derivation.chunk_count = Some(1);
    count_forged_derivation.derivation_material_digest = record_digest(
        count_forged_derivation.as_ref(),
        "derivation_material_digest",
    )
    .unwrap();
    count_forged_receipt.material_digest = count_forged_receipt.expected_material_digest().unwrap();
    count_forged_receipt.receipt_digest =
        record_digest(&count_forged_receipt, "receipt_digest").unwrap();
    count_result.insert(
        "artifact_consumption".into(),
        serde_json::to_value(count_forged_receipt).unwrap(),
    );
    count_tampered_import["collections"]["m23_count_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    let count_tamper_error = manager
        .import_snapshot(TENANT, INCARNATION, &count_tampered_import)
        .await
        .unwrap_err();
    assert!(
        count_tamper_error
            .to_string()
            .contains("corpus-to-graph derivation receipt does not match"),
        "unexpected self-consistent M23 chunk-count failure: {count_tamper_error}"
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m23_count_snapshot_should_not_apply/documents/marker")
            .is_none()
    );

    let mut tampered_import = raw.export_snapshot().await.unwrap();
    let tampered_job = tampered_import
        .pointer_mut(&format!("/collections/{JOB_ARCHIVE_COLLECTION}/documents"))
        .and_then(Value::as_object_mut)
        .unwrap()
        .get_mut(&unreferenced_job)
        .unwrap();
    let result = tampered_job["result"].as_object_mut().unwrap();
    let mut forged_receipt: crate::artifact_consumption::ArtifactConsumptionReceipt =
        serde_json::from_value(result["artifact_consumption"].clone()).unwrap();
    let forged_derivation = forged_receipt.derivation.as_mut().unwrap();
    let forged_preparation = forged_derivation.preparation.as_mut().unwrap();
    forged_preparation.raw_document_set_digest = digest("m23-forged-raw-document-set");
    forged_preparation.preparation_read_set_digest = canonical_digest(&json!({
        "corpus_manifest_digest": &forged_preparation.corpus_manifest_digest,
        "documents_blob_digest": &forged_preparation.documents_blob_digest,
        "documents_semantic_digest": &forged_preparation.documents_semantic_digest,
        "raw_document_set_digest": &forged_preparation.raw_document_set_digest,
        "preparation_plan_digest": &forged_preparation.preparation_plan_digest,
    }))
    .unwrap();
    forged_preparation.preparation_material_digest =
        record_digest(forged_preparation.as_ref(), "preparation_material_digest").unwrap();
    forged_derivation.construction_read_set_digest = canonical_digest(&json!({
        "corpus_manifest_digest": &forged_derivation.corpus_manifest_digest,
        "corpus_blob_digest": &forged_derivation.corpus_blob_digest,
        "corpus_semantic_digest": &forged_derivation.corpus_semantic_digest,
        "candidate_blob_digest": &forged_derivation.candidate_blob_digest,
        "candidate_semantic_digest": &forged_derivation.candidate_semantic_digest,
        "resolved_config_digest": &forged_derivation.resolved_config_digest,
        "derivation_plan_digest": &forged_derivation.derivation_plan_digest,
        "preparation_material_digest": &forged_preparation.preparation_material_digest,
    }))
    .unwrap();
    forged_derivation.derivation_material_digest =
        record_digest(forged_derivation.as_ref(), "derivation_material_digest").unwrap();
    forged_receipt.material_digest = forged_receipt.expected_material_digest().unwrap();
    forged_receipt.receipt_digest = record_digest(&forged_receipt, "receipt_digest").unwrap();
    result.insert(
        "artifact_consumption".into(),
        serde_json::to_value(forged_receipt).unwrap(),
    );
    tampered_import["collections"]["m23_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    let tamper_error = manager
        .import_snapshot(TENANT, INCARNATION, &tampered_import)
        .await
        .unwrap_err();
    assert!(
        tamper_error
            .to_string()
            .contains("raw-document preparation receipt does not match"),
        "unexpected self-consistent M23 receipt-tamper failure: {tamper_error}"
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m23_snapshot_should_not_apply/documents/marker")
            .is_none()
    );

    let corpus_attestation = manager
        .get_artifact_attestation(
            TENANT,
            INCARNATION,
            &candidate_context
                .artifact_attestations
                .as_ref()
                .unwrap()
                .corpus
                .attestation_id,
        )
        .await
        .unwrap();
    let documents_entry = corpus_attestation
        .manifest
        .entries
        .iter()
        .find(|entry| entry.logical_path == DOCUMENTS_ENTRYPOINT)
        .unwrap();
    let documents_path = staged_blob_path(&cas_root.0, &documents_entry.blob_digest);
    let original_documents = fs::read(&documents_path).unwrap();
    fs::write(&documents_path, b"tampered raw package").unwrap();
    let cas_tampered = state
        .jobs
        .submit(
            state.clone(),
            TENANT.into(),
            INCARNATION.into(),
            JobActor::request(None),
            "m23-cas-tamper",
            JobKind::ConstructEvaluate,
            json!({
                "space_type": SPACE,
                "eval": eval_spec(),
                "promotion_context": candidate_context.clone(),
            }),
        )
        .await
        .unwrap();
    let cas_tampered = wait_terminal(&state, &cas_tampered.job.id).await;
    fs::write(&documents_path, original_documents).unwrap();
    assert_eq!(cas_tampered.status, JobStatus::Failed);
    assert!(cas_tampered.result.is_none());

    let inconsistent_context = stored_prepared_context(
        &state,
        &cas_root.0,
        &attestor,
        "m23-inconsistent",
        &policy_binding,
        Some(&baseline_artifacts),
        true,
    )
    .await;
    let inconsistent = state
        .jobs
        .submit(
            state.clone(),
            TENANT.into(),
            INCARNATION.into(),
            JobActor::request(None),
            "m23-inconsistent-prepared-output",
            JobKind::ConstructEvaluate,
            json!({
                "space_type": SPACE,
                "eval": eval_spec(),
                "promotion_context": inconsistent_context,
            }),
        )
        .await
        .unwrap();
    let inconsistent = wait_terminal(&state, &inconsistent.job.id).await;
    assert_eq!(inconsistent.status, JobStatus::Failed);
    assert!(inconsistent.result.is_none());
    assert!(
        inconsistent
            .error
            .as_ref()
            .unwrap()
            .to_string()
            .contains("does not exactly reproduce from the verified raw document bytes"),
        "unexpected M23 prepared-output mismatch: {:?}",
        inconsistent.error
    );
    state.jobs.shutdown().await;
}
