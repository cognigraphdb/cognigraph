//! Rollback.

use super::*;

impl PromotionManager {
    pub async fn rollback_signed(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        actor: PromotionActor,
        idempotency_key: &str,
        authorization: crate::governance::PromotionIntentSubmission,
    ) -> Result<PromotionMutation<PromotionDecision>, CogniGraphError> {
        let payload = &authorization.statement.payload;
        if payload.requested_action != "rollback"
            || payload.target != *target
            || payload.rollback_target_evidence_id.as_deref() != Some(payload.evidence_id.as_str())
        {
            return Err(CogniGraphError::ValidationError(
                "signed rollback intent action, target, or evidence mismatch".into(),
            ));
        }
        let expected_head_decision_id = payload
            .expected_head_decision_id
            .clone()
            .ok_or_else(|| validation("signed rollback requires an expected head"))?;
        self.rollback_with_intent(
            tenant,
            incarnation,
            target,
            actor,
            idempotency_key,
            RollbackRequest {
                expected_head_decision_id,
                to_evidence_id: payload.evidence_id.clone(),
                reason: payload.reason.clone(),
            },
            Some(authorization),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn rollback_with_intent(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        actor: PromotionActor,
        idempotency_key: &str,
        request: RollbackRequest,
        intent_submission: Option<crate::governance::PromotionIntentSubmission>,
    ) -> Result<PromotionMutation<PromotionDecision>, CogniGraphError> {
        target.validate()?;
        if intent_submission.is_some() {
            actor.require_role("promoter")?;
        } else {
            actor.require_role("admin")?;
        }
        validate_idempotency_key(idempotency_key)?;
        validate_reason(&request.reason)?;
        validate_record_id(
            "expected_head_decision_id",
            &request.expected_head_decision_id,
        )?;
        validate_record_id("to_evidence_id", &request.to_evidence_id)?;
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
                "action": PromotionAction::Rollback,
                "target": target,
                "request": request,
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
                    "Idempotency-Key was already used with a different rollback",
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
        let reconciliation = self
            .reconcile_locked(tenant, incarnation, target, false)
            .await?;
        ensure_actionable_capacity(reconciliation.decisions_scanned)?;
        let current = self
            .current_raw(tenant, incarnation, target)
            .await?
            .ok_or_else(|| conflict("cannot rollback a target without an active promotion"))?;
        if current.applied_decision_id != request.expected_head_decision_id {
            self.metrics.conflicts.fetch_add(1, Ordering::Relaxed);
            return Err(conflict("rollback expected head is stale"));
        }
        if current.selection.prior_evidence_id.as_deref() != Some(request.to_evidence_id.as_str()) {
            return Err(conflict(
                "rollback target is not the current head's explicit prior evidence",
            ));
        }
        let target_evidence = self
            .get_evidence_locked(tenant, incarnation, &request.to_evidence_id)
            .await?;
        if intent_submission.is_none() && target_evidence.governance.is_some() {
            return Err(CogniGraphError::Forbidden(
                "M19 rollback requires a signed Promoter intent; Admin authority cannot decide it"
                    .into(),
            ));
        }
        if target_evidence.target != *target {
            return Err(conflict("rollback evidence belongs to another target"));
        }
        let selection = PromotionSelection {
            generation: current
                .selection
                .generation
                .checked_add(1)
                .ok_or_else(|| conflict("promotion generation is exhausted"))?,
            evidence_id: target_evidence.id.clone(),
            evidence_digest: target_evidence.evidence_digest.clone(),
            candidate_digest: target_evidence.candidate_digest.clone(),
            policy_digest: target_evidence.policy_digest.clone(),
            prior_evidence_id: Some(current.selection.evidence_id.clone()),
        };
        let gate_assessment_digest = canonical_digest(&target_evidence.gates)?;
        let accepted_at_ms = now_millis();
        self.validate_evidence_artifact_authority_active(
            tenant,
            incarnation,
            &target_evidence,
            accepted_at_ms,
        )
        .await?;
        let governance = if let Some(submission) = intent_submission {
            let binding = target_evidence.governance.as_ref();
            if let Some(binding) = binding {
                self.validate_policy_binding_locked(
                    tenant,
                    incarnation,
                    &target_evidence.target,
                    &target_evidence.policy,
                    binding,
                    accepted_at_ms,
                )
                .await?;
            }
            let supplied = &submission.statement.payload;
            if target_evidence
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
                requested_action: "rollback".into(),
                target: target.clone(),
                evidence_id: target_evidence.id.clone(),
                evidence_digest: target_evidence.evidence_digest.clone(),
                policy_revision_id: binding.map(|binding| binding.policy_revision_id.clone()),
                policy_revision_digest: binding
                    .map(|binding| binding.policy_revision_digest.clone()),
                approval_id: binding.map(|binding| binding.approval_id.clone()),
                approval_digest: binding.map(|binding| binding.approval_digest.clone()),
                artifact_authority_digest: target_evidence
                    .artifact_attestations
                    .as_ref()
                    .map(|authority| authority.authority_digest.clone()),
                consumption_authority_digest: target_evidence
                    .artifact_consumption
                    .as_ref()
                    .map(|authority| authority.authority_digest.clone()),
                derivation_authority_digest: target_evidence
                    .artifact_consumption
                    .as_ref()
                    .and_then(|authority| authority.derivation.as_ref())
                    .map(|authority| authority.authority_digest.clone()),
                preparation_authority_digest: target_evidence
                    .artifact_consumption
                    .as_ref()
                    .and_then(|authority| authority.derivation.as_ref())
                    .and_then(|authority| authority.preparation.as_ref())
                    .map(|authority| authority.authority_digest.clone()),
                gate_assessment_digest: gate_assessment_digest.clone(),
                expected_head_decision_id: Some(request.expected_head_decision_id.clone()),
                rollback_target_evidence_id: Some(target_evidence.id.clone()),
                reason: request.reason.clone(),
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
                    binding,
                    accepted_at_ms,
                )
                .await?,
            )
        } else {
            None
        };
        let mut decision = PromotionDecision {
            key,
            schema_version: if governance.is_some()
                && evidence_generation(&target_evidence)? == AuthorityGeneration::M23
            {
                M23_PROMOTION_DECISION_SCHEMA_VERSION
            } else if governance.is_some()
                && evidence_generation(&target_evidence)? == AuthorityGeneration::M22
            {
                M22_PROMOTION_DECISION_SCHEMA_VERSION
            } else if governance.is_some()
                && evidence_generation(&target_evidence)? == AuthorityGeneration::M21
            {
                M21_PROMOTION_DECISION_SCHEMA_VERSION
            } else if governance.is_some()
                && evidence_generation(&target_evidence)? == AuthorityGeneration::M20
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
            target: target.clone(),
            action: PromotionAction::Rollback,
            actor,
            reason: request.reason,
            created_at_ms: accepted_at_ms,
            idempotency_key_hash: key_hash,
            request_digest,
            decision_digest: String::new(),
            evidence_id: target_evidence.id.clone(),
            evidence_digest: target_evidence.evidence_digest.clone(),
            policy_digest: target_evidence.policy_digest.clone(),
            gate_assessment_digest,
            expected_head_decision_id: Some(request.expected_head_decision_id),
            predecessor_decision_id: Some(current.applied_decision_id),
            resulting_selection: Some(selection),
            governance,
        };
        decision.decision_digest = record_digest(&decision, "decision_digest")?;
        self.validate_decision(&decision, tenant, incarnation)?;
        self.insert_immutable(tenant, DECISIONS_COLLECTION, &decision.key, &decision)
            .await?;
        self.metrics
            .decisions_created
            .fetch_add(1, Ordering::Relaxed);
        if let Err(error) = self
            .ensure_head_from_decision(
                &decision,
                decision
                    .resulting_selection
                    .as_ref()
                    .expect("rollback has resulting selection"),
            )
            .await
        {
            let committed = conflict(format!(
                "rollback decision {} committed but its head projection failed: {error}; replay the same idempotent request or run reconciliation",
                decision.id
            ));
            self.record_error(tenant, committed.to_string());
            return Err(committed);
        }
        Ok(PromotionMutation {
            record: decision,
            replayed: false,
            head_changed: true,
        })
    }

    #[cfg(test)]
    pub async fn rollback(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        actor: PromotionActor,
        idempotency_key: &str,
        request: RollbackRequest,
    ) -> Result<PromotionMutation<PromotionDecision>, CogniGraphError> {
        self.rollback_with_intent(
            tenant,
            incarnation,
            target,
            actor,
            idempotency_key,
            request,
            None,
        )
        .await
    }
}
