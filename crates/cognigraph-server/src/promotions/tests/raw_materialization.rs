//! Raw materialization.

use super::*;

#[test]
fn m26_build_accepts_exact_m23_and_rejects_invalid_sources_without_writes() {
    std::thread::Builder::new()
        .name("m26-m23-build-matrix".into())
        .stack_size(16 * 1024 * 1024)
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
                let mut state = AppState::new_shared(raw.clone());
                state.artifact_cas = Some(Arc::new(
                    LocalArtifactCas::open(&cas_root.0, 1_u64 << 30).unwrap(),
                ));
                state.artifact_executable_digest = Some(current_executable_digest().await.unwrap());
                let manager = state.promotions.clone();
                let (binding, promoter, root) = governed_policy_binding(&state).await;
                let attestor = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::ArtifactAttestor,
                    Role::ArtifactAttestor,
                    "m26-m23-artifact-attestor-principal",
                    "m26-m23-artifact-attestor-user",
                )
                .await;
                let repair_author = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::PolicyAuthor,
                    Role::PolicyAuthor,
                    "m26-m23-repair-author-principal",
                    "m26-m23-repair-author-user",
                )
                .await;
                let repair_approver = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::PolicyApprover,
                    Role::PolicyApprover,
                    "m26-m23-repair-approver-principal",
                    "m26-m23-repair-approver-user",
                )
                .await;
                let target = PromotionTarget {
                    space_type: SPACE.into(),
                    channel: "stable".into(),
                };
                let baseline_context = stored_reproducible_context_variant(
                    &state,
                    &cas_root.0,
                    &attestor,
                    "m26-m23-baseline",
                    &binding,
                    None,
                    false,
                    true,
                    false,
                    ReproducibleContextVariant::Empty,
                )
                .await;
                let shared_artifacts = baseline_context.artifact_attestations.clone().unwrap();
                let candidate_context = stored_reproducible_context_variant(
                    &state,
                    &cas_root.0,
                    &attestor,
                    "m26-m23-candidate",
                    &binding,
                    Some(&shared_artifacts),
                    false,
                    true,
                    false,
                    ReproducibleContextVariant::Supply,
                )
                .await;
                let candidate =
                    stored_construction_candidate(&state, &cas_root.0, &candidate_context).await;
                let revision = create_test_semantic_repair_revision(
                    &state,
                    &repair_author,
                    &target,
                    None,
                    candidate,
                    "m26-m23-revision",
                )
                .await;
                let baseline_original = submit_evaluation(
                    &state,
                    "m26-m23-baseline-original",
                    baseline_context.clone(),
                )
                .await;
                let baseline_replay =
                    submit_evaluation(&state, "m26-m23-baseline-replay", baseline_context).await;
                let candidate_original = submit_evaluation(
                    &state,
                    "m26-m23-candidate-original",
                    candidate_context.clone(),
                )
                .await;
                let candidate_replay = submit_evaluation(
                    &state,
                    "m26-m23-candidate-replay",
                    candidate_context.clone(),
                )
                .await;
                let evidence = manager
                    .register_evidence(
                        TENANT,
                        INCARNATION,
                        promoter_actor(&promoter),
                        "m26-m23-evidence",
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
                    M23_PROMOTION_EVIDENCE_SCHEMA_VERSION
                );
                let preparation = evidence
                    .artifact_consumption
                    .as_ref()
                    .and_then(|authority| authority.derivation.as_ref())
                    .and_then(|authority| authority.preparation.as_ref())
                    .expect("M23 evidence retains exact raw-document preparation authority");
                let promote_key = "m26-m23-promote";
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
                            "select exact M23 authority for verified materialization",
                        ),
                    )
                    .await
                    .unwrap()
                    .record;
                manager
                    .ensure_materialized_repair_repository(TENANT)
                    .await
                    .unwrap();

                let request = BuildSemanticRepairGenerationRequest {
                    target: target.clone(),
                    expected_promotion_head_decision_id: decision.id.clone(),
                };
                let unapproved = rejected_m26_build_preserves_state(
                    &manager,
                    state.artifact_cas.as_deref().unwrap(),
                    raw.as_ref(),
                    promoter.actor.clone(),
                    "m26-m23-build-unapproved",
                    request.clone(),
                )
                .await;
                assert!(
                    unapproved.to_string().contains("approved"),
                    "unexpected unapproved-M25 M26 build failure: {unapproved}"
                );
                let review = approve_test_semantic_repair_revision(
                    &state,
                    &repair_approver,
                    &revision,
                    "m26-m23-review",
                )
                .await;

                let stale_head = rejected_m26_build_preserves_state(
                    &manager,
                    state.artifact_cas.as_deref().unwrap(),
                    raw.as_ref(),
                    promoter.actor.clone(),
                    "m26-m23-build-stale-head",
                    BuildSemanticRepairGenerationRequest {
                        target: target.clone(),
                        expected_promotion_head_decision_id: digest("m26-stale-head")
                            .trim_start_matches("sha256:")
                            .into(),
                    },
                )
                .await;
                assert!(
                    stale_head
                        .to_string()
                        .contains("does not match the current promotion head"),
                    "unexpected stale-head M26 build failure: {stale_head}"
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
                let corpus_entry = corpus_attestation
                    .manifest
                    .entries
                    .iter()
                    .find(|entry| entry.logical_path == CORPUS_ENTRYPOINT)
                    .unwrap();
                let corpus_path = staged_blob_path(&cas_root.0, &corpus_entry.blob_digest);
                let canonical_corpus = fs::read(&corpus_path).unwrap();

                let missing_backup = corpus_path.with_file_name(format!(
                    "{}.m26-missing-backup",
                    corpus_path.file_name().unwrap().to_string_lossy()
                ));
                fs::rename(&corpus_path, &missing_backup).unwrap();
                let missing = rejected_m26_build_preserves_state(
                    &manager,
                    state.artifact_cas.as_deref().unwrap(),
                    raw.as_ref(),
                    promoter.actor.clone(),
                    "m26-m23-build-missing-corpus",
                    request.clone(),
                )
                .await;
                fs::rename(&missing_backup, &corpus_path).unwrap();
                assert!(
                    missing
                        .to_string()
                        .contains("cannot inspect staged artifact blob"),
                    "unexpected missing-corpus M26 build failure: {missing}"
                );

                let mut wrong_length = canonical_corpus.clone();
                wrong_length.push(b'\n');
                fs::write(&corpus_path, &wrong_length).unwrap();
                let wrong_length = rejected_m26_build_preserves_state(
                    &manager,
                    state.artifact_cas.as_deref().unwrap(),
                    raw.as_ref(),
                    promoter.actor.clone(),
                    "m26-m23-build-wrong-length-corpus",
                    request.clone(),
                )
                .await;
                fs::write(&corpus_path, &canonical_corpus).unwrap();
                assert!(
                    wrong_length
                        .to_string()
                        .contains("does not match declared length"),
                    "unexpected wrong-length M26 build failure: {wrong_length}"
                );

                let mut wrong_digest = canonical_corpus.clone();
                wrong_digest[0] = if wrong_digest[0] == b'{' { b'[' } else { b'{' };
                fs::write(&corpus_path, &wrong_digest).unwrap();
                let wrong_digest = rejected_m26_build_preserves_state(
                    &manager,
                    state.artifact_cas.as_deref().unwrap(),
                    raw.as_ref(),
                    promoter.actor.clone(),
                    "m26-m23-build-wrong-digest-corpus",
                    request.clone(),
                )
                .await;
                fs::write(&corpus_path, &canonical_corpus).unwrap();
                assert!(
                    wrong_digest.to_string().contains("digest mismatch"),
                    "unexpected wrong-digest M26 build failure: {wrong_digest}"
                );

                let canonical_text = String::from_utf8(canonical_corpus.clone()).unwrap();
                let noncanonical_text = canonical_text
                    .replacen("\"schema_version\":1", "\"schema_version\" :1", 1)
                    .replacen("Meridian supply", "Meridiansupply", 1);
                let noncanonical = noncanonical_text.into_bytes();
                assert_eq!(noncanonical.len(), canonical_corpus.len());
                let parsed_noncanonical: PreparedChunkCorpusArtifact =
                    serde_json::from_slice(&noncanonical).unwrap();
                assert_ne!(
                    canonical_json_bytes(&parsed_noncanonical).unwrap(),
                    noncanonical,
                    "test corruption must remain valid but noncanonical JSON"
                );
                fs::write(&corpus_path, &noncanonical).unwrap();
                let noncanonical_error = rejected_m26_build_preserves_state(
                    &manager,
                    state.artifact_cas.as_deref().unwrap(),
                    raw.as_ref(),
                    promoter.actor.clone(),
                    "m26-m23-build-noncanonical-corpus",
                    request.clone(),
                )
                .await;
                fs::write(&corpus_path, &canonical_corpus).unwrap();
                assert!(
                    noncanonical_error.to_string().contains("digest mismatch"),
                    "unexpected noncanonical-corpus M26 build failure: {noncanonical_error}"
                );

                let before_success = observe_m26_build_state(raw.as_ref()).await;
                let built = manager
                    .build_semantic_repair_generation(
                        state.artifact_cas.as_deref().unwrap(),
                        TENANT,
                        INCARNATION,
                        promoter.actor.clone(),
                        "m26-m23-build-exact",
                        request,
                    )
                    .await
                    .unwrap();
                assert!(!built.replayed);
                let generation = built.record;
                assert_eq!(generation.source_evidence_id, evidence.id);
                assert_eq!(generation.promotion_head_decision_id, decision.id);
                assert_eq!(
                    generation.semantic_repair_revision_id,
                    revision.semantic_repair_revision_id
                );
                assert_eq!(
                    generation.semantic_repair_review_id,
                    review.semantic_repair_review_id
                );
                assert_eq!(
                    generation.prepared_corpus_digest,
                    preparation.prepared_corpus_digest
                );
                assert_eq!(generation.projection.chunk_count, 1);
                assert_eq!(generation.projection.fact_count, 1);
                assert_eq!(generation.projection.semantic_facts.len(), 1);
                assert_eq!(generation.impact.added_count, 1);
                assert_eq!(generation.impact.removed_count, 0);
                let after_success = observe_m26_build_state(raw.as_ref()).await;
                assert_eq!(after_success.authority, before_success.authority);
                assert_eq!(after_success.graph, before_success.graph);
                assert_eq!(
                    after_success.generations.len(),
                    before_success.generations.len() + 1
                );

                let forged_context = stored_reproducible_context(
                    &state,
                    &cas_root.0,
                    &attestor,
                    "m26-m23-forged-receipt",
                    &binding,
                    Some(&shared_artifacts),
                    true,
                    true,
                    false,
                )
                .await;
                let before_forged = observe_m26_build_state(raw.as_ref()).await;
                let forged = state
                    .jobs
                    .submit(
                        state.clone(),
                        TENANT.into(),
                        INCARNATION.into(),
                        JobActor::request(None),
                        "m26-m23-forged-receipt-source",
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
                    "unexpected candidate/receipt source failure: {:?}",
                    forged.error
                );
                assert_eq!(
                    observe_m26_build_state(raw.as_ref()).await,
                    before_forged,
                    "candidate/receipt mismatch mutated M26 authority or graph rows"
                );
                state.jobs.shutdown().await;
            });
        })
        .unwrap()
        .join()
        .unwrap();
}
