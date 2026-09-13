//! Policy fixtures.

use super::*;

pub(super) async fn governed_policy_binding(
    state: &AppState,
) -> (
    PolicyGovernanceBinding,
    TestGovernancePrincipal,
    SigningKeyMaterial,
) {
    let root = SigningKeyMaterial::generate(KeyPurpose::TrustRoot).unwrap();
    state
        .promotions
        .configure_governance_root(Some(&root.public_key))
        .unwrap();
    let author = register_test_governance_principal(
        state,
        &root,
        KeyPurpose::PolicyAuthor,
        Role::PolicyAuthor,
        "test-author-principal",
        "test-author-user",
    )
    .await;
    let approver = register_test_governance_principal(
        state,
        &root,
        KeyPurpose::PolicyApprover,
        Role::PolicyApprover,
        "test-approver-principal",
        "test-approver-user",
    )
    .await;
    let promoter = register_test_governance_principal(
        state,
        &root,
        KeyPurpose::Promoter,
        Role::Promoter,
        "test-promoter-principal",
        "test-promoter-user",
    )
    .await;

    let resolved_policy = policy();
    let target = PromotionTarget {
        space_type: SPACE.into(),
        channel: "stable".into(),
    };
    let policy_statement = GovernanceStatement::new(
        POLICY_REVISION_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyRevisionPayload {
            policy_revision_id: policy_revision_id(TENANT, INCARNATION, &target, &resolved_policy)
                .unwrap(),
            target: target.clone(),
            resolved_policy_digest: canonical_digest(&resolved_policy).unwrap(),
            resolved_policy,
            author_registration_id: author.record.registration_id.clone(),
            author_principal_id: author.record.principal_id.clone(),
            signed_at_ms: now_millis(),
        },
    );
    let policy_record = state
        .promotions
        .create_policy_revision(
            TENANT,
            INCARNATION,
            author.actor,
            "create-test-governed-policy",
            CreatePolicyRevisionRequest {
                author_signature: author.signing_key.sign(&policy_statement).unwrap(),
                statement: policy_statement,
            },
        )
        .await
        .unwrap()
        .record;
    let approval_statement = GovernanceStatement::new(
        POLICY_APPROVAL_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyApprovalPayload {
            approval_id: approval_id(TENANT, INCARNATION, &policy_record.policy_revision_id),
            policy_revision_id: policy_record.policy_revision_id.clone(),
            policy_revision_digest: policy_record.policy_revision_digest.clone(),
            target,
            resolved_policy_digest: policy_record.resolved_policy_digest.clone(),
            author_principal_id: policy_record.author_principal_id.clone(),
            approver_registration_id: approver.record.registration_id.clone(),
            approver_principal_id: approver.record.principal_id.clone(),
            decision: "approve".into(),
            reason: "independent test approval".into(),
            signed_at_ms: now_millis(),
        },
    );
    let approval = state
        .promotions
        .approve_policy_revision(
            TENANT,
            INCARNATION,
            approver.actor,
            "approve-test-governed-policy",
            ApprovePolicyRevisionRequest {
                approval_signature: approver.signing_key.sign(&approval_statement).unwrap(),
                statement: approval_statement,
            },
        )
        .await
        .unwrap()
        .record;
    let binding = state
        .promotions
        .policy_binding(TENANT, INCARNATION, &approval.approval_id)
        .await
        .unwrap();
    (binding, promoter, root)
}
