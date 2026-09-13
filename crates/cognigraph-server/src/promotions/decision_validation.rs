//! Decision validation.

use super::*;

impl PromotionManager {
    pub(super) fn validate_decision(
        &self,
        record: &PromotionDecision,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        if !matches!(
            record.schema_version,
            PROMOTION_DECISION_SCHEMA_VERSION
                | M19_PROMOTION_DECISION_SCHEMA_VERSION
                | M20_PROMOTION_DECISION_SCHEMA_VERSION
                | M21_PROMOTION_DECISION_SCHEMA_VERSION
                | M22_PROMOTION_DECISION_SCHEMA_VERSION
                | M23_PROMOTION_DECISION_SCHEMA_VERSION
        ) || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key != scoped_key(tenant, incarnation, "d", &record.id)
        {
            return Err(conflict("malformed or foreign promotion decision record"));
        }
        validate_record_id("decision id", &record.id)?;
        validate_record_id("decision evidence id", &record.evidence_id)?;
        validate_digest("idempotency_key_hash", &record.idempotency_key_hash)?;
        if record.id
            != new_record_id(
                tenant,
                incarnation,
                "decision",
                &record.idempotency_key_hash,
            )
        {
            return Err(conflict(
                "promotion decision id does not match its idempotency identity",
            ));
        }
        if record.created_at_ms == 0 {
            return Err(conflict("promotion decision timestamp is zero"));
        }
        if let Some(id) = &record.expected_head_decision_id {
            validate_record_id("expected_head_decision_id", id)?;
        }
        if let Some(id) = &record.predecessor_decision_id {
            validate_record_id("predecessor_decision_id", id)?;
        }
        validate_reason(&record.reason)?;
        match (record.schema_version, &record.governance) {
            (PROMOTION_DECISION_SCHEMA_VERSION, None) => record.actor.require_role("admin")?,
            (M19_PROMOTION_DECISION_SCHEMA_VERSION, Some(_)) => {
                record.actor.require_role("promoter")?
            }
            (M20_PROMOTION_DECISION_SCHEMA_VERSION, Some(_)) => {
                record.actor.require_role("promoter")?
            }
            (M21_PROMOTION_DECISION_SCHEMA_VERSION, Some(_)) => {
                record.actor.require_role("promoter")?
            }
            (M22_PROMOTION_DECISION_SCHEMA_VERSION, Some(_)) => {
                record.actor.require_role("promoter")?
            }
            (M23_PROMOTION_DECISION_SCHEMA_VERSION, Some(_)) => {
                record.actor.require_role("promoter")?
            }
            _ => {
                return Err(conflict(
                    "promotion decision schema and signed governance authority disagree",
                ));
            }
        }
        if let Some(authorization) = &record.governance {
            let payload = &authorization.statement.payload;
            let expected_generation = match record.schema_version {
                M19_PROMOTION_DECISION_SCHEMA_VERSION => {
                    payload.artifact_authority_digest.is_none()
                        && payload.consumption_authority_digest.is_none()
                        && payload.derivation_authority_digest.is_none()
                        && payload.preparation_authority_digest.is_none()
                        && authorization.statement.domain
                            == crate::governance::PROMOTION_INTENT_DOMAIN
                }
                M20_PROMOTION_DECISION_SCHEMA_VERSION => {
                    payload.artifact_authority_digest.is_some()
                        && payload.consumption_authority_digest.is_none()
                        && payload.derivation_authority_digest.is_none()
                        && payload.preparation_authority_digest.is_none()
                        && authorization.statement.domain
                            == crate::governance::PROMOTION_INTENT_DOMAIN
                }
                M21_PROMOTION_DECISION_SCHEMA_VERSION => {
                    payload.artifact_authority_digest.is_some()
                        && payload.consumption_authority_digest.is_some()
                        && payload.derivation_authority_digest.is_none()
                        && payload.preparation_authority_digest.is_none()
                        && authorization.statement.domain
                            == crate::governance::M21_PROMOTION_INTENT_DOMAIN
                }
                M22_PROMOTION_DECISION_SCHEMA_VERSION => {
                    payload.artifact_authority_digest.is_some()
                        && payload.consumption_authority_digest.is_some()
                        && payload.derivation_authority_digest.is_some()
                        && payload.preparation_authority_digest.is_none()
                        && authorization.statement.domain
                            == crate::governance::M22_PROMOTION_INTENT_DOMAIN
                }
                M23_PROMOTION_DECISION_SCHEMA_VERSION => {
                    payload.artifact_authority_digest.is_some()
                        && payload.consumption_authority_digest.is_some()
                        && payload.derivation_authority_digest.is_some()
                        && payload.preparation_authority_digest.is_some()
                        && authorization.statement.domain
                            == crate::governance::M23_PROMOTION_INTENT_DOMAIN
                }
                PROMOTION_DECISION_SCHEMA_VERSION => false,
                _ => unreachable!("decision schema was checked above"),
            };
            if !expected_generation {
                return Err(conflict(
                    "promotion decision schema, intent domain, and authority generation disagree",
                ));
            }
        }
        record.target.validate()?;
        for (label, value) in [
            ("decision_digest", &record.decision_digest),
            ("request_digest", &record.request_digest),
            ("evidence_digest", &record.evidence_digest),
            ("policy_digest", &record.policy_digest),
            ("gate_assessment_digest", &record.gate_assessment_digest),
        ] {
            validate_digest(label, value)?;
        }
        if record_digest(record, "decision_digest")? != record.decision_digest {
            return Err(conflict("promotion decision digest mismatch"));
        }
        let expected_request_digest = if let Some(authorization) = &record.governance {
            let signed = &authorization.statement.payload;
            let requested_action = if record.action == PromotionAction::Blocked {
                PromotionAction::Promote
            } else {
                record.action
            };
            let rollback_target_evidence_id = match requested_action {
                PromotionAction::Rollback => Some(record.evidence_id.clone()),
                PromotionAction::Promote => signed.rollback_target_evidence_id.clone(),
                PromotionAction::Reject => None,
                PromotionAction::Blocked => unreachable!("blocked is normalized above"),
            };
            let expected = crate::governance::PromotionIntentPayload {
                requested_action: match requested_action {
                    PromotionAction::Promote => "promote",
                    PromotionAction::Reject => "reject",
                    PromotionAction::Rollback => "rollback",
                    PromotionAction::Blocked => unreachable!("blocked is normalized above"),
                }
                .into(),
                target: record.target.clone(),
                evidence_id: record.evidence_id.clone(),
                evidence_digest: record.evidence_digest.clone(),
                policy_revision_id: signed.policy_revision_id.clone(),
                policy_revision_digest: signed.policy_revision_digest.clone(),
                approval_id: signed.approval_id.clone(),
                approval_digest: signed.approval_digest.clone(),
                artifact_authority_digest: signed.artifact_authority_digest.clone(),
                consumption_authority_digest: signed.consumption_authority_digest.clone(),
                derivation_authority_digest: signed.derivation_authority_digest.clone(),
                preparation_authority_digest: signed.preparation_authority_digest.clone(),
                gate_assessment_digest: record.gate_assessment_digest.clone(),
                expected_head_decision_id: record.expected_head_decision_id.clone(),
                rollback_target_evidence_id,
                reason: record.reason.clone(),
                idempotency_key_hash: record.idempotency_key_hash.clone(),
                promoter_registration_id: signed.promoter_registration_id.clone(),
                promoter_principal_id: signed.promoter_principal_id.clone(),
                signed_at_ms: signed.signed_at_ms,
            };
            self.validate_signed_promotion_intent(
                tenant,
                incarnation,
                &record.actor,
                &expected,
                authorization,
                None,
                record.created_at_ms,
            )?;
            canonical_digest(&json!({
                "actor": record.actor,
                "idempotency_key_hash": record.idempotency_key_hash,
                "authorization": crate::governance::PromotionIntentSubmission {
                    statement: authorization.statement.clone(),
                    promoter_signature: authorization.promoter_signature.clone(),
                },
            }))?
        } else if record.action == PromotionAction::Rollback {
            let expected_head_decision_id = record
                .expected_head_decision_id
                .clone()
                .ok_or_else(|| conflict("rollback decision is missing its expected head"))?;
            canonical_digest(&json!({
                "action": PromotionAction::Rollback,
                "target": record.target,
                "request": RollbackRequest {
                    expected_head_decision_id,
                    to_evidence_id: record.evidence_id.clone(),
                    reason: record.reason.clone(),
                },
            }))?
        } else {
            let requested_action = if record.action == PromotionAction::Blocked {
                PromotionAction::Promote
            } else {
                record.action
            };
            canonical_digest(&json!({
                "action": requested_action,
                "evidence_id": record.evidence_id,
                "reason": record.reason,
                "expected_head_decision_id": record.expected_head_decision_id,
                "rollback_to": Option::<String>::None,
            }))?
        };
        if record.request_digest != expected_request_digest {
            return Err(conflict("promotion decision request digest mismatch"));
        }
        let should_apply = matches!(
            record.action,
            PromotionAction::Promote | PromotionAction::Rollback
        );
        if should_apply != record.resulting_selection.is_some() {
            return Err(conflict(
                "promotion decision action and resulting selection disagree",
            ));
        }
        if let Some(selection) = &record.resulting_selection {
            validate_record_id("selection evidence id", &selection.evidence_id)?;
            if let Some(id) = &selection.prior_evidence_id {
                validate_record_id("prior evidence id", id)?;
            }
            for (label, digest) in [
                ("selection evidence digest", &selection.evidence_digest),
                ("selection candidate digest", &selection.candidate_digest),
                ("selection policy digest", &selection.policy_digest),
            ] {
                validate_digest(label, digest)?;
            }
            if selection.generation == 0 {
                return Err(conflict("promotion selection generation is zero"));
            }
        }
        Ok(())
    }
}
