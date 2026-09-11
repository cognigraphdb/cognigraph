//! Evidence registration.

use super::*;

impl PromotionManager {
    pub async fn register_evidence(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: PromotionActor,
        idempotency_key: &str,
        request: RegisterEvidenceRequest,
    ) -> Result<PromotionMutation<PromotionEvidence>, CogniGraphError> {
        validate_idempotency_key(idempotency_key)?;
        actor.validate()?;
        request.validate()?;
        self.ensure_repository(tenant).await?;
        self.ensure_governance_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let id = new_record_id(tenant, incarnation, "evidence", &key_hash);
        let key = scoped_key(tenant, incarnation, "e", &id);
        let request_digest = canonical_digest(&request)?;
        if let Some(existing) = self
            .get_authority_raw::<PromotionEvidence>(tenant, EVIDENCE_COLLECTION, &key)
            .await?
        {
            self.validate_stored_evidence(&existing, tenant, incarnation)?;
            if existing.request_digest == request_digest {
                self.metrics
                    .evidence_replayed
                    .fetch_add(1, Ordering::Relaxed);
                return Ok(PromotionMutation {
                    record: existing,
                    replayed: true,
                    head_changed: false,
                });
            }
            self.metrics.conflicts.fetch_add(1, Ordering::Relaxed);
            return Err(conflict(
                "Idempotency-Key was already used with different evidence input",
            ));
        }
        self.ensure_mutations_healthy(tenant)?;

        let candidate_original = self
            .authoritative_evaluation_source(
                tenant,
                incarnation,
                &request.candidate_original_job_id,
            )
            .await?;
        let candidate_replay = self
            .authoritative_evaluation_source(tenant, incarnation, &request.candidate_replay_job_id)
            .await?;
        let baseline_original = self
            .authoritative_evaluation_source(tenant, incarnation, &request.baseline_original_job_id)
            .await?;
        let baseline_replay = self
            .authoritative_evaluation_source(tenant, incarnation, &request.baseline_replay_job_id)
            .await?;
        let target = candidate_original.context.target.clone();
        self.reconcile_locked(tenant, incarnation, &target, false)
            .await?;
        let current = self.current_raw(tenant, incarnation, &target).await?;
        let observed = current
            .as_ref()
            .map(|head| head.applied_decision_id.as_str());
        if observed != request.expected_head_decision_id.as_deref() {
            self.metrics.conflicts.fetch_add(1, Ordering::Relaxed);
            return Err(conflict(
                "expected_head_decision_id does not match the current promotion head",
            ));
        }
        if let Some(head) = &current {
            let selected_evidence = self
                .get_evidence_locked(tenant, incarnation, &head.selection.evidence_id)
                .await?;
            ensure_fresh_target_boundary(
                context_generation(&candidate_original.context)?,
                evidence_generation(&selected_evidence)?,
            )?;
            let selected_context = &selected_evidence.runs[0].source.context;
            let baseline_context = &baseline_original.context;
            if !same_construction_identity(baseline_context, selected_context) {
                return Err(conflict(
                    "baseline construction identity does not match the current promoted evidence",
                ));
            }
        }
        let gates = assess_evidence_runs(
            &candidate_original,
            &candidate_replay,
            &baseline_original,
            &baseline_replay,
        )?;
        let policy = candidate_original.context.policy.clone();
        let policy_digest = canonical_digest(&policy)?;
        let governance = candidate_original.context.governance.clone();
        let artifact_attestations = match (
            &candidate_original.context.artifact_attestations,
            &baseline_original.context.artifact_attestations,
        ) {
            (Some(candidate), Some(baseline)) => {
                let mut authority = EvidenceArtifactAuthority {
                    candidate: candidate.clone(),
                    baseline: baseline.clone(),
                    authority_digest: String::new(),
                };
                authority.authority_digest = record_digest(&authority, "authority_digest")?;
                authority.validate()?;
                Some(authority)
            }
            (None, None) => None,
            _ => {
                return Err(validation(
                    "candidate and baseline artifact authority generations differ",
                ));
            }
        };
        let artifact_consumption = match (
            &candidate_original.artifact_consumption,
            &candidate_replay.artifact_consumption,
            &baseline_original.artifact_consumption,
            &baseline_replay.artifact_consumption,
        ) {
            (
                Some(candidate_original),
                Some(candidate_replay),
                Some(baseline_original),
                Some(baseline_replay),
            ) => Some(Box::new(
                crate::artifact_consumption::EvidenceConsumptionAuthority::from_receipts(
                    candidate_original,
                    candidate_replay,
                    baseline_original,
                    baseline_replay,
                )?,
            )),
            (None, None, None, None) => None,
            _ => {
                return Err(validation(
                    "promotion evidence cannot mix consumed and legacy evaluation sources",
                ));
            }
        };
        let existing_evidence = self
            .all_scoped_records::<PromotionEvidence>(EVIDENCE_COLLECTION, tenant, incarnation, "e")
            .await?;
        let mut existing_generations = Vec::new();
        for record in &existing_evidence {
            self.validate_stored_evidence(record, tenant, incarnation)?;
            if record.target == target {
                existing_generations.push(evidence_generation(record)?);
            }
        }
        let generation = context_generation(&candidate_original.context)?;
        ensure_target_evidence_generation(generation, existing_generations)?;
        let accepted_at_ms = now_millis();
        if let Some(binding) = &governance {
            actor.require_role("promoter")?;
            self.validate_policy_binding_locked(
                tenant,
                incarnation,
                &target,
                &policy,
                binding,
                accepted_at_ms,
            )
            .await?;
            if artifact_attestations.as_ref().is_some_and(|authority| {
                artifact_authority_conflicts_with_policy(authority, binding)
            }) {
                return Err(CogniGraphError::Forbidden(
                "candidate and baseline artifact attestor principals must be distinct from policy author and approver"
                    .into(),
            ));
            }
        } else {
            actor.require_role("admin")?;
        }
        for source in [
            &candidate_original,
            &candidate_replay,
            &baseline_original,
            &baseline_replay,
        ] {
            if let Some(artifacts) = &source.context.artifact_attestations {
                self.validate_artifact_binding_set_active(
                    tenant,
                    incarnation,
                    &source.context,
                    artifacts,
                    accepted_at_ms,
                )
                .await?;
            }
        }
        let mut evidence = PromotionEvidence {
            key,
            schema_version: match generation {
                AuthorityGeneration::M18 => PROMOTION_EVIDENCE_SCHEMA_VERSION,
                AuthorityGeneration::M19 => M19_PROMOTION_EVIDENCE_SCHEMA_VERSION,
                AuthorityGeneration::M20 => M20_PROMOTION_EVIDENCE_SCHEMA_VERSION,
                AuthorityGeneration::M21 => M21_PROMOTION_EVIDENCE_SCHEMA_VERSION,
                AuthorityGeneration::M22 => M22_PROMOTION_EVIDENCE_SCHEMA_VERSION,
                AuthorityGeneration::M23 => M23_PROMOTION_EVIDENCE_SCHEMA_VERSION,
            },
            digest_algorithm: DIGEST_ALGORITHM.into(),
            id,
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            target,
            created_at_ms: accepted_at_ms,
            created_by: actor,
            idempotency_key_hash: key_hash,
            request_digest,
            evidence_digest: String::new(),
            expected_head_decision_id: request.expected_head_decision_id,
            rollback_target_evidence_id: current
                .as_ref()
                .map(|head| head.selection.evidence_id.clone()),
            candidate_digest: candidate_original
                .context
                .candidate
                .candidate_digest
                .clone(),
            baseline_candidate_digest: baseline_original.context.candidate.candidate_digest.clone(),
            policy,
            policy_digest,
            governance,
            artifact_attestations,
            artifact_consumption,
            runs: vec![
                EvidenceRun {
                    role: EvidenceRunRole::CandidateOriginal,
                    source: candidate_original,
                },
                EvidenceRun {
                    role: EvidenceRunRole::CandidateReplay,
                    source: candidate_replay,
                },
                EvidenceRun {
                    role: EvidenceRunRole::BaselineOriginal,
                    source: baseline_original,
                },
                EvidenceRun {
                    role: EvidenceRunRole::BaselineReplay,
                    source: baseline_replay,
                },
            ],
            gates,
        };
        evidence.evidence_digest = record_digest(&evidence, "evidence_digest")?;
        self.validate_evidence(&evidence, tenant, incarnation)?;
        self.insert_immutable(tenant, EVIDENCE_COLLECTION, &evidence.key, &evidence)
            .await?;
        self.metrics
            .evidence_created
            .fetch_add(1, Ordering::Relaxed);
        Ok(PromotionMutation {
            record: evidence,
            replayed: false,
            head_changed: false,
        })
    }
}
