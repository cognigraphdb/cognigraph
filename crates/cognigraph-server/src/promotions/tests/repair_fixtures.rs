//! Repair fixtures.

use super::*;

pub(super) fn semantic_repair_candidate(
    candidate: &str,
    target: &PromotionTarget,
) -> ConstructionCandidateArtifact {
    ConstructionCandidateArtifact {
        schema_version: 1,
        kind: "semantic-neuron-bundle".into(),
        id: candidate.into(),
        revision: "1".into(),
        base_space_type: ConstructionSpaceType {
            id: target.space_type.clone(),
            name: "Pharma".into(),
            version: 1,
            description: "M25 governed semantic repair authority test".into(),
            entities: vec![
                ConstructionEntity {
                    name: "Compound X".into(),
                    entity_type: "compound".into(),
                    aliases: Vec::new(),
                },
                ConstructionEntity {
                    name: "Meridian".into(),
                    entity_type: "organization".into(),
                    aliases: Vec::new(),
                },
            ],
            relation_rules: vec![ConstructionRelationRule {
                source: "Meridian".into(),
                relation: "SUPPLIES".into(),
                target: "Compound X".into(),
                when_any: vec!["supplies Compound X".into()],
                require_in_sentence: Vec::new(),
            }],
        },
        accepted_neurons: Vec::new(),
    }
}
pub(super) fn governed_semantic_context(
    candidate: &str,
    binding: &PolicyGovernanceBinding,
) -> (PromotionContext, ConstructionCandidateArtifact) {
    let mut context = governed_context(candidate, binding);
    let artifact = semantic_repair_candidate(candidate, &context.target);
    let candidate_bytes =
        cognigraph_governance::canonical_json_bytes(&artifact).expect("canonical candidate");
    let candidate_digest = digest_bytes(&candidate_bytes);
    context.candidate.kind = artifact.kind.clone();
    context.candidate.id = artifact.id.clone();
    context.candidate.revision = artifact.revision.clone();
    context.candidate.artifact = ArtifactIdentity {
        uri: format!("urn:cognigraph:test:m25:candidate:{candidate}"),
        media_type: "application/json".into(),
        bytes: candidate_bytes.len() as u64,
        digest: candidate_digest.clone(),
    };
    context.candidate.candidate_digest = candidate_digest.clone();
    context.revisions.graph.candidate_digest = Some(candidate_digest);
    (context, artifact)
}
pub(super) async fn create_test_semantic_repair_revision(
    state: &AppState,
    author: &TestGovernancePrincipal,
    target: &PromotionTarget,
    base_promotion_head_decision_id: Option<&str>,
    candidate: ConstructionCandidateArtifact,
    idempotency_key: &str,
) -> SemanticRepairRevisionRecord {
    let candidate_digest = canonical_digest(&candidate).unwrap();
    let statement = GovernanceStatement::new(
        SEMANTIC_REPAIR_REVISION_DOMAIN,
        TENANT,
        INCARNATION,
        SemanticRepairRevisionPayload {
            semantic_repair_revision_id: semantic_repair_revision_id(
                TENANT,
                INCARNATION,
                target,
                base_promotion_head_decision_id,
                &candidate_digest,
            )
            .unwrap(),
            target: target.clone(),
            base_promotion_head_decision_id: base_promotion_head_decision_id.map(str::to_owned),
            candidate,
            candidate_digest,
            author_registration_id: author.record.registration_id.clone(),
            author_principal_id: author.record.principal_id.clone(),
            signed_at_ms: now_millis(),
        },
    );
    state
        .promotions
        .create_semantic_repair_revision(
            TENANT,
            INCARNATION,
            author.actor.clone(),
            idempotency_key,
            CreateSemanticRepairRevisionRequest {
                author_signature: author.signing_key.sign(&statement).unwrap(),
                statement,
            },
        )
        .await
        .unwrap()
        .record
}
pub(super) async fn approve_test_semantic_repair_revision(
    state: &AppState,
    approver: &TestGovernancePrincipal,
    revision: &SemanticRepairRevisionRecord,
    idempotency_key: &str,
) -> SemanticRepairReviewRecord {
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
            reason: "independent M25 test approval".into(),
            signed_at_ms: now_millis().max(revision.created_at_ms),
        },
    );
    state
        .promotions
        .review_semantic_repair_revision(
            TENANT,
            INCARNATION,
            approver.actor.clone(),
            idempotency_key,
            ReviewSemanticRepairRevisionRequest {
                approver_signature: approver.signing_key.sign(&statement).unwrap(),
                statement,
            },
        )
        .await
        .unwrap()
        .record
}
