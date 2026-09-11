//! Decision transition.

use super::*;

impl PromotionManager {
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn decide(
        &self,
        tenant: &str,
        incarnation: &str,
        evidence_id: &str,
        actor: PromotionActor,
        idempotency_key: &str,
        requested_action: PromotionAction,
        reason: String,
        expected_head_decision_id: Option<String>,
        rollback_to: Option<String>,
        intent_submission: Option<crate::governance::PromotionIntentSubmission>,
    ) -> Result<PromotionMutation<PromotionDecision>, CogniGraphError> {
        validate_record_id("evidence id", evidence_id)?;
        validate_idempotency_key(idempotency_key)?;
        if intent_submission.is_some() {
            actor.require_role("promoter")?;
        } else {
            actor.require_role("admin")?;
        }
        self.ensure_repository(tenant).await?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let id = new_record_id(tenant, incarnation, "decision", &key_hash);
        let key = scoped_key(tenant, incarnation, "d", &id);
        let request_digest = match &intent_submission {
            Some(authorization) => canonical_digest(&json!({
                "actor": actor,
                "idempotency_key_hash": key_hash,
                "authorization": authorization,
            }))?,
            None => canonical_digest(&json!({
                "action": requested_action,
                "evidence_id": evidence_id,
                "reason": reason,
                "expected_head_decision_id": expected_head_decision_id,
                "rollback_to": rollback_to,
            }))?,
        };
        if let Some(existing) = self
            .get_authority_raw::<PromotionDecision>(tenant, DECISIONS_COLLECTION, &key)
            .await?
        {
            self.validate_stored_decision(&existing, tenant, incarnation)?;
            if existing.request_digest != request_digest {
                self.metrics.conflicts.fetch_add(1, Ordering::Relaxed);
                return Err(conflict(
                    "Idempotency-Key was already used with a different promotion decision",
                ));
            }
            let reconciliation = self
                .reconcile_locked(tenant, incarnation, &existing.target, false)
                .await?;
            self.metrics
                .decisions_replayed
                .fetch_add(1, Ordering::Relaxed);
            return Ok(PromotionMutation {
                record: existing,
                replayed: true,
                head_changed: reconciliation.head_changed,
            });
        }
        self.ensure_mutations_healthy(tenant)?;
        let evidence = self
            .get_evidence_locked(tenant, incarnation, evidence_id)
            .await?;
        if intent_submission.is_none() && evidence.governance.is_some() {
            return Err(CogniGraphError::Forbidden(
                "M19 evidence requires a signed Promoter intent; Admin authority cannot decide it"
                    .into(),
            ));
        }
        let reconciliation = self
            .reconcile_locked(tenant, incarnation, &evidence.target, false)
            .await?;
        let current = self
            .current_raw(tenant, incarnation, &evidence.target)
            .await?;
        if requested_action == PromotionAction::Promote {
            let observed = current
                .as_ref()
                .map(|head| head.applied_decision_id.as_str());
            if observed != expected_head_decision_id.as_deref()
                || evidence.expected_head_decision_id.as_deref() != observed
            {
                self.metrics.conflicts.fetch_add(1, Ordering::Relaxed);
                return Err(conflict(
                    "promotion head changed after evidence registration",
                ));
            }
            if let Some(head) = &current {
                let selected_evidence = self
                    .get_evidence_locked(tenant, incarnation, &head.selection.evidence_id)
                    .await?;
                if evidence.rollback_target_evidence_id.as_deref()
                    != Some(selected_evidence.id.as_str())
                    || !same_construction_identity(
                        &evidence.runs[2].source.context,
                        &selected_evidence.runs[0].source.context,
                    )
                {
                    return Err(conflict(
                        "promotion evidence baseline does not match the current selected evidence",
                    ));
                }
            }
        }
        let gate_assessment_digest = canonical_digest(&evidence.gates)?;
        let accepted_at_ms = now_millis();
        self.validate_evidence_artifact_authority_active(
            tenant,
            incarnation,
            &evidence,
            accepted_at_ms,
        )
        .await?;
        let governance = if let Some(submission) = intent_submission {
            let binding = evidence.governance.as_ref().ok_or_else(|| {
                CogniGraphError::Forbidden(
                    "legacy unsigned M18 evidence cannot receive a new signed M19 decision".into(),
                )
            })?;
            self.validate_policy_binding_locked(
                tenant,
                incarnation,
                &evidence.target,
                &evidence.policy,
                binding,
                accepted_at_ms,
            )
            .await?;
            let supplied = &submission.statement.payload;
            if evidence
                .artifact_attestations
                .as_ref()
                .is_some_and(|authority| {
                    authority
                        .candidate
                        .attestor_principals()
                        .contains(supplied.promoter_principal_id.as_str())
                        || authority
                            .baseline
                            .attestor_principals()
                            .contains(supplied.promoter_principal_id.as_str())
                })
            {
                return Err(CogniGraphError::Forbidden(
                    "promoter principal must be distinct from artifact attestors".into(),
                ));
            }
            let expected = crate::governance::PromotionIntentPayload {
                requested_action: match requested_action {
                    PromotionAction::Promote => "promote",
                    PromotionAction::Reject => "reject",
                    _ => {
                        return Err(validation(
                            "signed evidence decision action is not promote or reject",
                        ));
                    }
                }
                .into(),
                target: evidence.target.clone(),
                evidence_id: evidence.id.clone(),
                evidence_digest: evidence.evidence_digest.clone(),
                policy_revision_id: Some(binding.policy_revision_id.clone()),
                policy_revision_digest: Some(binding.policy_revision_digest.clone()),
                approval_id: Some(binding.approval_id.clone()),
                approval_digest: Some(binding.approval_digest.clone()),
                artifact_authority_digest: evidence
                    .artifact_attestations
                    .as_ref()
                    .map(|authority| authority.authority_digest.clone()),
                consumption_authority_digest: evidence
                    .artifact_consumption
                    .as_ref()
                    .map(|authority| authority.authority_digest.clone()),
                derivation_authority_digest: evidence
                    .artifact_consumption
                    .as_ref()
                    .and_then(|authority| authority.derivation.as_ref())
                    .map(|authority| authority.authority_digest.clone()),
                preparation_authority_digest: evidence
                    .artifact_consumption
                    .as_ref()
                    .and_then(|authority| authority.derivation.as_ref())
                    .and_then(|authority| authority.preparation.as_ref())
                    .map(|authority| authority.authority_digest.clone()),
                gate_assessment_digest: gate_assessment_digest.clone(),
                expected_head_decision_id: expected_head_decision_id.clone(),
                rollback_target_evidence_id: if requested_action == PromotionAction::Promote {
                    evidence.rollback_target_evidence_id.clone()
                } else {
                    None
                },
                reason: reason.clone(),
                idempotency_key_hash: key_hash.clone(),
                promoter_registration_id: supplied.promoter_registration_id.clone(),
                promoter_principal_id: supplied.promoter_principal_id.clone(),
                signed_at_ms: supplied.signed_at_ms,
            };
            Some(
                self.authorize_promotion_intent_locked(
                    tenant,
                    incarnation,
                    &actor,
                    &expected,
                    submission,
                    Some(binding),
                    accepted_at_ms,
                )
                .await?,
            )
        } else {
            None
        };
        let action =
            if requested_action == PromotionAction::Promote && !evidence.gates.overall_passed {
                PromotionAction::Blocked
            } else {
                requested_action
            };
        if action == PromotionAction::Promote {
            ensure_actionable_capacity(reconciliation.decisions_scanned)?;
        }
        let selection = if action == PromotionAction::Promote {
            Some(PromotionSelection {
                generation: current.as_ref().map_or(Ok(1), |head| {
                    head.selection
                        .generation
                        .checked_add(1)
                        .ok_or_else(|| conflict("promotion generation is exhausted"))
                })?,
                evidence_id: evidence.id.clone(),
                evidence_digest: evidence.evidence_digest.clone(),
                candidate_digest: evidence.candidate_digest.clone(),
                policy_digest: evidence.policy_digest.clone(),
                prior_evidence_id: current
                    .as_ref()
                    .map(|head| head.selection.evidence_id.clone()),
            })
        } else {
            None
        };
        let mut decision = PromotionDecision {
            key,
            schema_version: if governance.is_some()
                && evidence_generation(&evidence)? == AuthorityGeneration::M23
            {
                M23_PROMOTION_DECISION_SCHEMA_VERSION
            } else if governance.is_some()
                && evidence_generation(&evidence)? == AuthorityGeneration::M22
            {
                M22_PROMOTION_DECISION_SCHEMA_VERSION
            } else if governance.is_some()
                && evidence_generation(&evidence)? == AuthorityGeneration::M21
            {
                M21_PROMOTION_DECISION_SCHEMA_VERSION
            } else if governance.is_some()
                && evidence_generation(&evidence)? == AuthorityGeneration::M20
            {
                M20_PROMOTION_DECISION_SCHEMA_VERSION
            } else if governance.is_some() {
                M19_PROMOTION_DECISION_SCHEMA_VERSION
            } else {
                PROMOTION_DECISION_SCHEMA_VERSION
            },
            digest_algorithm: DIGEST_ALGORITHM.into(),
            id,
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            target: evidence.target.clone(),
            action,
            actor,
            reason,
            created_at_ms: accepted_at_ms,
            idempotency_key_hash: key_hash,
            request_digest,
            decision_digest: String::new(),
            evidence_id: evidence.id.clone(),
            evidence_digest: evidence.evidence_digest.clone(),
            policy_digest: evidence.policy_digest.clone(),
            gate_assessment_digest,
            expected_head_decision_id,
            predecessor_decision_id: current
                .as_ref()
                .map(|head| head.applied_decision_id.clone()),
            resulting_selection: selection,
            governance,
        };
        decision.decision_digest = record_digest(&decision, "decision_digest")?;
        self.validate_decision(&decision, tenant, incarnation)?;
        self.insert_immutable(tenant, DECISIONS_COLLECTION, &decision.key, &decision)
            .await?;
        self.metrics
            .decisions_created
            .fetch_add(1, Ordering::Relaxed);
        if let Some(selection) = &decision.resulting_selection
            && let Err(error) = self.ensure_head_from_decision(&decision, selection).await
        {
            let committed = conflict(format!(
                "promotion decision {} committed but its head projection failed: {error}; replay the same idempotent request or run reconciliation",
                decision.id
            ));
            self.record_error(tenant, committed.to_string());
            return Err(committed);
        }
        if action == PromotionAction::Blocked {
            self.metrics.blocked.fetch_add(1, Ordering::Relaxed);
        }
        let head_changed = decision.resulting_selection.is_some();
        Ok(PromotionMutation {
            record: decision,
            replayed: false,
            head_changed,
        })
    }
}
