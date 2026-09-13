//! Deployment.

use super::*;

impl PromotionManager {
    pub async fn deploy_semantic_repair_generation(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        generation_id: &str,
        authorization: SemanticRepairDeploymentIntentSubmission,
    ) -> Result<DeploymentMutation, CogniGraphError> {
        require_native_atomic_backend(self)?;
        actor.require_role(Role::Promoter)?;
        validate_idempotency_key(idempotency_key)?;
        validate_record_id("semantic_repair_generation_id", generation_id)?;
        self.ensure_repository(tenant).await?;
        self.ensure_governance_repository(tenant).await?;
        self.ensure_semantic_repair_repository(tenant).await?;
        self.ensure_materialized_repair_repository(tenant).await?;
        self.ensure_materialization_target_collections(tenant)
            .await?;

        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let decision_id =
            new_record_id(tenant, incarnation, "semantic-repair-deployment", &key_hash);
        let decision_key = scoped_key(tenant, incarnation, "m26d", &decision_id);
        let request_digest = canonical_digest(&json!({
            "actor": actor,
            "authorization": authorization,
        }))?;

        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        if let Some(existing) = self
            .get_authority_raw::<SemanticRepairDeploymentDecision>(
                tenant,
                SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
                &decision_key,
            )
            .await?
        {
            self.validate_stored_deployment_decision(&existing, tenant, incarnation)
                .await?;
            if existing.actor == actor && existing.request_digest == request_digest {
                // Validate the current chain and active target, but return the
                // exact historical resulting head for this idempotent act.
                self.current_semantic_repair_deployment_locked(
                    tenant,
                    incarnation,
                    &existing.space_type,
                )
                .await?
                .ok_or_else(|| conflict("replayed M26 deployment has no derived head"))?;
                let head = self.deployment_head_from_decision(&existing)?;
                self.record_m26_deployment(true);
                return Ok(DeploymentMutation {
                    decision: existing,
                    head,
                    replayed: true,
                });
            }
            return Err(conflict(
                "M26 deployment Idempotency-Key already names another signed act",
            ));
        }

        let generation = self
            .get_semantic_repair_generation_locked(tenant, incarnation, generation_id)
            .await?;
        let payload = &authorization.statement.payload;
        let pinned = self
            .pin_materialization_authority_locked(
                tenant,
                incarnation,
                &payload.target,
                &payload.promotion_head_decision_id,
            )
            .await?;
        if generation.target != payload.target
            || generation.source_evidence_id != pinned.evidence.id
            || generation.source_evidence_digest != pinned.evidence.evidence_digest
            || generation.candidate_digest != pinned.evidence.candidate_digest
            || generation.semantic_repair_revision_id
                != pinned.semantic.revision.semantic_repair_revision_id
            || generation.semantic_repair_revision_digest
                != pinned.semantic.revision.semantic_repair_revision_digest
            || generation.semantic_repair_review_id
                != pinned.semantic.review.semantic_repair_review_id
            || generation.semantic_repair_review_digest
                != pinned.semantic.review.semantic_repair_review_digest
        {
            return Err(conflict(
                "M26 generation does not match the current selected and approved authority",
            ));
        }
        let current = self
            .current_semantic_repair_deployment_locked(
                tenant,
                incarnation,
                &generation.target.space_type,
            )
            .await?;
        let expected_head = current
            .as_ref()
            .map(|head| head.applied_deployment_decision_id.clone());
        if payload.expected_deployment_head_decision_id != expected_head {
            return Err(conflict(
                "expected_deployment_head_decision_id does not match the current deployed head",
            ));
        }
        if current.as_ref().is_some_and(|head| {
            head.selection.semantic_repair_generation_id == generation.semantic_repair_generation_id
        }) {
            return Err(conflict("M26 deployment would be a no-op"));
        }

        match payload.requested_action {
            SemanticRepairDeploymentAction::Activate => {
                if payload.rollback_target_generation_id.is_some() {
                    return Err(validation(
                        "M26 activate requires rollback_target_generation_id to be explicit null",
                    ));
                }
                if current.is_some() && pinned.decision.action == PromotionAction::Rollback {
                    return Err(conflict(
                        "a control-plane rollback requires an explicit M26 rollback act",
                    ));
                }
            }
            SemanticRepairDeploymentAction::Rollback => {
                let current = current.as_ref().ok_or_else(|| {
                    conflict("M26 rollback requires an existing deployed generation")
                })?;
                if pinned.decision.action != PromotionAction::Rollback
                    || payload.rollback_target_generation_id.as_deref()
                        != Some(generation.semantic_repair_generation_id.as_str())
                    || current
                        .selection
                        .prior_semantic_repair_generation_id
                        .as_deref()
                        != Some(generation.semantic_repair_generation_id.as_str())
                {
                    return Err(conflict(
                        "M26 rollback is not the retained one-step prior generation selected by a signed promotion rollback",
                    ));
                }
            }
        }

        let now = now_millis();
        validate_deployment_intent_payload(payload, tenant, incarnation, now)?;
        if authorization.statement.schema_version
            != cognigraph_governance::GOVERNANCE_SCHEMA_VERSION
            || authorization.statement.domain != SEMANTIC_REPAIR_DEPLOYMENT_INTENT_DOMAIN
            || authorization.statement.tenant != tenant
            || authorization.statement.tenant_incarnation != incarnation
            || payload.semantic_repair_generation_id != generation.semantic_repair_generation_id
            || payload.semantic_repair_generation_digest
                != generation.semantic_repair_generation_digest
            || payload.impact_digest != generation.impact.impact_digest
            || payload.promotion_head_projection_digest != pinned.head.projection_digest
            || payload.candidate_digest != generation.candidate_digest
            || payload.semantic_repair_revision_id != generation.semantic_repair_revision_id
            || payload.semantic_repair_revision_digest != generation.semantic_repair_revision_digest
            || payload.semantic_repair_review_id != generation.semantic_repair_review_id
            || payload.semantic_repair_review_digest != generation.semantic_repair_review_digest
            || payload.idempotency_key_hash != key_hash
        {
            return Err(validation(
                "signed M26 deployment intent does not bind the exact current generation authority",
            ));
        }
        let promoter = self
            .get_active_key_locked(
                tenant,
                incarnation,
                &payload.promoter_registration_id,
                KeyPurpose::Promoter,
                now,
            )
            .await?;
        let promoter_revocation_id =
            key_revocation_id(tenant, incarnation, &payload.promoter_registration_id);
        let promoter_revocation: Option<GovernanceKeyRevocation> = match self
            .get_governance_revocation(tenant, incarnation, &promoter_revocation_id)
            .await
        {
            Ok(record) => Some(record),
            Err(CogniGraphError::DocumentNotFound { .. }) => None,
            Err(error) => return Err(error),
        };
        require_signing_actor(&promoter, &actor, &payload.promoter_principal_id)?;
        if promoter.principal_id == pinned.semantic.revision.author_principal_id
            || promoter.principal_id == pinned.semantic.review.approver_principal_id
            || pinned
                .evidence
                .artifact_attestations
                .as_ref()
                .is_some_and(|authority| {
                    authority
                        .candidate
                        .attestor_principals()
                        .contains(promoter.principal_id.as_str())
                        || authority
                            .baseline
                            .attestor_principals()
                            .contains(promoter.principal_id.as_str())
                })
        {
            return Err(CogniGraphError::Forbidden(
            "M26 deployment signer must be distinct from repair authors, reviewers, and artifact attestors"
                .into(),
        ));
        }
        promoter
            .verification_key
            .verify(
                &authorization.statement,
                &authorization.promoter_signature,
                SEMANTIC_REPAIR_DEPLOYMENT_INTENT_DOMAIN,
                tenant,
                incarnation,
                KeyPurpose::Promoter,
            )
            .map_err(|error| validation(format!("invalid M26 deployment signature: {error}")))?;

        let prior_generation_id = current
            .as_ref()
            .map(|head| head.selection.semantic_repair_generation_id.clone());
        let deployment_generation = current
            .as_ref()
            .map(|head| head.selection.generation)
            .unwrap_or_default()
            .checked_add(1)
            .ok_or_else(|| conflict("M26 deployment generation is exhausted"))?;
        let selection = SemanticRepairDeploymentSelection {
            generation: deployment_generation,
            semantic_repair_generation_id: generation.semantic_repair_generation_id.clone(),
            semantic_repair_generation_digest: generation.semantic_repair_generation_digest.clone(),
            target: generation.target.clone(),
            candidate_digest: generation.candidate_digest.clone(),
            prior_semantic_repair_generation_id: prior_generation_id,
        };
        let signed_intent = SignedSemanticRepairDeploymentIntent {
            statement: authorization.statement.clone(),
            promoter_signature: authorization.promoter_signature.clone(),
            promoter_registration: promoter.clone(),
        };
        let mut decision = SemanticRepairDeploymentDecision {
            key: decision_key,
            schema_version: MATERIALIZATION_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            deployment_decision_id: decision_id,
            deployment_decision_digest: String::new(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            space_type: generation.target.space_type.clone(),
            action: payload.requested_action,
            target: generation.target.clone(),
            semantic_repair_generation_id: generation.semantic_repair_generation_id.clone(),
            semantic_repair_generation_digest: generation.semantic_repair_generation_digest.clone(),
            impact_digest: generation.impact.impact_digest.clone(),
            promotion_head_decision_id: pinned.head.applied_decision_id.clone(),
            promotion_head_projection_digest: pinned.head.projection_digest.clone(),
            candidate_digest: generation.candidate_digest.clone(),
            expected_deployment_head_decision_id: expected_head.clone(),
            predecessor_deployment_decision_id: expected_head,
            resulting_selection: selection.clone(),
            deployment_intent: signed_intent,
            actor,
            reason: payload.reason.clone(),
            created_at_ms: now,
            idempotency_key_hash: key_hash,
            request_digest,
        };
        decision.deployment_decision_digest =
            record_digest(&decision, "deployment_decision_digest")?;
        validate_deployment_decision_against(
            &decision,
            tenant,
            incarnation,
            &generation,
            &pinned.decision,
            &pinned.head.projection_digest,
            &pinned.evidence,
            &pinned.semantic.revision,
            &pinned.semantic.review,
            &promoter,
            promoter_revocation.as_ref(),
        )?;
        let mut head = SemanticRepairDeploymentHead {
            key: deployment_head_key(tenant, incarnation, &generation.target.space_type)?,
            schema_version: MATERIALIZATION_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            space_type: generation.target.space_type.clone(),
            applied_deployment_decision_id: decision.deployment_decision_id.clone(),
            selection,
            updated_at_ms: now,
            projection_digest: String::new(),
        };
        head.projection_digest = record_digest(&head, "projection_digest")?;

        let decision_count = self
            .deployment_decisions_for_space_locked(
                tenant,
                incarnation,
                &generation.target.space_type,
            )
            .await?
            .len();
        let proposed_decision_count = decision_count
            .checked_add(1)
            .ok_or_else(|| capacity("M26 deployment decision count overflowed"))?;
        validate_deployment_chain_capacities([(
        generation.target.space_type.as_str(),
        proposed_decision_count,
    )])
    .map_err(|violation| match violation {
        DeploymentChainCapacityViolation::DecisionCount => capacity(format!(
            "M26 deployment decision limit of {MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE} is exhausted for this space"
        )),
        DeploymentChainCapacityViolation::SpaceCount => {
            capacity("M26 deployment space limit is exhausted")
        }
    })?;

        let mut ops = self
            .target_replacement_ops(&generation.target.space_type, &generation.projection)
            .await?;
        ops.push(BatchOp::Insert {
            collection: SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION.into(),
            doc: serde_json::to_value(&decision)?,
        });
        let existing_head = self
            .backend
            .get_document(SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION, &head.key)
            .await?
            .is_some();
        ops.push(if existing_head {
            BatchOp::Replace {
                collection: SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION.into(),
                key: head.key.clone(),
                doc: serde_json::to_value(&head)?,
            }
        } else {
            BatchOp::Insert {
                collection: SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION.into(),
                doc: serde_json::to_value(&head)?,
            }
        });
        self.backend.execute_batch(ops).await?;
        self.record_m26_deployment(false);
        Ok(DeploymentMutation {
            decision,
            head,
            replayed: false,
        })
    }
}
