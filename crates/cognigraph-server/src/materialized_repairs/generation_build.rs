//! Generation build.

use super::*;

impl PromotionManager {
    pub async fn build_semantic_repair_generation(
        &self,
        cas: &LocalArtifactCas,
        tenant: &str,
        incarnation: &str,
        actor: GovernanceActor,
        idempotency_key: &str,
        request: BuildSemanticRepairGenerationRequest,
    ) -> Result<MaterializationMutation<SemanticRepairGenerationRecord>, CogniGraphError> {
        require_native_atomic_backend(self)?;
        actor.require_role(Role::Promoter)?;
        validate_idempotency_key(idempotency_key)?;
        request.target.validate()?;
        validate_record_id(
            "expected_promotion_head_decision_id",
            &request.expected_promotion_head_decision_id,
        )?;
        self.ensure_repository(tenant).await?;
        self.ensure_governance_repository(tenant).await?;
        self.ensure_semantic_repair_repository(tenant).await?;
        self.ensure_materialized_repair_repository(tenant).await?;

        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = canonical_digest(&request)?;
        let pinned = {
            let _guard = self.transition_lock.lock().await;
            if let Some(existing) = self
                .generation_by_idempotency_locked(tenant, incarnation, &key_hash)
                .await?
            {
                self.validate_stored_semantic_repair_generation(&existing, tenant, incarnation)
                    .await?;
                if existing.created_by == actor && existing.request_digest == request_digest {
                    self.record_m26_generation(true);
                    return Ok(MaterializationMutation {
                        record: existing,
                        replayed: true,
                    });
                }
                return Err(conflict(
                    "M26 generation Idempotency-Key already names another request or actor",
                ));
            }
            self.pin_materialization_authority_locked(
                tenant,
                incarnation,
                &request.target,
                &request.expected_promotion_head_decision_id,
            )
            .await?
        };

        let candidate_original =
            source_by_role(&pinned.evidence, EvidenceRunRole::CandidateOriginal)?;
        let candidate_replay = source_by_role(&pinned.evidence, EvidenceRunRole::CandidateReplay)?;
        let baseline_original =
            source_by_role(&pinned.evidence, EvidenceRunRole::BaselineOriginal)?;
        let baseline_replay = source_by_role(&pinned.evidence, EvidenceRunRole::BaselineReplay)?;
        let candidate_original_receipt = candidate_original
            .artifact_consumption
            .as_deref()
            .ok_or_else(|| {
                conflict("M26 candidate original has no verified consumption receipt")
            })?;
        let candidate_replay_receipt = candidate_replay
            .artifact_consumption
            .as_deref()
            .ok_or_else(|| conflict("M26 candidate replay has no verified consumption receipt"))?;
        let candidate_derivation = derivation_for(candidate_original)?;
        let candidate_replay_derivation = derivation_for(candidate_replay)?;
        let baseline_derivation = derivation_for(baseline_original)?;
        let baseline_replay_derivation = derivation_for(baseline_replay)?;

        let corpus_entry = candidate_derivation
            .corpus_manifest
            .entries
            .iter()
            .find(|entry| entry.logical_path == CORPUS_ENTRYPOINT)
            .ok_or_else(|| conflict("M26 candidate corpus manifest has no corpus.json"))?;
        if corpus_entry.blob_digest != candidate_derivation.corpus_blob_digest
            || corpus_entry.byte_length > MAX_M26_CANONICAL_GENERATION_BYTES as u64 * 4
        {
            return Err(conflict(
                "M26 candidate corpus entry does not match its derivation receipt",
            ));
        }
        let mut budget = cas.begin_evaluation(tenant, incarnation)?;
        let verified = cas
            .verify_blob(
                &mut budget,
                &corpus_entry.blob_digest,
                corpus_entry.byte_length,
                Some(MAX_M26_CANONICAL_GENERATION_BYTES as u64 * 4),
            )
            .await?;
        let corpus_bytes = verified
            .retained_bytes
            .ok_or_else(|| conflict("M26 verified corpus bytes were not retained"))?;
        let corpus: PreparedChunkCorpusArtifact = serde_json::from_slice(&corpus_bytes)
            .map_err(|error| validation(format!("invalid M26 prepared corpus JSON: {error}")))?;
        if canonical_json_bytes(&corpus)? != corpus_bytes
            || canonical_digest(&corpus)? != candidate_derivation.corpus_semantic_digest
            || candidate_derivation.corpus_semantic_digest != corpus_entry.blob_digest
        {
            return Err(conflict(
                "M26 prepared corpus bytes are not the exact canonical verified corpus",
            ));
        }
        let chunks = corpus.materialization_chunks(&candidate_original.context)?;
        if chunks.len() > MAX_M26_CHUNKS {
            return Err(capacity(format!(
                "M26 generation has {} chunks, exceeding the {MAX_M26_CHUNKS} chunk limit",
                chunks.len()
            )));
        }
        let (effective, vetoes, resolved_digest) = pinned
            .semantic
            .revision
            .candidate
            .resolve_materialization(&candidate_original.context, &chunks)?;
        if resolved_digest != candidate_derivation.resolved_config_digest {
            return Err(conflict(
                "M26 resolved configuration differs from the M22/M23 authority",
            ));
        }
        let derived = derive_materialized_graph(
            &request.target.space_type,
            &chunks,
            &effective,
            &vetoes,
            materialization_options(),
        )
        .await
        .map_err(|error| match error {
            MaterializationError::EntityLimitExceeded { .. }
            | MaterializationError::ChunkLimitExceeded { .. }
            | MaterializationError::MentionLimitExceeded { .. }
            | MaterializationError::FactOccurrenceLimitExceeded { .. }
            | MaterializationError::SemanticFactLimitExceeded { .. } => {
                capacity(format!("M26 materialization failed: {error}"))
            }
            _ => validation(format!("M26 materialization failed: {error}")),
        })?;
        let projection = MaterializedProjection::from_derived(derived)?;
        if projection.semantic_facts != candidate_derivation.facts
            || projection.semantic_facts != candidate_replay_derivation.facts
            || projection.semantic_facts_digest != candidate_derivation.facts_digest
            || projection.semantic_facts_digest != candidate_replay_derivation.facts_digest
            || baseline_derivation.facts != baseline_replay_derivation.facts
        {
            return Err(conflict(
                "M26 complete occurrence projection does not exactly reproduce both candidate receipts",
            ));
        }
        let impact = VerifiedSemanticRepairImpact::new(
            &projection.semantic_facts,
            &baseline_derivation.facts,
            &projection.projection_digest,
        )?;
        let plan = MaterializationPlan::current()?;
        let identity = json!({
            "tenant": tenant,
            "tenant_incarnation": incarnation,
            "target": request.target,
            "source_evidence_id": pinned.evidence.id,
            "source_evidence_digest": pinned.evidence.evidence_digest,
            "candidate_digest": pinned.evidence.candidate_digest,
            "semantic_repair_revision_digest": pinned.semantic.revision.semantic_repair_revision_digest,
            "semantic_repair_review_digest": pinned.semantic.review.semantic_repair_review_digest,
            "prepared_corpus_digest": candidate_derivation.corpus_semantic_digest,
            "derivation_material_digest": candidate_derivation.derivation_material_digest,
            "materialization_plan_digest": plan.plan_digest,
            "projection_digest": projection.projection_digest,
            "impact_digest": impact.impact_digest,
        });
        let id = generation_id(&identity)?;
        let mut record = SemanticRepairGenerationRecord {
            key: scoped_key(tenant, incarnation, "m26g", &id),
            schema_version: MATERIALIZATION_RECORD_SCHEMA_VERSION,
            digest_algorithm: DIGEST_ALGORITHM.into(),
            tenant: tenant.into(),
            tenant_incarnation: incarnation.into(),
            semantic_repair_generation_id: id,
            target: request.target.clone(),
            source_evidence_id: pinned.evidence.id.clone(),
            source_evidence_digest: pinned.evidence.evidence_digest.clone(),
            source_candidate_original_job_id: candidate_original.job_id.clone(),
            source_candidate_original_receipt_digest: candidate_original_receipt
                .receipt_digest
                .clone(),
            source_candidate_replay_job_id: candidate_replay.job_id.clone(),
            source_candidate_replay_receipt_digest: candidate_replay_receipt.receipt_digest.clone(),
            promotion_head_decision_id: pinned.head.applied_decision_id.clone(),
            promotion_head_projection_digest: pinned.head.projection_digest.clone(),
            candidate_digest: pinned.evidence.candidate_digest.clone(),
            semantic_repair_revision_id: pinned
                .semantic
                .revision
                .semantic_repair_revision_id
                .clone(),
            semantic_repair_revision_digest: pinned
                .semantic
                .revision
                .semantic_repair_revision_digest
                .clone(),
            semantic_repair_review_id: pinned.semantic.review.semantic_repair_review_id.clone(),
            semantic_repair_review_digest: pinned
                .semantic
                .review
                .semantic_repair_review_digest
                .clone(),
            prepared_corpus_digest: candidate_derivation.corpus_semantic_digest.clone(),
            derivation_plan_digest: candidate_derivation.derivation_plan_digest.clone(),
            derivation_material_digest: candidate_derivation.derivation_material_digest.clone(),
            materialization_plan: plan,
            projection,
            impact,
            created_at_ms: now_millis(),
            created_by: actor.clone(),
            idempotency_key_hash: key_hash.clone(),
            request_digest: request_digest.clone(),
            canonical_record_bytes: 0,
            semantic_repair_generation_digest: String::new(),
        };
        stabilize_generation_record_size(&mut record)?;
        self.validate_semantic_repair_generation_against(&record, tenant, incarnation, &pinned)?;

        let _guard = self.transition_lock.lock().await;
        let current = self
            .pin_materialization_authority_locked(
                tenant,
                incarnation,
                &request.target,
                &request.expected_promotion_head_decision_id,
            )
            .await?;
        if current.head != pinned.head
            || current.decision != pinned.decision
            || current.evidence != pinned.evidence
            || current.semantic != pinned.semantic
        {
            return Err(conflict(
                "M26 promotion or Semantic Repair authority changed while materializing",
            ));
        }
        // Another synchronous build may have crossed the unlocked CAS and
        // derivation phase with this request. Recheck the global M26
        // idempotency namespace while holding the final write lock so one
        // key cannot authorize two natural generations and identical races
        // converge on the first immutable record.
        if let Some(existing) = self
            .generation_by_idempotency_locked(tenant, incarnation, &key_hash)
            .await?
        {
            self.validate_stored_semantic_repair_generation(&existing, tenant, incarnation)
                .await?;
            if existing.created_by == actor && existing.request_digest == request_digest {
                self.record_m26_generation(true);
                return Ok(MaterializationMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "M26 generation Idempotency-Key already names another request or actor",
            ));
        }
        if let Some(existing) = self
            .get_authority_raw::<SemanticRepairGenerationRecord>(
                tenant,
                SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
                &record.key,
            )
            .await?
        {
            self.validate_stored_semantic_repair_generation(&existing, tenant, incarnation)
                .await?;
            if existing == record {
                self.record_m26_generation(true);
                return Ok(MaterializationMutation {
                    record: existing,
                    replayed: true,
                });
            }
            return Err(conflict(
                "M26 natural generation identity already has different immutable content",
            ));
        }
        self.ensure_generation_capacity_locked(tenant, incarnation, &record)
            .await?;
        self.insert_immutable(
            tenant,
            SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
            &record.key,
            &record,
        )
        .await?;
        self.record_m26_generation(false);
        Ok(MaterializationMutation {
            record,
            replayed: false,
        })
    }
}
