//! Legacy materialization.

use super::*;

#[test]
fn m26_build_rejects_pre_m22_selection_without_mutating_authority_or_graph() {
    std::thread::Builder::new()
        .name("m26-pre-m22-build-matrix".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                let cas_root = TestArtifactCasRoot::new();
                let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
                for (collection, kind) in [
                    ("entities", CollectionType::Document),
                    ("chunks", CollectionType::Document),
                    ("mentions", CollectionType::Edge),
                    ("facts", CollectionType::Edge),
                ] {
                    raw.ensure_collection(collection, kind).await.unwrap();
                }
                raw.create_edge(
                    "facts",
                    json!({
                        "_key": "m26-pre-m22-live-fact",
                        "_from": "entities/meridian",
                        "_to": "entities/compound-x",
                        "relation_type": "SUPPLIES",
                        "space_id": SPACE,
                        "evidence_chunk_id": "m26-pre-m22-live-chunk",
                    }),
                )
                .await
                .unwrap();
                let mut state = AppState::new_shared(raw.clone());
                state.artifact_cas = Some(Arc::new(
                    LocalArtifactCas::open(&cas_root.0, 1_u64 << 30).unwrap(),
                ));
                let manager = state.promotions.clone();
                let (binding, promoter, root) = governed_policy_binding(&state).await;
                let repair_author = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::PolicyAuthor,
                    Role::PolicyAuthor,
                    "m26-pre-m22-repair-author-principal",
                    "m26-pre-m22-repair-author-user",
                )
                .await;
                let repair_approver = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::PolicyApprover,
                    Role::PolicyApprover,
                    "m26-pre-m22-repair-approver-principal",
                    "m26-pre-m22-repair-approver-user",
                )
                .await;
                let target = PromotionTarget {
                    space_type: SPACE.into(),
                    channel: "stable".into(),
                };
                let (baseline_context, _) =
                    governed_semantic_context("m26-pre-m22-baseline", &binding);
                let (candidate_context, candidate) =
                    governed_semantic_context("m26-pre-m22-candidate", &binding);
                let revision = create_test_semantic_repair_revision(
                    &state,
                    &repair_author,
                    &target,
                    None,
                    candidate,
                    "m26-pre-m22-revision",
                )
                .await;
                approve_test_semantic_repair_revision(
                    &state,
                    &repair_approver,
                    &revision,
                    "m26-pre-m22-review",
                )
                .await;
                let baseline_original = submit_evaluation(
                    &state,
                    "m26-pre-m22-baseline-original",
                    baseline_context.clone(),
                )
                .await;
                let baseline_replay =
                    submit_evaluation(&state, "m26-pre-m22-baseline-replay", baseline_context)
                        .await;
                let candidate_original = submit_evaluation(
                    &state,
                    "m26-pre-m22-candidate-original",
                    candidate_context.clone(),
                )
                .await;
                let candidate_replay =
                    submit_evaluation(&state, "m26-pre-m22-candidate-replay", candidate_context)
                        .await;
                let evidence = manager
                    .register_evidence(
                        TENANT,
                        INCARNATION,
                        promoter_actor(&promoter),
                        "m26-pre-m22-evidence",
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
                    M19_PROMOTION_EVIDENCE_SCHEMA_VERSION
                );
                let promote_key = "m26-pre-m22-promote";
                let decision = manager
                    .promote_signed(
                        TENANT,
                        INCARNATION,
                        &evidence.id,
                        promoter_actor(&promoter),
                        promote_key,
                        signed_promote_intent(
                            &promoter,
                            &evidence,
                            promote_key,
                            "select governed pre-M22 authority",
                        ),
                    )
                    .await
                    .unwrap()
                    .record;
                manager
                    .ensure_materialized_repair_repository(TENANT)
                    .await
                    .unwrap();
                let error = rejected_m26_build_preserves_state(
                    &manager,
                    state.artifact_cas.as_deref().unwrap(),
                    raw.as_ref(),
                    promoter.actor.clone(),
                    "m26-pre-m22-build",
                    BuildSemanticRepairGenerationRequest {
                        target,
                        expected_promotion_head_decision_id: decision.id,
                    },
                )
                .await;
                assert!(
                    error
                        .to_string()
                        .contains("requires one consistent M22 or M23 evaluation authority"),
                    "unexpected pre-M22 M26 build failure: {error}"
                );
                state.jobs.shutdown().await;
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
