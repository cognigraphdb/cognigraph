//! CG-112: every signed record of a tenant shares one Idempotency-Key
//! namespace, at admission as well as when stored authority is validated.

use super::*;
use crate::artifact_attestations::ARTIFACT_ATTESTATIONS_COLLECTION;
use crate::governance::GOVERNANCE_KEY_REVOCATIONS_COLLECTION;

const REVISION_KEY: &str = "cg112-repair-revision";
const REVIEW_KEY: &str = "cg112-repair-review";

#[test]
fn a_key_spent_on_semantic_repair_authority_is_refused_for_every_governance_record() {
    lifecycle(flow);
}

fn refused<T: std::fmt::Debug>(result: Result<T, CogniGraphError>, context: &str) {
    match result {
        Err(CogniGraphError::DocumentConflict(message)) => assert!(
            message.contains("Idempotency-Key was already used"),
            "{context}: {message}"
        ),
        other => panic!("{context}: expected a conflict, got {other:?}"),
    }
}

async fn healthy(state: &AppState, target: &PromotionTarget, context: &str) {
    state
        .promotions
        .reconcile(TENANT, INCARNATION, target, false)
        .await
        .unwrap_or_else(|error| panic!("{context}: reconcile failed: {error}"));
    state
        .promotions
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap_or_else(|error| panic!("{context}: recovery failed: {error}"));
}

async fn flow() {
    let raw: Arc<dyn GraphBackend> = Arc::new(NativeBackend::new());
    let state = AppState::new_shared(raw.clone());
    let manager = state.promotions.clone();
    let (binding, promoter, root) = governed_policy_binding(&state).await;
    let author = register_test_governance_principal(
        &state,
        &root,
        KeyPurpose::PolicyAuthor,
        Role::PolicyAuthor,
        "cg112-author-principal",
        "cg112-author-user",
    )
    .await;
    let approver = register_test_governance_principal(
        &state,
        &root,
        KeyPurpose::PolicyApprover,
        Role::PolicyApprover,
        "cg112-approver-principal",
        "cg112-approver-user",
    )
    .await;
    let attestor = register_test_governance_principal(
        &state,
        &root,
        KeyPurpose::ArtifactAttestor,
        Role::ArtifactAttestor,
        "cg112-attestor-principal",
        "cg112-attestor-user",
    )
    .await;
    let target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "stable".into(),
    };
    let canary = PromotionTarget {
        space_type: SPACE.into(),
        channel: "canary".into(),
    };

    // The two semantic-repair record kinds spend one key each.
    let (_, candidate) = governed_semantic_context("cg112-candidate", &binding);
    let revision = create_test_semantic_repair_revision(
        &state,
        &author,
        &target,
        None,
        candidate,
        REVISION_KEY,
    )
    .await;
    let review_request = review_request(&approver, &revision);
    let review = manager
        .review_semantic_repair_revision(
            TENANT,
            INCARNATION,
            approver.actor.clone(),
            REVIEW_KEY,
            review_request.clone(),
        )
        .await
        .unwrap();
    assert!(!review.replayed);
    // A policy revision that is still waiting for approval.
    let canary_policy = manager
        .create_policy_revision(
            TENANT,
            INCARNATION,
            author.actor.clone(),
            "cg112-canary-policy",
            policy_request(&author, &canary),
        )
        .await
        .unwrap()
        .record;

    // Each of the five governance record kinds refuses a spent key, whether
    // a revision or a review spent it.
    refused(
        manager
            .create_artifact_attestation(
                TENANT,
                INCARNATION,
                attestor.actor.clone(),
                REVISION_KEY,
                signed_artifact_attestation_request(
                    &attestor,
                    ArtifactKind::Verifier,
                    "cg112-verifier",
                    verifier_subject(),
                ),
            )
            .await,
        "artifact attestation",
    );
    refused(
        manager
            .register_governance_key(
                TENANT,
                INCARNATION,
                governance_admin(),
                governance_user(Role::Promoter, "cg112-spare-user"),
                REVIEW_KEY,
                registration_request(&root, "cg112-spare-principal", "cg112-spare-user"),
            )
            .await,
        "key registration",
    );
    refused(
        manager
            .revoke_governance_key(
                TENANT,
                INCARNATION,
                governance_admin(),
                REVISION_KEY,
                revocation_request(&root, &promoter.record),
            )
            .await,
        "key revocation",
    );
    refused(
        manager
            .create_policy_revision(
                TENANT,
                INCARNATION,
                author.actor.clone(),
                REVIEW_KEY,
                policy_request(
                    &author,
                    &PromotionTarget {
                        space_type: SPACE.into(),
                        channel: "beta".into(),
                    },
                ),
            )
            .await,
        "policy revision",
    );
    refused(
        manager
            .approve_policy_revision(
                TENANT,
                INCARNATION,
                approver.actor.clone(),
                REVISION_KEY,
                approval_request(&approver, &canary_policy),
            )
            .await,
        "policy approval",
    );

    // Nothing was stored, so the tenant's authority still validates, and a
    // snapshot of it restores into an empty store.
    for collection in [
        ARTIFACT_ATTESTATIONS_COLLECTION,
        GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
    ] {
        let stored = match raw.list_documents(collection, None, None).await {
            Ok(rows) => rows.len(),
            Err(_) => 0,
        };
        assert_eq!(stored, 0, "{collection}");
    }
    healthy(&state, &target, "after the refusals").await;
    let snapshot = raw.export_snapshot().await.unwrap();
    let restored = AppState::new_shared(Arc::new(NativeBackend::new()));
    restored
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();
    restored
        .promotions
        .import_snapshot(TENANT, INCARNATION, &snapshot)
        .await
        .unwrap();

    // An exact replay of the request that spent a key is still a replay.
    let replay = manager
        .review_semantic_repair_revision(
            TENANT,
            INCARNATION,
            approver.actor.clone(),
            REVIEW_KEY,
            review_request,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.record, review.record);

    // The same requests go through with keys of their own.
    let attestation = manager
        .create_artifact_attestation(
            TENANT,
            INCARNATION,
            attestor.actor.clone(),
            "cg112-attestation",
            signed_artifact_attestation_request(
                &attestor,
                ArtifactKind::Verifier,
                "cg112-verifier",
                verifier_subject(),
            ),
        )
        .await
        .unwrap()
        .record;
    manager
        .approve_policy_revision(
            TENANT,
            INCARNATION,
            approver.actor.clone(),
            "cg112-canary-approval",
            approval_request(&approver, &canary_policy),
        )
        .await
        .unwrap();
    healthy(&state, &target, "after the accepted records").await;

    // The reverse order was always refused and still is.
    let (_, second_candidate) = governed_semantic_context("cg112-candidate-b", &binding);
    let second = SemanticRepairRevisionPayload {
        semantic_repair_revision_id: semantic_repair_revision_id(
            TENANT,
            INCARNATION,
            &canary,
            None,
            &canonical_digest(&second_candidate).unwrap(),
        )
        .unwrap(),
        target: canary.clone(),
        base_promotion_head_decision_id: None,
        candidate_digest: canonical_digest(&second_candidate).unwrap(),
        candidate: second_candidate,
        author_registration_id: author.record.registration_id.clone(),
        author_principal_id: author.record.principal_id.clone(),
        signed_at_ms: now_millis(),
    };
    let statement =
        GovernanceStatement::new(SEMANTIC_REPAIR_REVISION_DOMAIN, TENANT, INCARNATION, second);
    refused(
        manager
            .create_semantic_repair_revision(
                TENANT,
                INCARNATION,
                author.actor.clone(),
                "cg112-attestation",
                CreateSemanticRepairRevisionRequest {
                    author_signature: author.signing_key.sign(&statement).unwrap(),
                    statement,
                },
            )
            .await,
        "semantic repair revision",
    );

    // Stored authority that already holds a cross-kind reuse, as a store
    // written before this check would, is still refused by validation.
    let mut poisoned = attestation.clone();
    poisoned.idempotency_key_hash = revision.idempotency_key_hash.clone();
    poisoned.attestation_digest = record_digest(&poisoned, "attestation_digest").unwrap();
    raw.replace_document(
        ARTIFACT_ATTESTATIONS_COLLECTION,
        &poisoned.key,
        serde_json::to_value(&poisoned).unwrap(),
    )
    .await
    .unwrap();
    let error = manager
        .recover_tenant(TENANT, INCARNATION)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("governance records reuse an idempotency-key hash"),
        "{error}"
    );
    state.jobs.shutdown().await;
}
