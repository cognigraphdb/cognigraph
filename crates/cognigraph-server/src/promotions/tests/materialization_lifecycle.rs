//! Materialization lifecycle.

use super::*;

#[test]
fn verified_semantic_repair_materializes_switches_and_recovers_exact_authority() {
    std::thread::Builder::new()
        .name("m26-lifecycle-test".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let lifecycle = async {
                let cas_root = TestArtifactCasRoot::new();
                let fail_next_batch = Arc::new(AtomicBool::new(false));
                let raw: Arc<dyn GraphBackend> = Arc::new(FailNextBatchBackend {
                    inner: Arc::new(NativeBackend::new()),
                    fail_next_batch: fail_next_batch.clone(),
                });
                let mut state = AppState::new_shared(raw.clone());
                let live_auth = AuthProvider::new(raw.clone()).await.unwrap();
                let live_promoter = live_auth
                    .create_user(
                        "test-promoter-user@example.test",
                        "m26-live-password",
                        Role::Promoter,
                    )
                    .await
                    .unwrap();
                let mut live_promoter_document = raw
                    .get_document("_users", &live_promoter.key)
                    .await
                    .unwrap()
                    .unwrap();
                assert!(raw
                    .delete_document("_users", &live_promoter.key)
                    .await
                    .unwrap());
                live_promoter_document["_key"] = json!("test-promoter-user");
                raw.create_document("_users", live_promoter_document)
                    .await
                    .unwrap();
                state.artifact_cas = Some(Arc::new(
                    LocalArtifactCas::open(&cas_root.0, 1_u64 << 30).unwrap(),
                ));
                state.artifact_executable_digest =
                    Some(current_executable_digest().await.unwrap());
                let manager = state.promotions.clone();
                let (binding, promoter, root) = governed_policy_binding(&state).await;
                let attestor = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::ArtifactAttestor,
                    Role::ArtifactAttestor,
                    "m26-artifact-attestor-principal",
                    "m26-artifact-attestor-user",
                )
                .await;
                let repair_author = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::PolicyAuthor,
                    Role::PolicyAuthor,
                    "m26-repair-author-principal",
                    "m26-repair-author-user",
                )
                .await;
                let repair_approver = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::PolicyApprover,
                    Role::PolicyApprover,
                    "m26-repair-approver-principal",
                    "m26-repair-approver-user",
                )
                .await;
                let alternate_promoter = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::Promoter,
                    Role::Promoter,
                    "m26-alternate-promoter-principal",
                    "m26-alternate-promoter-user",
                )
                .await;
                let wrong_purpose_promoter = staged_test_governance_principal(
                    &root,
                    KeyPurpose::PolicyAuthor,
                    Role::Promoter,
                    "m26-wrong-purpose-principal",
                    "m26-wrong-purpose-promoter-user",
                    "m26-wrong-purpose-promoter-key",
                );
                let cross_duty_promoters = [
                    (
                        "author",
                        staged_test_governance_principal(
                            &root,
                            KeyPurpose::Promoter,
                            Role::Promoter,
                            &repair_author.record.principal_id,
                            "m26-author-principal-promoter-user",
                            "m26-author-principal-promoter-key",
                        ),
                    ),
                    (
                        "approver",
                        staged_test_governance_principal(
                            &root,
                            KeyPurpose::Promoter,
                            Role::Promoter,
                            &repair_approver.record.principal_id,
                            "m26-approver-principal-promoter-user",
                            "m26-approver-principal-promoter-key",
                        ),
                    ),
                    (
                        "artifact-attestor",
                        staged_test_governance_principal(
                            &root,
                            KeyPurpose::Promoter,
                            Role::Promoter,
                            &attestor.record.principal_id,
                            "m26-attestor-principal-promoter-user",
                            "m26-attestor-principal-promoter-key",
                        ),
                    ),
                ];
                let revoked_promoter = register_test_governance_principal(
                    &state,
                    &root,
                    KeyPurpose::Promoter,
                    Role::Promoter,
                    "m26-revoked-promoter-principal",
                    "m26-revoked-promoter-user",
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
                    "m26-baseline-empty",
                    &binding,
                    None,
                    false,
                    false,
                    false,
                    ReproducibleContextVariant::EmptyWithAll,
                )
                .await;
                let shared_artifacts = baseline_context.artifact_attestations.clone().unwrap();
                let candidate_a_context = stored_reproducible_context_variant(
                    &state,
                    &cas_root.0,
                    &attestor,
                    "m26-candidate-a",
                    &binding,
                    Some(&shared_artifacts),
                    false,
                    false,
                    false,
                    ReproducibleContextVariant::SupplyAndDistribute,
                )
                .await;
                let candidate_a =
                    stored_construction_candidate(&state, &cas_root.0, &candidate_a_context)
                        .await;
                let revision_a = create_test_semantic_repair_revision(
                    &state,
                    &repair_author,
                    &target,
                    None,
                    candidate_a,
                    "m26-create-revision-a",
                )
                .await;
                approve_test_semantic_repair_revision(
                    &state,
                    &repair_approver,
                    &revision_a,
                    "m26-approve-revision-a",
                )
                .await;

                let baseline_original = submit_evaluation(
                    &state,
                    "m26-baseline-original",
                    baseline_context.clone(),
                )
                .await;
                let baseline_replay =
                    submit_evaluation(&state, "m26-baseline-replay", baseline_context).await;
                let candidate_a_original = submit_evaluation(
                    &state,
                    "m26-candidate-a-original",
                    candidate_a_context.clone(),
                )
                .await;
                let candidate_a_replay =
                    submit_evaluation(&state, "m26-candidate-a-replay", candidate_a_context)
                        .await;
                let evidence_a = manager
                    .register_evidence(
                        TENANT,
                        INCARNATION,
                        promoter_actor(&promoter),
                        "m26-evidence-a",
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
                let promote_a_key = "m26-promote-a";
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
                            "select exact M26 candidate A",
                        ),
                    )
                    .await
                    .unwrap()
                    .record;
                let promotion_head_a = manager
                    .current(TENANT, INCARNATION, &target)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(promotion_head_a.applied_decision_id, promotion_a.id);

                let build_a_key = "m26-build-generation-a";
                let build_a_request = BuildSemanticRepairGenerationRequest {
                    target: target.clone(),
                    expected_promotion_head_decision_id: promotion_a.id.clone(),
                };
                let build_a_left = manager.build_semantic_repair_generation(
                    state.artifact_cas.as_deref().unwrap(),
                    TENANT,
                    INCARNATION,
                    promoter.actor.clone(),
                    build_a_key,
                    build_a_request.clone(),
                );
                let build_a_right = manager.build_semantic_repair_generation(
                    state.artifact_cas.as_deref().unwrap(),
                    TENANT,
                    INCARNATION,
                    promoter.actor.clone(),
                    build_a_key,
                    build_a_request.clone(),
                );
                let (generation_a_left, generation_a_right) =
                    tokio::join!(build_a_left, build_a_right);
                let generation_a_left = generation_a_left.unwrap();
                let generation_a_right = generation_a_right.unwrap();
                assert_ne!(generation_a_left.replayed, generation_a_right.replayed);
                assert_eq!(generation_a_left.record, generation_a_right.record);
                let generation_a = generation_a_left.record;
                assert_eq!(generation_a.impact.added_count, 2);
                assert_eq!(generation_a.impact.removed_count, 0);
                assert_eq!(generation_a.impact.unchanged_count, 0);
                assert_eq!(generation_a.projection.semantic_facts.len(), 2);
                assert_eq!(
                    generation_a
                        .impact
                        .added_facts
                        .iter()
                        .map(|fact| fact.relation.as_str())
                        .collect::<Vec<_>>(),
                    vec!["DISTRIBUTES", "SUPPLIES"]
                );
                let generation_a_replay = manager
                    .build_semantic_repair_generation(
                        state.artifact_cas.as_deref().unwrap(),
                        TENANT,
                        INCARNATION,
                        promoter.actor.clone(),
                        build_a_key,
                        build_a_request,
                    )
                    .await
                    .unwrap();
                assert!(generation_a_replay.replayed);
                assert_eq!(generation_a_replay.record, generation_a);

                for (collection, kind) in [
                    ("entities", CollectionType::Document),
                    ("chunks", CollectionType::Document),
                    ("mentions", CollectionType::Edge),
                    ("facts", CollectionType::Edge),
                ] {
                    raw.ensure_collection(collection, kind).await.unwrap();
                }
                let shared_entity = serde_json::to_value(
                    generation_a
                        .projection
                        .entities
                        .first()
                        .expect("M26 projection has a shared entity"),
                )
                .unwrap();
                let shared_entity_key = shared_entity["_key"].as_str().unwrap().to_string();
                let generation_only_entity = generation_a
                    .projection
                    .entities
                    .iter()
                    .find(|entity| entity.key != shared_entity_key)
                    .map(serde_json::to_value)
                    .transpose()
                    .unwrap()
                    .expect("M26 projection has an entity absent from live storage");
                let generation_only_entity_key = generation_only_entity["_key"]
                    .as_str()
                    .unwrap()
                    .to_string();
                assert!(
                    raw.get_document("entities", &generation_only_entity_key)
                        .await
                        .unwrap()
                        .is_none(),
                    "generation-only entity already existed before M26 activation"
                );
                raw.create_document("entities", shared_entity.clone())
                    .await
                    .unwrap();
                let stale_target_chunk_key = "m26-stale-target-chunk";
                let stale_target_mention_key = "m26-stale-target-mention";
                let stale_target_fact_key = "m26-stale-target-fact";
                raw.create_document(
                    "chunks",
                    json!({
                        "_key": stale_target_chunk_key,
                        "space_id": SPACE,
                        "chunk_id": "stale-target-chunk",
                        "title": "Stale target row",
                        "text": "This target row is outside the selected generation.",
                    }),
                )
                .await
                .unwrap();
                raw.create_edge(
                    "mentions",
                    json!({
                        "_key": stale_target_mention_key,
                        "_from": format!("chunks/{stale_target_chunk_key}"),
                        "_to": format!("entities/{shared_entity_key}"),
                        "relation_type": "MENTIONS",
                        "space_id": SPACE,
                        "evidence_chunk_id": "stale-target-chunk",
                    }),
                )
                .await
                .unwrap();
                raw.create_edge(
                    "facts",
                    json!({
                        "_key": stale_target_fact_key,
                        "_from": format!("entities/{shared_entity_key}"),
                        "_to": format!("entities/{shared_entity_key}"),
                        "relation_type": "STALE",
                        "space_id": SPACE,
                        "evidence_chunk_id": "stale-target-chunk",
                    }),
                )
                .await
                .unwrap();
                raw.create_document(
                    "entities",
                    json!({
                        "_key": "m26-other-entity",
                        "name": "Other Entity",
                        "entity_type": "other",
                        "aliases": [],
                    }),
                )
                .await
                .unwrap();
                raw.create_document(
                    "chunks",
                    json!({
                        "_key": "m26-other-chunk",
                        "space_id": "other-space",
                        "chunk_id": "other-chunk",
                        "title": "Other",
                        "text": "Other Entity exists.",
                    }),
                )
                .await
                .unwrap();
                for collection in ["mentions", "facts"] {
                    raw.create_edge(
                        collection,
                        json!({
                            "_key": format!("m26-other-{collection}"),
                            "_from": "entities/m26-other-entity",
                            "_to": "entities/m26-other-entity",
                            "relation_type": "EXISTS",
                            "space_id": "other-space",
                            "evidence_chunk_id": "other-chunk",
                        }),
                    )
                    .await
                    .unwrap();
                }
                let mut other_rows = Vec::new();
                for (collection, key) in [
                    ("entities", "m26-other-entity"),
                    ("chunks", "m26-other-chunk"),
                    ("mentions", "m26-other-mentions"),
                    ("facts", "m26-other-facts"),
                ] {
                    other_rows.push((
                        collection,
                        key,
                        raw.get_document(collection, key).await.unwrap().unwrap(),
                    ));
                }

                let deploy_a_key = "m26-deploy-generation-a";
                let premature_key = "m26-deploy-before-promoter-validity";
                let mut premature_authorization = signed_semantic_repair_deployment_intent(
                    &promoter,
                    &generation_a,
                    &promotion_head_a,
                    SemanticRepairDeploymentAction::Activate,
                    None,
                    None,
                    premature_key,
                    "must not admit a signature before key validity",
                );
                premature_authorization.statement.payload.signed_at_ms = promoter
                    .record
                    .not_before_ms
                    .saturating_sub(1);
                premature_authorization.promoter_signature = promoter
                    .signing_key
                    .sign(&premature_authorization.statement)
                    .unwrap();
                assert!(
                    matches!(
                        rejected_m26_deployment_preserves_state(
                            &manager,
                            raw.as_ref(),
                            promoter.actor.clone(),
                            premature_key,
                            &generation_a.semantic_repair_generation_id,
                            premature_authorization,
                        )
                        .await,
                        CogniGraphError::DocumentConflict(_)
                    ),
                    "a pre-validity deployment signature was admitted"
                );

                let authorization_a = signed_semantic_repair_deployment_intent(
                    &promoter,
                    &generation_a,
                    &promotion_head_a,
                    SemanticRepairDeploymentAction::Activate,
                    None,
                    None,
                    deploy_a_key,
                    "activate exact M26 generation A",
                );
                let serialized_authorization_a =
                    serde_json::to_value(&authorization_a).unwrap();
                assert!(
                    serialized_authorization_a
                        .pointer("/statement/payload/expected_deployment_head_decision_id")
                        .unwrap()
                        .is_null()
                );
                assert!(
                    serialized_authorization_a
                        .pointer("/statement/payload/rollback_target_generation_id")
                        .unwrap()
                        .is_null()
                );
                assert_eq!(
                    serde_json::from_value::<SemanticRepairDeploymentIntentSubmission>(
                        serialized_authorization_a.clone()
                    )
                    .unwrap(),
                    authorization_a
                );
                for required_null_field in [
                    "expected_deployment_head_decision_id",
                    "rollback_target_generation_id",
                ] {
                    let mut missing = serialized_authorization_a.clone();
                    missing
                        .pointer_mut("/statement/payload")
                        .and_then(Value::as_object_mut)
                        .unwrap()
                        .remove(required_null_field);
                    assert!(
                        serde_json::from_value::<SemanticRepairDeploymentIntentSubmission>(
                            missing
                        )
                        .is_err(),
                        "missing required-null field `{required_null_field}` was accepted"
                    );
                }

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
                            &revoked_promoter.record.registration_id,
                        ),
                        registration_id: revoked_promoter.record.registration_id.clone(),
                        key_id: revoked_promoter.record.verification_key.key_id.clone(),
                        public_key_digest: revoked_promoter.record.public_key_digest.clone(),
                        reason: "retire M26 deployment test promoter".into(),
                        signed_at_ms: revoked_at,
                        effective_at_ms: revoked_at,
                    },
                );
                manager
                    .revoke_governance_key(
                        TENANT,
                        INCARNATION,
                        GovernanceActor::from_user(governance_user(
                            Role::Admin,
                            "governance-admin",
                        )),
                        "m26-revoke-deployment-test-promoter",
                        RevokeGovernanceKeyRequest {
                            root_signature: root.sign(&revocation_statement).unwrap(),
                            statement: revocation_statement,
                        },
                    )
                    .await
                    .unwrap();

                let wrong_purpose_key = "m26-deploy-wrong-key-purpose";
                raw.create_document(
                    GOVERNANCE_KEYS_COLLECTION,
                    serde_json::to_value(&wrong_purpose_promoter.record).unwrap(),
                )
                .await
                .unwrap();
                let wrong_purpose_error = rejected_m26_deployment_preserves_state(
                    &manager,
                    raw.as_ref(),
                    wrong_purpose_promoter.actor.clone(),
                    wrong_purpose_key,
                    &generation_a.semantic_repair_generation_id,
                    signed_semantic_repair_deployment_intent(
                        &wrong_purpose_promoter,
                        &generation_a,
                        &promotion_head_a,
                        SemanticRepairDeploymentAction::Activate,
                        None,
                        None,
                        wrong_purpose_key,
                        "wrong key purpose must fail closed",
                    ),
                )
                .await;
                assert!(
                    matches!(
                        &wrong_purpose_error,
                        CogniGraphError::Forbidden(message)
                            if message == "governance key is inactive or has the wrong purpose"
                    ),
                    "unexpected wrong-purpose rejection: {wrong_purpose_error}"
                );
                assert!(
                    raw.delete_document(
                        GOVERNANCE_KEYS_COLLECTION,
                        &wrong_purpose_promoter.record.key,
                    )
                    .await
                    .unwrap()
                );

                let mut rejected_authorizations = Vec::new();

                let wrong_owner_key = "m26-deploy-wrong-actor-owner";
                rejected_authorizations.push((
                    wrong_owner_key,
                    alternate_promoter.actor.clone(),
                    signed_semantic_repair_deployment_intent(
                        &promoter,
                        &generation_a,
                        &promotion_head_a,
                        SemanticRepairDeploymentAction::Activate,
                        None,
                        None,
                        wrong_owner_key,
                        "actor ownership must be exact",
                    ),
                ));

                let invalid_signature_key = "m26-deploy-invalid-signature";
                let mut invalid_signature = signed_semantic_repair_deployment_intent(
                    &promoter,
                    &generation_a,
                    &promotion_head_a,
                    SemanticRepairDeploymentAction::Activate,
                    None,
                    None,
                    invalid_signature_key,
                    "invalid signature must fail closed",
                );
                invalid_signature.promoter_signature = alternate_promoter
                    .signing_key
                    .sign(&invalid_signature.statement)
                    .unwrap();
                rejected_authorizations.push((
                    invalid_signature_key,
                    promoter.actor.clone(),
                    invalid_signature,
                ));

                let target_binding_key = "m26-deploy-target-binding";
                let mut target_binding = signed_semantic_repair_deployment_intent(
                    &promoter,
                    &generation_a,
                    &promotion_head_a,
                    SemanticRepairDeploymentAction::Activate,
                    None,
                    None,
                    target_binding_key,
                    "target binding must be exact",
                );
                target_binding.statement.payload.target.channel = "canary".into();
                resign_semantic_repair_deployment_intent(&promoter, &mut target_binding);
                rejected_authorizations.push((
                    target_binding_key,
                    promoter.actor.clone(),
                    target_binding,
                ));

                let incarnation_binding_key = "m26-deploy-incarnation-binding";
                let mut incarnation_binding = signed_semantic_repair_deployment_intent(
                    &promoter,
                    &generation_a,
                    &promotion_head_a,
                    SemanticRepairDeploymentAction::Activate,
                    None,
                    None,
                    incarnation_binding_key,
                    "incarnation binding must be exact",
                );
                incarnation_binding.statement.tenant_incarnation = "other-incarnation".into();
                resign_semantic_repair_deployment_intent(&promoter, &mut incarnation_binding);
                rejected_authorizations.push((
                    incarnation_binding_key,
                    promoter.actor.clone(),
                    incarnation_binding,
                ));

                for (label, mutate) in [
                    (
                        "generation-id",
                        0_u8,
                    ),
                    ("generation-digest", 1),
                    ("impact-digest", 2),
                    ("candidate-digest", 3),
                    ("revision-id", 4),
                    ("revision-digest", 5),
                    ("review-id", 6),
                    ("review-digest", 7),
                    ("promotion-decision-id", 8),
                    ("promotion-projection-digest", 9),
                ] {
                    let key = match label {
                        "generation-id" => "m26-deploy-generation-id-binding",
                        "generation-digest" => "m26-deploy-generation-digest-binding",
                        "impact-digest" => "m26-deploy-impact-binding",
                        "candidate-digest" => "m26-deploy-candidate-binding",
                        "revision-id" => "m26-deploy-revision-id-binding",
                        "revision-digest" => "m26-deploy-revision-digest-binding",
                        "review-id" => "m26-deploy-review-id-binding",
                        "review-digest" => "m26-deploy-review-digest-binding",
                        "promotion-decision-id" => "m26-deploy-promotion-id-binding",
                        "promotion-projection-digest" => {
                            "m26-deploy-promotion-projection-binding"
                        }
                        _ => unreachable!(),
                    };
                    let mut authorization = signed_semantic_repair_deployment_intent(
                        &promoter,
                        &generation_a,
                        &promotion_head_a,
                        SemanticRepairDeploymentAction::Activate,
                        None,
                        None,
                        key,
                        &format!("{label} binding must be exact"),
                    );
                    match mutate {
                        0 => {
                            authorization
                                .statement
                                .payload
                                .semantic_repair_generation_id =
                                unrelated_record_id("m26-wrong-generation-id")
                        }
                        1 => {
                            authorization
                                .statement
                                .payload
                                .semantic_repair_generation_digest =
                                digest("m26-wrong-generation-digest")
                        }
                        2 => {
                            authorization.statement.payload.impact_digest =
                                digest("m26-wrong-impact-digest")
                        }
                        3 => {
                            authorization.statement.payload.candidate_digest =
                                digest("m26-wrong-candidate-digest")
                        }
                        4 => {
                            authorization
                                .statement
                                .payload
                                .semantic_repair_revision_id =
                                unrelated_record_id("m26-wrong-revision-id")
                        }
                        5 => {
                            authorization
                                .statement
                                .payload
                                .semantic_repair_revision_digest =
                                digest("m26-wrong-revision-digest")
                        }
                        6 => {
                            authorization.statement.payload.semantic_repair_review_id =
                                unrelated_record_id("m26-wrong-review-id")
                        }
                        7 => {
                            authorization.statement.payload.semantic_repair_review_digest =
                                digest("m26-wrong-review-digest")
                        }
                        8 => {
                            authorization.statement.payload.promotion_head_decision_id =
                                unrelated_record_id("m26-wrong-promotion-id")
                        }
                        9 => {
                            authorization
                                .statement
                                .payload
                                .promotion_head_projection_digest =
                                digest("m26-wrong-promotion-projection")
                        }
                        _ => unreachable!(),
                    }
                    resign_semantic_repair_deployment_intent(&promoter, &mut authorization);
                    rejected_authorizations.push((key, promoter.actor.clone(), authorization));
                }

                for (duty, cross_duty_promoter) in &cross_duty_promoters {
                    raw.create_document(
                        GOVERNANCE_KEYS_COLLECTION,
                        serde_json::to_value(&cross_duty_promoter.record).unwrap(),
                    )
                    .await
                    .unwrap();
                    let same_principal_key =
                        format!("m26-deploy-{duty}-principal-separation");
                    let same_principal_error = rejected_m26_deployment_preserves_state(
                        &manager,
                        raw.as_ref(),
                        cross_duty_promoter.actor.clone(),
                        &same_principal_key,
                        &generation_a.semantic_repair_generation_id,
                        signed_semantic_repair_deployment_intent(
                            cross_duty_promoter,
                            &generation_a,
                            &promotion_head_a,
                            SemanticRepairDeploymentAction::Activate,
                            None,
                            None,
                            &same_principal_key,
                            &format!("repair {duty} cannot deploy"),
                        ),
                    )
                    .await;
                    assert!(
                        matches!(
                            &same_principal_error,
                            CogniGraphError::DocumentConflict(message)
                                if message == "governance principal crosses duties"
                        ),
                        "unexpected {duty} separation rejection: {same_principal_error}"
                    );
                    assert!(
                        raw.delete_document(
                            GOVERNANCE_KEYS_COLLECTION,
                            &cross_duty_promoter.record.key,
                        )
                        .await
                        .unwrap()
                    );
                    manager.recover_tenant(TENANT, INCARNATION).await.unwrap();
                }

                let revoked_key = "m26-deploy-revoked-promoter";
                rejected_authorizations.push((
                    revoked_key,
                    revoked_promoter.actor.clone(),
                    signed_semantic_repair_deployment_intent(
                        &revoked_promoter,
                        &generation_a,
                        &promotion_head_a,
                        SemanticRepairDeploymentAction::Activate,
                        None,
                        None,
                        revoked_key,
                        "prospectively revoked promoter cannot deploy",
                    ),
                ));

                for (key, actor, authorization) in rejected_authorizations {
                    rejected_m26_deployment_preserves_state(
                        &manager,
                        raw.as_ref(),
                        actor,
                        key,
                        &generation_a.semantic_repair_generation_id,
                        authorization,
                    )
                    .await;
                }

                let mut conflicting_entity = shared_entity.clone();
                conflicting_entity["name"] = json!("Conflicting shared identity");
                raw.replace_document("entities", &shared_entity_key, conflicting_entity)
                    .await
                    .unwrap();
                let entity_conflict_key = "m26-deploy-conflicting-shared-entity";
                let entity_conflict = rejected_m26_deployment_preserves_state(
                    &manager,
                    raw.as_ref(),
                    promoter.actor.clone(),
                    entity_conflict_key,
                    &generation_a.semantic_repair_generation_id,
                    signed_semantic_repair_deployment_intent(
                        &promoter,
                        &generation_a,
                        &promotion_head_a,
                        SemanticRepairDeploymentAction::Activate,
                        None,
                        None,
                        entity_conflict_key,
                        "conflicting shared entity must fail closed",
                    ),
                )
                .await;
                assert!(
                    matches!(entity_conflict, CogniGraphError::DocumentConflict(_)),
                    "unexpected shared-entity conflict: {entity_conflict}"
                );
                raw.replace_document("entities", &shared_entity_key, shared_entity.clone())
                    .await
                    .unwrap();
                let shared_entity_before_activation = raw
                    .get_document("entities", &shared_entity_key)
                    .await
                    .unwrap()
                    .unwrap();

                let injected_failure_key = "m26-deploy-injected-batch-failure";
                fail_next_batch.store(true, Ordering::SeqCst);
                let injected_failure = rejected_m26_deployment_preserves_state(
                    &manager,
                    raw.as_ref(),
                    promoter.actor.clone(),
                    injected_failure_key,
                    &generation_a.semantic_repair_generation_id,
                    signed_semantic_repair_deployment_intent(
                        &promoter,
                        &generation_a,
                        &promotion_head_a,
                        SemanticRepairDeploymentAction::Activate,
                        None,
                        None,
                        injected_failure_key,
                        "injected batch failure must remain atomic",
                    ),
                )
                .await;
                assert!(
                    matches!(injected_failure, CogniGraphError::BackendError(_)),
                    "unexpected injected batch error: {injected_failure}"
                );
                assert!(
                    raw.get_document("entities", &generation_only_entity_key)
                        .await
                        .unwrap()
                        .is_none(),
                    "failed M26 activation partially inserted a generation-only entity"
                );

                let deployment_a = crate::routes::construct::directed_tests::deploy_after_directed(
                    &state,
                    SPACE,
                    manager.deploy_semantic_repair_generation(
                        TENANT,
                        INCARNATION,
                        promoter.actor.clone(),
                        deploy_a_key,
                        &generation_a.semantic_repair_generation_id,
                        authorization_a.clone(),
                    ),
                )
                    .await
                    .unwrap();
                assert!(!deployment_a.replayed);
                assert_eq!(deployment_a.head.selection.generation, 1);
                assert_eq!(
                    manager
                        .current_semantic_repair_deployment(TENANT, INCARNATION, SPACE)
                        .await
                        .unwrap(),
                    Some(deployment_a.head.clone())
                );
                let deployment_a_replay = manager
                    .deploy_semantic_repair_generation(
                        TENANT,
                        INCARNATION,
                        promoter.actor.clone(),
                        deploy_a_key,
                        &generation_a.semantic_repair_generation_id,
                        authorization_a,
                    )
                    .await
                    .unwrap();
                assert!(deployment_a_replay.replayed);
                assert_eq!(deployment_a_replay.decision, deployment_a.decision);
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_a.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_a.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_a.projection.facts,
                )
                .await;
                for (collection, stale_key) in [
                    ("chunks", stale_target_chunk_key),
                    ("mentions", stale_target_mention_key),
                    ("facts", stale_target_fact_key),
                ] {
                    assert!(
                        raw.get_document(collection, stale_key)
                            .await
                            .unwrap()
                            .is_none(),
                        "M26 activation retained stale `{collection}/{stale_key}`"
                    );
                }
                let stored_generation_only_entity = raw
                    .get_document("entities", &generation_only_entity_key)
                    .await
                    .unwrap()
                    .expect("M26 activation inserts a missing generation entity");
                assert!(
                    same_stored_record(
                        &stored_generation_only_entity,
                        &generation_only_entity,
                    )
                    .unwrap(),
                    "inserted generation entity differs from the frozen projection"
                );
                assert_eq!(
                    raw.get_document("entities", &shared_entity_key)
                        .await
                        .unwrap(),
                    Some(shared_entity_before_activation.clone()),
                    "compatible shared entity was rewritten during activation"
                );

                let stale_head_key = "m26-deploy-stale-head";
                let stale_head = rejected_m26_deployment_preserves_state(
                    &manager,
                    raw.as_ref(),
                    promoter.actor.clone(),
                    stale_head_key,
                    &generation_a.semantic_repair_generation_id,
                    signed_semantic_repair_deployment_intent(
                        &promoter,
                        &generation_a,
                        &promotion_head_a,
                        SemanticRepairDeploymentAction::Activate,
                        None,
                        None,
                        stale_head_key,
                        "stale deployment head must fail closed",
                    ),
                )
                .await;
                assert!(
                    matches!(stale_head, CogniGraphError::DocumentConflict(_)),
                    "unexpected stale-head error: {stale_head}"
                );

                let conflicting_idempotency = signed_semantic_repair_deployment_intent(
                    &promoter,
                    &generation_a,
                    &promotion_head_a,
                    SemanticRepairDeploymentAction::Activate,
                    None,
                    None,
                    deploy_a_key,
                    "same key cannot authorize different signed bytes",
                );
                let idempotency_conflict = rejected_m26_deployment_preserves_state(
                    &manager,
                    raw.as_ref(),
                    promoter.actor.clone(),
                    deploy_a_key,
                    &generation_a.semantic_repair_generation_id,
                    conflicting_idempotency,
                )
                .await;
                assert!(
                    matches!(
                        idempotency_conflict,
                        CogniGraphError::DocumentConflict(_)
                    ),
                    "unexpected idempotency conflict: {idempotency_conflict}"
                );

                let active_fact_before = raw
                    .get_document("facts", &generation_a.projection.facts[0].key)
                    .await
                    .unwrap()
                    .unwrap();
                let (fenced_space, fenced_neurons) = revision_a
                    .candidate
                    .validate_semantic_repair_candidate(&target)
                    .unwrap();
                let fenced_config = effective_config(&fenced_space, &fenced_neurons);
                let fenced_vetoes = effective_vetoes(&fenced_neurons);
                let fenced = manager
                    .ingest_unmaterialized_chunks(
                        raw.as_ref(),
                        (TENANT, INCARNATION),
                        SPACE,
                        &fenced_config,
                        &[Chunk {
                            id: "chunk-m26".into(),
                            title: String::new(),
                            text: "Acme no longer supplies Beta.".into(),
                        }],
                        &fenced_vetoes,
                    )
                    .await;
                assert!(
                    matches!(fenced, Err(CogniGraphError::DocumentConflict(_))),
                    "ordinary ingestion mutated an active M26 target: {fenced:?}"
                );
                assert_eq!(
                    raw.get_document("facts", &generation_a.projection.facts[0].key)
                        .await
                        .unwrap()
                        .as_ref(),
                    Some(&active_fact_before)
                );

                crate::routes::construct::directed_tests::assert_materialized_fence(&state, SPACE)
                    .await;

                let candidate_b_context = stored_reproducible_context_variant(
                    &state,
                    &cas_root.0,
                    &attestor,
                    "m26-candidate-b",
                    &binding,
                    Some(&shared_artifacts),
                    false,
                    false,
                    false,
                    ReproducibleContextVariant::SupplyAndManufacture,
                )
                .await;
                let candidate_b =
                    stored_construction_candidate(&state, &cas_root.0, &candidate_b_context)
                        .await;
                let revision_b = create_test_semantic_repair_revision(
                    &state,
                    &repair_author,
                    &target,
                    Some(&promotion_a.id),
                    candidate_b,
                    "m26-create-revision-b",
                )
                .await;
                approve_test_semantic_repair_revision(
                    &state,
                    &repair_approver,
                    &revision_b,
                    "m26-approve-revision-b",
                )
                .await;
                let candidate_b_original = submit_evaluation(
                    &state,
                    "m26-candidate-b-original",
                    candidate_b_context.clone(),
                )
                .await;
                let candidate_b_replay =
                    submit_evaluation(&state, "m26-candidate-b-replay", candidate_b_context)
                        .await;
                let evidence_b = manager
                    .register_evidence(
                        TENANT,
                        INCARNATION,
                        promoter_actor(&promoter),
                        "m26-evidence-b",
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
                let promote_b_key = "m26-promote-b";
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
                            "switch to exact M26 candidate B",
                        ),
                    )
                    .await
                    .unwrap()
                    .record;
                let promotion_head_b = manager
                    .current(TENANT, INCARNATION, &target)
                    .await
                    .unwrap()
                    .unwrap();
                let generation_b = manager
                    .build_semantic_repair_generation(
                        state.artifact_cas.as_deref().unwrap(),
                        TENANT,
                        INCARNATION,
                        promoter.actor.clone(),
                        "m26-build-generation-b",
                        BuildSemanticRepairGenerationRequest {
                            target: target.clone(),
                            expected_promotion_head_decision_id: promotion_b.id.clone(),
                        },
                    )
                    .await
                    .unwrap()
                    .record;
                assert_eq!(generation_b.impact.added_count, 1);
                assert_eq!(generation_b.impact.removed_count, 1);
                assert_eq!(generation_b.impact.unchanged_count, 1);
                assert_eq!(generation_b.impact.added_facts[0].relation, "MANUFACTURES");
                assert_eq!(generation_b.impact.removed_facts[0].relation, "DISTRIBUTES");
                assert_eq!(generation_b.projection.semantic_facts.len(), 2);

                let deploy_b_key = "m26-deploy-generation-b";
                let deployment_b = manager
                    .deploy_semantic_repair_generation(
                        TENANT,
                        INCARNATION,
                        promoter.actor.clone(),
                        deploy_b_key,
                        &generation_b.semantic_repair_generation_id,
                        signed_semantic_repair_deployment_intent(
                            &promoter,
                            &generation_b,
                            &promotion_head_b,
                            SemanticRepairDeploymentAction::Activate,
                            Some(&deployment_a.decision.deployment_decision_id),
                            None,
                            deploy_b_key,
                            "atomically switch to exact M26 generation B",
                        ),
                    )
                    .await
                    .unwrap();
                assert_eq!(deployment_b.head.selection.generation, 2);
                assert_eq!(
                    deployment_b
                        .head
                        .selection
                        .prior_semantic_repair_generation_id
                        .as_deref(),
                    Some(generation_a.semantic_repair_generation_id.as_str())
                );
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_b.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_b.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_b.projection.facts,
                )
                .await;
                assert!(
                    raw.get_document("entities", &generation_only_entity_key)
                        .await
                        .unwrap()
                        .is_some(),
                    "generation switch removed a still-authoritative entity"
                );
                for (collection, key, expected) in &other_rows {
                    assert_eq!(
                        raw.get_document(collection, key).await.unwrap().as_ref(),
                        Some(expected)
                    );
                }

                let rollback_key = "m26-promotion-rollback-b-to-a";
                let promotion_rollback = manager
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
                            "restore exact M26 candidate A authority",
                        ),
                    )
                    .await
                    .unwrap()
                    .record;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_b.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_b.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_b.projection.facts,
                )
                .await;
                let rollback_head = manager
                    .current(TENANT, INCARNATION, &target)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(rollback_head.applied_decision_id, promotion_rollback.id);

                let deploy_rollback_key = "m26-deploy-rollback-to-generation-a";
                let deployment_rollback = manager
                    .deploy_semantic_repair_generation(
                        TENANT,
                        INCARNATION,
                        promoter.actor.clone(),
                        deploy_rollback_key,
                        &generation_a.semantic_repair_generation_id,
                        signed_semantic_repair_deployment_intent(
                            &promoter,
                            &generation_a,
                            &rollback_head,
                            SemanticRepairDeploymentAction::Rollback,
                            Some(&deployment_b.decision.deployment_decision_id),
                            Some(&generation_a.semantic_repair_generation_id),
                            deploy_rollback_key,
                            "explicitly restore exact M26 generation A",
                        ),
                    )
                    .await
                    .unwrap();
                assert_eq!(deployment_rollback.head.selection.generation, 3);
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_a.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_a.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_a.projection.facts,
                )
                .await;
                let stored_generation_only_entity_after_rollback = raw
                    .get_document("entities", &generation_only_entity_key)
                    .await
                    .unwrap()
                    .expect("rollback preserves the selected generation entity");
                assert!(
                    same_stored_record(
                        &stored_generation_only_entity_after_rollback,
                        &generation_only_entity,
                    )
                    .unwrap(),
                    "rollback changed a selected generation entity"
                );
                assert_eq!(
                    raw.get_document("entities", &shared_entity_key)
                        .await
                        .unwrap(),
                    Some(shared_entity_before_activation.clone()),
                    "rollback deleted or rewrote a compatible shared entity"
                );
                for (collection, key, expected) in &other_rows {
                    assert_eq!(
                        raw.get_document(collection, key).await.unwrap().as_ref(),
                        Some(expected)
                    );
                }

                let active_fact_key = generation_a.projection.facts[0].key.clone();
                let mut forged_active_fact = raw
                    .get_document("facts", &active_fact_key)
                    .await
                    .unwrap()
                    .unwrap();
                forged_active_fact["forged"] = json!(true);
                raw.replace_document("facts", &active_fact_key, forged_active_fact)
                    .await
                    .unwrap();
                let forged_status = manager.operator_status(TENANT, INCARNATION).await.unwrap();
                assert_eq!(forged_status["healthy"], json!(false));
                assert_eq!(forged_status["repair_required"], json!(true));
                assert!(manager.recover_tenant(TENANT, INCARNATION).await.unwrap() >= 1);
                let mut repaired_active_fact = raw
                    .get_document("facts", &active_fact_key)
                    .await
                    .unwrap()
                    .unwrap();
                repaired_active_fact
                    .as_object_mut()
                    .unwrap()
                    .remove("confidence");
                assert!(same_stored_record(
                    &repaired_active_fact,
                    &serde_json::to_value(&generation_a.projection.facts[0]).unwrap(),
                )
                .unwrap());
                assert!(
                    raw.delete_document("facts", &active_fact_key)
                        .await
                        .unwrap()
                );
                let degraded = manager.operator_status(TENANT, INCARNATION).await.unwrap();
                assert_eq!(degraded["healthy"], json!(false));
                assert_eq!(degraded["repair_required"], json!(true));
                assert_eq!(
                    degraded["verified_semantic_repair"]["repair_required"],
                    json!(true)
                );
                assert!(manager.recover_tenant(TENANT, INCARNATION).await.unwrap() >= 1);
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_a.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_a.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_a.projection.facts,
                )
                .await;
                let recovered_status =
                    manager.operator_status(TENANT, INCARNATION).await.unwrap();
                assert_eq!(
                    recovered_status["healthy"],
                    json!(true),
                    "post-recovery status: {recovered_status:#}"
                );

                assert!(
                    raw.delete_document(
                        SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION,
                        &deployment_rollback.head.key,
                    )
                    .await
                    .unwrap()
                );
                let missing_head = manager.operator_status(TENANT, INCARNATION).await.unwrap();
                assert_eq!(missing_head["healthy"], json!(false));
                assert_eq!(missing_head["repair_required"], json!(true));
                assert!(manager.recover_tenant(TENANT, INCARNATION).await.unwrap() >= 1);
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_a.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_a.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_a.projection.facts,
                )
                .await;
                assert_eq!(
                    manager
                        .current_semantic_repair_deployment(TENANT, INCARNATION, SPACE)
                        .await
                        .unwrap(),
                    Some(deployment_rollback.head.clone())
                );
                for (collection, key, expected) in &other_rows {
                    assert_eq!(
                        raw.get_document(collection, key).await.unwrap().as_ref(),
                        Some(expected)
                    );
                }

                let snapshot = raw.export_snapshot().await.unwrap();
                if let Some(export_root) = std::env::var_os("COGNIGRAPH_M26_LIVE_FIXTURE_DIR") {
                    let export_root = PathBuf::from(export_root);
                    fs::create_dir_all(&export_root).unwrap();
                    copy_test_tree(&cas_root.0, &export_root.join("cas"));
                    fs::write(
                        export_root.join("snapshot.json"),
                        serde_json::to_vec_pretty(&snapshot).unwrap(),
                    )
                    .unwrap();
                    fs::write(
                        export_root.join("live.json"),
                        serde_json::to_vec_pretty(&json!({
                            "governance_root_public_key": root.public_key,
                            "promoter_username": "test-promoter-user@example.test",
                            "promoter_password": "m26-live-password",
                            "build_idempotency_key": build_a_key,
                            "build_request": {
                                "target": target,
                                "expected_promotion_head_decision_id": promotion_a.id,
                            },
                            "generation_id": generation_a.semantic_repair_generation_id,
                            "deploy_idempotency_key": deploy_a_key,
                            "deploy_authorization": serialized_authorization_a,
                            "space_type": SPACE,
                            "expected_active_deployment_decision_id": deployment_rollback.decision.deployment_decision_id,
                            "expected_active_fact_count": 2,
                        }))
                        .unwrap(),
                    )
                    .unwrap();
                }

                let mut wrong_existing_type = snapshot.clone();
                assert_eq!(
                    wrong_existing_type["collections"]["facts"]["type"],
                    json!("edge"),
                    "the incoming snapshot must independently declare the correct target type"
                );
                wrong_existing_type["collections"]
                    ["m26_wrong_existing_type_should_not_apply"] = json!({
                    "type": "document",
                    "documents": {"marker": {"_key": "marker", "value": 1}}
                });
                let wrong_type_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
                wrong_type_raw
                    .ensure_collection("facts", CollectionType::Document)
                    .await
                    .unwrap();
                let wrong_type_state = AppState::new_shared(wrong_type_raw.clone());
                wrong_type_state
                    .promotions
                    .configure_governance_root(Some(&root.public_key))
                    .unwrap();
                let wrong_type_error = wrong_type_state
                    .promotions
                    .import_snapshot(TENANT, INCARNATION, &wrong_existing_type)
                    .await
                    .unwrap_err();
                assert!(
                    matches!(wrong_type_error, CogniGraphError::DocumentConflict(_)),
                    "unexpected existing-target-type error: {wrong_type_error}"
                );
                assert_eq!(
                    wrong_type_raw
                        .get_document(
                            "m26_wrong_existing_type_should_not_apply",
                            "marker"
                        )
                        .await
                        .unwrap(),
                    None
                );
                wrong_type_state.jobs.shutdown().await;

                let mut mismatched_target_key = snapshot.clone();
                let active_fact_value = mismatched_target_key
                    .get_mut("collections")
                    .and_then(Value::as_object_mut)
                    .and_then(|collections| collections.get_mut("facts"))
                    .and_then(Value::as_object_mut)
                    .and_then(|collection| collection.get_mut("documents"))
                    .and_then(Value::as_object_mut)
                    .and_then(|documents| {
                        documents.remove(&generation_a.projection.facts[0].key)
                    })
                    .expect("exported active M26 fact");
                mismatched_target_key["collections"]["facts"]["documents"]
                    ["m26-wrong-map-key"] = active_fact_value;
                mismatched_target_key["collections"]
                    ["m26_target_key_mismatch_should_not_apply"] = json!({
                    "type": "document",
                    "documents": {"marker": {"_key": "marker", "value": 1}}
                });
                let mismatched_key_raw: Arc<dyn GraphBackend> =
                    Arc::new(NativeBackend::new());
                let mismatched_key_state = AppState::new_shared(mismatched_key_raw.clone());
                mismatched_key_state
                    .promotions
                    .configure_governance_root(Some(&root.public_key))
                    .unwrap();
                assert!(matches!(
                    mismatched_key_state
                        .promotions
                        .import_snapshot(TENANT, INCARNATION, &mismatched_target_key)
                        .await,
                    Err(CogniGraphError::DocumentConflict(_))
                ));
                assert_eq!(
                    mismatched_key_raw
                        .get_document("m26_target_key_mismatch_should_not_apply", "marker")
                        .await
                        .unwrap(),
                    None
                );
                mismatched_key_state.jobs.shutdown().await;

                let mut duplicate_idempotency = snapshot.clone();
                let mut duplicate_generation = generation_b.clone();
                duplicate_generation.idempotency_key_hash =
                    generation_a.idempotency_key_hash.clone();
                for _ in 0..8 {
                    duplicate_generation.semantic_repair_generation_digest = record_digest(
                        &duplicate_generation,
                        "semantic_repair_generation_digest",
                    )
                    .unwrap();
                    let bytes = canonical_json_bytes(&duplicate_generation).unwrap().len() as u64;
                    if bytes == duplicate_generation.canonical_record_bytes {
                        break;
                    }
                    duplicate_generation.canonical_record_bytes = bytes;
                }
                duplicate_idempotency["collections"]
                    [SEMANTIC_REPAIR_GENERATIONS_COLLECTION]["documents"]
                    [&duplicate_generation.key] =
                    serde_json::to_value(&duplicate_generation).unwrap();
                duplicate_idempotency["collections"]
                    ["m26_duplicate_idempotency_should_not_apply"] = json!({
                    "type": "document",
                    "documents": {"marker": {"_key": "marker", "value": 1}}
                });
                let duplicate_idempotency_raw: Arc<dyn GraphBackend> =
                    Arc::new(NativeBackend::new());
                let duplicate_idempotency_state =
                    AppState::new_shared(duplicate_idempotency_raw.clone());
                duplicate_idempotency_state
                    .promotions
                    .configure_governance_root(Some(&root.public_key))
                    .unwrap();
                assert!(matches!(
                    duplicate_idempotency_state
                        .promotions
                        .import_snapshot(TENANT, INCARNATION, &duplicate_idempotency)
                        .await,
                    Err(CogniGraphError::DocumentConflict(_))
                ));
                assert_eq!(
                    duplicate_idempotency_raw
                        .get_document("m26_duplicate_idempotency_should_not_apply", "marker")
                        .await
                        .unwrap(),
                    None
                );
                duplicate_idempotency_state.jobs.shutdown().await;

                let mut forged_generation_field = snapshot.clone();
                forged_generation_field["collections"]
                    [SEMANTIC_REPAIR_GENERATIONS_COLLECTION]["documents"]
                    [&generation_a.key]["confidence"] = json!(0.99);
                forged_generation_field["collections"]
                    ["m26_generation_extra_field_should_not_apply"] = json!({
                    "type": "document",
                    "documents": {"marker": {"_key": "marker", "value": 1}}
                });
                let forged_field_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
                let forged_field_state = AppState::new_shared(forged_field_raw.clone());
                forged_field_state
                    .promotions
                    .configure_governance_root(Some(&root.public_key))
                    .unwrap();
                assert!(forged_field_state
                    .promotions
                    .import_snapshot(TENANT, INCARNATION, &forged_generation_field)
                    .await
                    .is_err());
                assert_eq!(
                    forged_field_raw
                        .get_document("m26_generation_extra_field_should_not_apply", "marker")
                        .await
                        .unwrap(),
                    None
                );
                forged_field_state.jobs.shutdown().await;

                let mut tampered_generation = snapshot.clone();
                let generation_value = tampered_generation
                    .get_mut("collections")
                    .and_then(Value::as_object_mut)
                    .and_then(|collections| {
                        collections.get_mut(SEMANTIC_REPAIR_GENERATIONS_COLLECTION)
                    })
                    .and_then(Value::as_object_mut)
                    .and_then(|collection| collection.get_mut("documents"))
                    .and_then(Value::as_object_mut)
                    .and_then(|documents| documents.get_mut(&generation_a.key))
                    .expect("exported M26 generation");
                generation_value["impact"]["added_count"] = json!(999);
                tampered_generation["collections"]
                    ["m26_generation_tamper_should_not_apply"] = json!({
                    "type": "document",
                    "documents": {"marker": {"_key": "marker", "value": 1}}
                });
                let generation_tamper_raw: Arc<dyn GraphBackend> =
                    Arc::new(NativeBackend::new());
                let generation_tamper_state =
                    AppState::new_shared(generation_tamper_raw.clone());
                generation_tamper_state
                    .promotions
                    .configure_governance_root(Some(&root.public_key))
                    .unwrap();
                let generation_tamper_error = generation_tamper_state
                    .promotions
                    .import_snapshot(TENANT, INCARNATION, &tampered_generation)
                    .await
                    .unwrap_err();
                assert!(
                    matches!(
                        generation_tamper_error,
                        CogniGraphError::DocumentConflict(_)
                    ),
                    "unexpected M26 generation-tamper error: {generation_tamper_error}"
                );
                assert_eq!(
                    generation_tamper_raw
                        .get_document("m26_generation_tamper_should_not_apply", "marker")
                        .await
                        .unwrap(),
                    None
                );
                assert_eq!(
                    generation_tamper_raw
                        .get_document(SEMANTIC_REPAIR_GENERATIONS_COLLECTION, &generation_a.key)
                        .await
                        .unwrap(),
                    None
                );
                generation_tamper_state.jobs.shutdown().await;

                let mut tampered_decision = snapshot.clone();
                let decision_value = tampered_decision
                    .get_mut("collections")
                    .and_then(Value::as_object_mut)
                    .and_then(|collections| {
                        collections.get_mut(SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION)
                    })
                    .and_then(Value::as_object_mut)
                    .and_then(|collection| collection.get_mut("documents"))
                    .and_then(Value::as_object_mut)
                    .and_then(|documents| {
                        documents.get_mut(&deployment_rollback.decision.key)
                    })
                    .expect("exported M26 deployment decision");
                decision_value["reason"] = json!("tampered after signing");
                tampered_decision["collections"]
                    ["m26_decision_tamper_should_not_apply"] = json!({
                    "type": "document",
                    "documents": {"marker": {"_key": "marker", "value": 1}}
                });
                let decision_tamper_raw: Arc<dyn GraphBackend> =
                    Arc::new(NativeBackend::new());
                let decision_tamper_state = AppState::new_shared(decision_tamper_raw.clone());
                decision_tamper_state
                    .promotions
                    .configure_governance_root(Some(&root.public_key))
                    .unwrap();
                let decision_tamper_error = decision_tamper_state
                    .promotions
                    .import_snapshot(TENANT, INCARNATION, &tampered_decision)
                    .await
                    .unwrap_err();
                assert!(
                    matches!(
                        decision_tamper_error,
                        CogniGraphError::DocumentConflict(_)
                    ),
                    "unexpected M26 deployment-tamper error: {decision_tamper_error}"
                );
                assert_eq!(
                    decision_tamper_raw
                        .get_document("m26_decision_tamper_should_not_apply", "marker")
                        .await
                        .unwrap(),
                    None
                );
                assert_eq!(
                    decision_tamper_raw
                        .get_document(
                            SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
                            &deployment_rollback.decision.key,
                        )
                        .await
                        .unwrap(),
                    None
                );
                decision_tamper_state.jobs.shutdown().await;

                let mut untrusted_head_snapshot = snapshot;
                untrusted_head_snapshot
                    .get_mut("collections")
                    .and_then(Value::as_object_mut)
                    .and_then(|collections| {
                        collections.get_mut(SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION)
                    })
                    .and_then(Value::as_object_mut)
                    .and_then(|collection| collection.get_mut("documents"))
                    .and_then(Value::as_object_mut)
                    .expect("exported M26 deployment heads")
                    .insert(
                        deployment_rollback.head.key.clone(),
                        json!({
                            "_key": deployment_rollback.head.key,
                            "forged": true,
                        }),
                    );

                let restored_raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
                restored_raw
                    .ensure_collection("chunks", CollectionType::Document)
                    .await
                    .unwrap();
                restored_raw
                    .create_document(
                        "chunks",
                        json!({
                            "_key": "m26-additive-third-space",
                            "space_id": "third-space",
                            "chunk_id": "third-space-chunk",
                            "title": "Pre-existing additive row",
                            "text": "This row is outside the restored M26 target.",
                        }),
                    )
                    .await
                    .unwrap();
                let additive_row = restored_raw
                    .get_document("chunks", "m26-additive-third-space")
                    .await
                    .unwrap()
                    .unwrap();
                let restored_state = AppState::new_shared(restored_raw.clone());
                restored_state
                    .promotions
                    .configure_governance_root(Some(&root.public_key))
                    .unwrap();
                restored_state
                    .promotions
                    .import_snapshot(TENANT, INCARNATION, &untrusted_head_snapshot)
                    .await
                    .unwrap();
                assert_eq!(
                    restored_state
                        .promotions
                        .current_semantic_repair_deployment(TENANT, INCARNATION, SPACE)
                        .await
                        .unwrap(),
                    Some(deployment_rollback.head.clone())
                );
                assert_eq!(
                    restored_state
                        .promotions
                        .current_semantic_repair_deployment(
                            "other-tenant",
                            INCARNATION,
                            SPACE,
                        )
                        .await
                        .unwrap(),
                    None
                );
                assert_eq!(
                    restored_state
                        .promotions
                        .current_semantic_repair_deployment(
                            TENANT,
                            "other-incarnation",
                            SPACE,
                        )
                        .await
                        .unwrap(),
                    None
                );
                assert_eq!(
                    restored_raw
                        .get_document("chunks", "m26-additive-third-space")
                        .await
                        .unwrap(),
                    Some(additive_row.clone())
                );
                assert_materialized_space_rows(
                    restored_raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_a.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    restored_raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_a.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    restored_raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_a.projection.facts,
                )
                .await;
                let restored_generation_only_entity = restored_raw
                    .get_document("entities", &generation_only_entity_key)
                    .await
                    .unwrap()
                    .expect("snapshot restoration preserves generation entities");
                assert!(
                    same_stored_record(
                        &restored_generation_only_entity,
                        &generation_only_entity,
                    )
                    .unwrap(),
                    "restored generation entity differs from frozen authority"
                );

                assert!(
                    restored_raw
                        .delete_document("facts", &active_fact_key)
                        .await
                        .unwrap()
                );
                let restored_degraded = restored_state
                    .promotions
                    .operator_status(TENANT, INCARNATION)
                    .await
                    .unwrap();
                assert_eq!(restored_degraded["healthy"], json!(false));
                assert_eq!(
                    restored_degraded["verified_semantic_repair"]["repair_required"],
                    json!(true)
                );
                assert!(
                    restored_state
                        .promotions
                        .recover_tenant(TENANT, INCARNATION)
                        .await
                        .unwrap()
                        >= 1
                );
                assert_materialized_space_rows(
                    restored_raw.as_ref(),
                    "chunks",
                    SPACE,
                    &generation_a.projection.chunks,
                )
                .await;
                assert_materialized_space_rows(
                    restored_raw.as_ref(),
                    "mentions",
                    SPACE,
                    &generation_a.projection.mentions,
                )
                .await;
                assert_materialized_space_rows(
                    restored_raw.as_ref(),
                    "facts",
                    SPACE,
                    &generation_a.projection.facts,
                )
                .await;
                assert_eq!(
                    restored_state
                        .promotions
                        .current_semantic_repair_deployment(TENANT, INCARNATION, SPACE)
                        .await
                        .unwrap(),
                    Some(deployment_rollback.head)
                );
                assert_eq!(
                    restored_raw
                        .get_document("chunks", "m26-additive-third-space")
                        .await
                        .unwrap(),
                    Some(additive_row)
                );
                restored_state.jobs.shutdown().await;
                state.jobs.shutdown().await;
            };
            // This full-history debug fixture repeatedly canonicalizes six
            // nested M22/M25 authority jobs. Keep that test-only work off
            // libtest's 2 MiB stack while proving the future itself stays
            // bounded rather than hiding an accidentally multi-MiB state.
            assert!(
                std::mem::size_of_val(&lifecycle) < 512 * 1024,
                "M26 lifecycle future unexpectedly exceeded 512 KiB"
            );
            runtime.block_on(Box::pin(lifecycle));
        })
        .unwrap()
        .join()
        .unwrap();
}
