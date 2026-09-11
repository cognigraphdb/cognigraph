//! Intent validation.

use super::*;

pub(super) fn validate_deployment_intent_payload(
    payload: &SemanticRepairDeploymentIntentPayload,
    _tenant: &str,
    _incarnation: &str,
    now: u64,
) -> Result<(), CogniGraphError> {
    payload.target.validate()?;
    for (label, id) in [
        (
            "semantic_repair_generation_id",
            payload.semantic_repair_generation_id.as_str(),
        ),
        (
            "promotion_head_decision_id",
            payload.promotion_head_decision_id.as_str(),
        ),
        (
            "semantic_repair_revision_id",
            payload.semantic_repair_revision_id.as_str(),
        ),
        (
            "semantic_repair_review_id",
            payload.semantic_repair_review_id.as_str(),
        ),
    ] {
        validate_record_id(label, id)?;
    }
    for id in [
        payload.expected_deployment_head_decision_id.as_deref(),
        payload.rollback_target_generation_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_record_id("M26 nullable deployment id", id)?;
    }
    for (label, digest) in [
        (
            "semantic_repair_generation_digest",
            payload.semantic_repair_generation_digest.as_str(),
        ),
        ("impact_digest", payload.impact_digest.as_str()),
        (
            "promotion_head_projection_digest",
            payload.promotion_head_projection_digest.as_str(),
        ),
        ("candidate_digest", payload.candidate_digest.as_str()),
        (
            "semantic_repair_revision_digest",
            payload.semantic_repair_revision_digest.as_str(),
        ),
        (
            "semantic_repair_review_digest",
            payload.semantic_repair_review_digest.as_str(),
        ),
        (
            "idempotency_key_hash",
            payload.idempotency_key_hash.as_str(),
        ),
    ] {
        validate_digest(label, digest)?;
    }
    validate_reason(&payload.reason)?;
    validate_identifier(
        "promoter_registration_id",
        &payload.promoter_registration_id,
    )?;
    validate_identifier("promoter_principal_id", &payload.promoter_principal_id)?;
    if payload.signed_at_ms == 0 || payload.signed_at_ms > now.saturating_add(MAX_CLOCK_SKEW_MS) {
        return Err(validation(
            "M26 deployment signed_at_ms is zero or more than five minutes in the future",
        ));
    }
    Ok(())
}
pub(super) fn require_signing_actor(
    key: &GovernanceKeyRecord,
    actor: &GovernanceActor,
    principal_id: &str,
) -> Result<(), CogniGraphError> {
    if key.subject_user_key != actor.user_key
        || key.principal_id != principal_id
        || key.principal_id.trim().is_empty()
    {
        return Err(CogniGraphError::Forbidden(
            "authenticated user does not own the M26 deployment signing principal".into(),
        ));
    }
    Ok(())
}
