//! Signed request builders for the idempotency namespace test (CG-112).

use super::*;
use crate::governance::PolicyRevisionRecord;

pub(super) fn governance_admin() -> GovernanceActor {
    GovernanceActor::from_user(governance_user(Role::Admin, "governance-admin"))
}

pub(super) fn registration_request(
    root: &SigningKeyMaterial,
    principal_id: &str,
    user_key: &str,
) -> RegisterGovernanceKeyRequest {
    let signing_key = SigningKeyMaterial::generate(KeyPurpose::Promoter).unwrap();
    let verification_key = signing_key.verification_key().unwrap();
    let now = now_millis();
    let statement = GovernanceStatement::new(
        KEY_REGISTRATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRegistrationPayload {
            registration_id: key_registration_id(TENANT, INCARNATION, &verification_key.key_id),
            principal_id: principal_id.into(),
            subject_user_key: user_key.into(),
            verification_key,
            not_before_ms: now.saturating_sub(1),
            not_after_ms: None,
            signed_at_ms: now,
        },
    );
    RegisterGovernanceKeyRequest {
        root_signature: root.sign(&statement).unwrap(),
        possession_signature: signing_key.sign(&statement).unwrap(),
        statement,
    }
}

pub(super) fn revocation_request(
    root: &SigningKeyMaterial,
    key: &GovernanceKeyRecord,
) -> RevokeGovernanceKeyRequest {
    let now = now_millis();
    let statement = GovernanceStatement::new(
        KEY_REVOCATION_DOMAIN,
        TENANT,
        INCARNATION,
        KeyRevocationPayload {
            revocation_id: key_revocation_id(TENANT, INCARNATION, &key.registration_id),
            registration_id: key.registration_id.clone(),
            key_id: key.verification_key.key_id.clone(),
            public_key_digest: key.public_key_digest.clone(),
            reason: "CG-112 idempotency namespace test".into(),
            signed_at_ms: now,
            effective_at_ms: now,
        },
    );
    RevokeGovernanceKeyRequest {
        root_signature: root.sign(&statement).unwrap(),
        statement,
    }
}

pub(super) fn policy_request(
    author: &TestGovernancePrincipal,
    target: &PromotionTarget,
) -> CreatePolicyRevisionRequest {
    let resolved_policy = policy();
    let statement = GovernanceStatement::new(
        POLICY_REVISION_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyRevisionPayload {
            policy_revision_id: policy_revision_id(TENANT, INCARNATION, target, &resolved_policy)
                .unwrap(),
            target: target.clone(),
            resolved_policy_digest: canonical_digest(&resolved_policy).unwrap(),
            resolved_policy,
            author_registration_id: author.record.registration_id.clone(),
            author_principal_id: author.record.principal_id.clone(),
            signed_at_ms: now_millis(),
        },
    );
    CreatePolicyRevisionRequest {
        author_signature: author.signing_key.sign(&statement).unwrap(),
        statement,
    }
}

pub(super) fn approval_request(
    approver: &TestGovernancePrincipal,
    policy: &PolicyRevisionRecord,
) -> ApprovePolicyRevisionRequest {
    let statement = GovernanceStatement::new(
        POLICY_APPROVAL_DOMAIN,
        TENANT,
        INCARNATION,
        PolicyApprovalPayload {
            approval_id: approval_id(TENANT, INCARNATION, &policy.policy_revision_id),
            policy_revision_id: policy.policy_revision_id.clone(),
            policy_revision_digest: policy.policy_revision_digest.clone(),
            target: policy.target.clone(),
            resolved_policy_digest: policy.resolved_policy_digest.clone(),
            author_principal_id: policy.author_principal_id.clone(),
            approver_registration_id: approver.record.registration_id.clone(),
            approver_principal_id: approver.record.principal_id.clone(),
            decision: "approve".into(),
            reason: "CG-112 idempotency namespace test".into(),
            signed_at_ms: now_millis().max(policy.created_at_ms),
        },
    );
    ApprovePolicyRevisionRequest {
        approval_signature: approver.signing_key.sign(&statement).unwrap(),
        statement,
    }
}

pub(super) fn review_request(
    approver: &TestGovernancePrincipal,
    revision: &SemanticRepairRevisionRecord,
) -> ReviewSemanticRepairRevisionRequest {
    let statement = GovernanceStatement::new(
        SEMANTIC_REPAIR_REVIEW_DOMAIN,
        TENANT,
        INCARNATION,
        SemanticRepairReviewPayload {
            semantic_repair_review_id: semantic_repair_review_id(
                TENANT,
                INCARNATION,
                &revision.semantic_repair_revision_id,
            ),
            semantic_repair_revision_id: revision.semantic_repair_revision_id.clone(),
            semantic_repair_revision_digest: revision.semantic_repair_revision_digest.clone(),
            target: revision.target.clone(),
            base_promotion_head_decision_id: revision.base_promotion_head_decision_id.clone(),
            candidate_digest: revision.candidate_digest.clone(),
            author_principal_id: revision.author_principal_id.clone(),
            approver_registration_id: approver.record.registration_id.clone(),
            approver_principal_id: approver.record.principal_id.clone(),
            decision: SemanticRepairReviewDecision::Approve,
            reason: "CG-112 idempotency namespace test".into(),
            signed_at_ms: now_millis().max(revision.created_at_ms),
        },
    );
    ReviewSemanticRepairRevisionRequest {
        approver_signature: approver.signing_key.sign(&statement).unwrap(),
        statement,
    }
}

pub(super) fn verifier_subject() -> ArtifactSubject {
    ArtifactSubject::Verifier {
        verifier_name: "cg112-verifier".into(),
        verifier_version: "1".into(),
        semantics_identity_digest: digest("cg112-verifier"),
    }
}
