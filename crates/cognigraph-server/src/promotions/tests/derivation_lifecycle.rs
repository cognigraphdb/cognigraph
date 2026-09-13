//! Derivation lifecycle.

use super::*;

#[tokio::test]
async fn m22_reproducible_derivation_drives_promotion_and_rejects_forged_graphs() {
    let cas_root = TestArtifactCasRoot::new();
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    raw.create_edge(
        "facts",
        json!({
            "_key": "m22-live-forbidden-only",
            "_from": "entities/meridian",
            "_to": "entities/compound-x",
            "relation_type": "OWNS",
            "space_id": SPACE,
            "evidence_chunk_id": "m22-live-forbidden-chunk",
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
        "m22-artifact-attestor-principal",
        "m22-artifact-attestor-user",
    )
    .await;

    let baseline_context = stored_derived_context(
        &state,
        &cas_root.0,
        &attestor,
        "m22-baseline",
        &policy_binding,
        None,
        false,
    )
    .await;
    let baseline_artifacts = baseline_context.artifact_attestations.clone().unwrap();
    let candidate_context = stored_derived_context(
        &state,
        &cas_root.0,
        &attestor,
        "m22-candidate",
        &policy_binding,
        Some(&baseline_artifacts),
        false,
    )
    .await;

    let baseline_original =
        submit_evaluation(&state, "m22-baseline-original", baseline_context.clone()).await;
    let baseline_replay = submit_evaluation(&state, "m22-baseline-replay", baseline_context).await;
    let candidate_original =
        submit_evaluation(&state, "m22-candidate-original", candidate_context.clone()).await;
    let candidate_replay =
        submit_evaluation(&state, "m22-candidate-replay", candidate_context.clone()).await;
    let unreferenced_job = submit_evaluation(
        &state,
        "m22-unreferenced-valid-job",
        candidate_context.clone(),
    )
    .await;
    let candidate_job = state
        .jobs
        .get(TENANT, INCARNATION, &candidate_original)
        .await
        .unwrap();
    assert_eq!(candidate_job.schema_version, 3);
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
        M22_CONSUMPTION_RECEIPT_SCHEMA_VERSION
    );
    let derivation = receipt.derivation.as_ref().unwrap();
    assert_eq!(derivation.chunk_count, Some(1));
    assert_eq!(derivation.fact_count, 1);
    assert_eq!(
        derivation.claimed_graph_blob_digest,
        derivation.derived_graph_blob_digest
    );
    receipt.validate(&candidate_context).unwrap();

    let evidence = manager
        .register_evidence(
            TENANT,
            INCARNATION,
            promoter_actor(&promoter),
            "m22-evidence",
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
        M22_PROMOTION_EVIDENCE_SCHEMA_VERSION
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
        M22_PROMOTION_EVIDENCE_SCHEMA_VERSION
    );
    let derivation_authority = evidence
        .artifact_consumption
        .as_ref()
        .unwrap()
        .derivation
        .as_ref()
        .unwrap();
    assert_eq!(
        derivation_authority.candidate_original_derivation_digest,
        derivation_authority.candidate_replay_derivation_digest
    );

    let decision_key = "m22-promote";
    let intent = signed_promote_intent(
        &promoter,
        &evidence,
        decision_key,
        "promote the exactly reproduced M22 candidate",
    );
    assert_eq!(intent.statement.domain, M22_PROMOTION_INTENT_DOMAIN);
    assert_eq!(
        intent
            .statement
            .payload
            .derivation_authority_digest
            .as_deref(),
        Some(derivation_authority.authority_digest.as_str())
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
        M22_PROMOTION_DECISION_SCHEMA_VERSION
    );
    let head = manager
        .current(TENANT, INCARNATION, &evidence.target)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(head.schema_version, M22_PROMOTION_HEAD_SCHEMA_VERSION);
    assert_eq!(head.applied_decision_id, decision.id);

    state
        .jobs
        .recover_tenant(state.clone(), TENANT.into(), INCARNATION.into())
        .await
        .unwrap();
    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
    assert_eq!(
        manager.operator_status(TENANT, INCARNATION).await.unwrap()["m22_evidence_bundles"],
        1
    );

    let mut tampered_job = serde_json::to_value(&candidate_job).unwrap();
    tampered_job["result"]["artifact_consumption"]["derivation"]["derived_graph_blob_digest"] =
        json!(digest("m22-tampered-derived-graph"));
    assert!(
        state
            .jobs
            .promotion_evaluation_source_from_snapshot_value(
                TENANT,
                INCARNATION,
                tampered_job,
                false,
            )
            .is_err()
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
            Some("M22 snapshot preflight regression".into()),
        )
        .await
        .unwrap();
    assert_eq!(archived.archived, 5);

    let mut null_plan_import = raw.export_snapshot().await.unwrap();
    let null_plan = null_plan_import
        .pointer_mut(&format!(
            "/collections/{JOB_ARCHIVE_COLLECTION}/documents/{unreferenced_job}/_execution/promotion_context/consumption_plan"
        ))
        .and_then(Value::as_object_mut)
        .unwrap();
    null_plan.insert("preparation".into(), Value::Null);
    null_plan_import["collections"]["m22_null_plan_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    assert!(
        manager
            .import_snapshot(TENANT, INCARNATION, &null_plan_import)
            .await
            .is_err(),
        "M22 snapshot import accepted an explicit null forbidden preparation plan"
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m22_null_plan_snapshot_should_not_apply/documents/marker")
            .is_none()
    );

    let mut null_preparation_import = raw.export_snapshot().await.unwrap();
    let null_preparation_derivation = null_preparation_import
        .pointer_mut(&format!(
            "/collections/{JOB_ARCHIVE_COLLECTION}/documents/{unreferenced_job}/result/artifact_consumption/derivation"
        ))
        .and_then(Value::as_object_mut)
        .unwrap();
    null_preparation_derivation.insert("preparation".into(), Value::Null);
    null_preparation_import["collections"]["m22_null_snapshot_should_not_apply"] = json!({
        "type": "document",
        "documents": {"marker": {"_key": "marker", "value": 1}}
    });
    assert!(
        manager
            .import_snapshot(TENANT, INCARNATION, &null_preparation_import)
            .await
            .is_err(),
        "M22 snapshot import accepted an explicit null forbidden preparation receipt"
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m22_null_snapshot_should_not_apply/documents/marker")
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
    result.get_mut("recall").unwrap()["found"] = json!(0);
    result.insert("recall_ok".into(), json!(false));
    result.insert(
        "missing".into(),
        json!(["Meridian --SUPPLIES--> Compound X"]),
    );
    let mut receipt: crate::artifact_consumption::ArtifactConsumptionReceipt =
        serde_json::from_value(result["artifact_consumption"].clone()).unwrap();
    let derivation = receipt.derivation.as_mut().unwrap();
    // Preserve every signed manifest and exact blob address. Rewriting
    // only the server-authored fact/result projection and recomputing all
    // unkeyed receipt hashes must still fail against graph.json.
    derivation.facts.clear();
    derivation.facts_digest = canonical_digest(&Vec::<VerifiedGraphFact>::new()).unwrap();
    derivation.fact_count = 0;
    derivation.derivation_material_digest =
        record_digest(derivation.as_ref(), "derivation_material_digest").unwrap();
    let mut result_without_receipt = Value::Object(result.clone());
    result_without_receipt
        .as_object_mut()
        .unwrap()
        .remove("artifact_consumption");
    receipt.evaluation_result_digest = canonical_digest(&result_without_receipt).unwrap();
    receipt.material_digest = receipt.expected_material_digest().unwrap();
    receipt.receipt_digest = record_digest(&receipt, "receipt_digest").unwrap();
    result.insert(
        "artifact_consumption".into(),
        serde_json::to_value(receipt).unwrap(),
    );
    tampered_import["collections"]["m22_snapshot_should_not_apply"] = json!({
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
            .contains("facts do not reconstruct the signed graph manifest entry"),
        "unexpected self-consistent M22 snapshot-tamper failure: {tamper_error}"
    );
    assert!(
        raw.export_snapshot()
            .await
            .unwrap()
            .pointer("/collections/m22_snapshot_should_not_apply/documents/marker")
            .is_none()
    );

    let forged_context = stored_derived_context(
        &state,
        &cas_root.0,
        &attestor,
        "m22-forged",
        &policy_binding,
        Some(&baseline_artifacts),
        true,
    )
    .await;
    let forged = state
        .jobs
        .submit(
            state.clone(),
            TENANT.into(),
            INCARNATION.into(),
            JobActor::request(None),
            "m22-forged-evidence-row",
            JobKind::ConstructEvaluate,
            json!({
                "space_type": SPACE,
                "eval": eval_spec(),
                "promotion_context": forged_context,
            }),
        )
        .await
        .unwrap();
    let forged = wait_terminal(&state, &forged.job.id).await;
    assert_eq!(forged.status, JobStatus::Failed);
    assert!(forged.result.is_none());
    assert!(
        forged
            .error
            .as_ref()
            .unwrap()
            .to_string()
            .contains("does not exactly reproduce"),
        "unexpected M22 forged-graph failure: {:?}",
        forged.error
    );
    state.jobs.shutdown().await;
}
