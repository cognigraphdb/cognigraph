//! Generation validation.

use super::*;

impl PromotionManager {
    pub(super) async fn validate_stored_semantic_repair_generation(
        &self,
        record: &SemanticRepairGenerationRecord,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let result = async {
            let decision = self
                .get_decision(tenant, incarnation, &record.promotion_head_decision_id)
                .await?;
            let selection = decision
                .resulting_selection
                .as_ref()
                .ok_or_else(|| conflict("M26 generation source decision has no selection"))?;
            let head = self.head_from_decision(&decision, selection)?;
            let semantic = self
                .resolve_semantic_repair_authority_locked(
                    tenant,
                    incarnation,
                    &head,
                    &record.candidate_digest,
                )
                .await?;
            let evidence = self
                .get_evidence(tenant, incarnation, &decision.evidence_id)
                .await?;
            self.validate_semantic_repair_generation_against(
                record,
                tenant,
                incarnation,
                &PinnedMaterializationAuthority {
                    head,
                    decision,
                    evidence,
                    semantic,
                },
            )
        }
        .await;
        result.inspect_err(|error| self.record_error(tenant, error.to_string()))
    }

    pub(super) fn validate_semantic_repair_generation_against(
        &self,
        record: &SemanticRepairGenerationRecord,
        tenant: &str,
        incarnation: &str,
        authority: &PinnedMaterializationAuthority,
    ) -> Result<(), CogniGraphError> {
        record.materialization_plan.validate()?;
        record.projection.validate()?;
        let candidate_original =
            source_by_role(&authority.evidence, EvidenceRunRole::CandidateOriginal)?;
        let candidate_replay =
            source_by_role(&authority.evidence, EvidenceRunRole::CandidateReplay)?;
        let baseline_original =
            source_by_role(&authority.evidence, EvidenceRunRole::BaselineOriginal)?;
        let baseline_replay = source_by_role(&authority.evidence, EvidenceRunRole::BaselineReplay)?;
        let candidate_original_receipt = candidate_original
            .artifact_consumption
            .as_deref()
            .ok_or_else(|| conflict("M26 generation source has no candidate receipt"))?;
        let candidate_replay_receipt = candidate_replay
            .artifact_consumption
            .as_deref()
            .ok_or_else(|| conflict("M26 generation source has no candidate replay receipt"))?;
        let candidate_derivation = derivation_for(candidate_original)?;
        let candidate_replay_derivation = derivation_for(candidate_replay)?;
        let baseline_derivation = derivation_for(baseline_original)?;
        let baseline_replay_derivation = derivation_for(baseline_replay)?;
        record.impact.validate(
            &candidate_derivation.facts,
            &baseline_derivation.facts,
            &record.projection.projection_digest,
        )?;
        let identity = json!({
            "tenant": tenant,
            "tenant_incarnation": incarnation,
            "target": record.target,
            "source_evidence_id": record.source_evidence_id,
            "source_evidence_digest": record.source_evidence_digest,
            "candidate_digest": record.candidate_digest,
            "semantic_repair_revision_digest": record.semantic_repair_revision_digest,
            "semantic_repair_review_digest": record.semantic_repair_review_digest,
            "prepared_corpus_digest": record.prepared_corpus_digest,
            "derivation_material_digest": record.derivation_material_digest,
            "materialization_plan_digest": record.materialization_plan.plan_digest,
            "projection_digest": record.projection.projection_digest,
            "impact_digest": record.impact.impact_digest,
        });
        if record.schema_version != MATERIALIZATION_RECORD_SCHEMA_VERSION
            || record.digest_algorithm != DIGEST_ALGORITHM
            || record.tenant != tenant
            || record.tenant_incarnation != incarnation
            || record.key
                != scoped_key(
                    tenant,
                    incarnation,
                    "m26g",
                    &record.semantic_repair_generation_id,
                )
            || record.semantic_repair_generation_id != generation_id(&identity)?
            || record.target != authority.evidence.target
            || record.projection.space_id != record.target.space_type
            || record.source_evidence_id != authority.evidence.id
            || record.source_evidence_digest != authority.evidence.evidence_digest
            || record.candidate_digest != authority.evidence.candidate_digest
            || record.candidate_digest != authority.semantic.revision.candidate_digest
            || record.promotion_head_decision_id != authority.decision.id
            || record.promotion_head_projection_digest != authority.head.projection_digest
            || record.semantic_repair_revision_id
                != authority.semantic.revision.semantic_repair_revision_id
            || record.semantic_repair_revision_digest
                != authority.semantic.revision.semantic_repair_revision_digest
            || record.semantic_repair_review_id
                != authority.semantic.review.semantic_repair_review_id
            || record.semantic_repair_review_digest
                != authority.semantic.review.semantic_repair_review_digest
            || record.source_candidate_original_job_id != candidate_original.job_id
            || record.source_candidate_original_receipt_digest
                != candidate_original_receipt.receipt_digest
            || record.source_candidate_replay_job_id != candidate_replay.job_id
            || record.source_candidate_replay_receipt_digest
                != candidate_replay_receipt.receipt_digest
            || record.prepared_corpus_digest != candidate_derivation.corpus_semantic_digest
            || record.derivation_plan_digest != candidate_derivation.derivation_plan_digest
            || record.derivation_material_digest != candidate_derivation.derivation_material_digest
            || candidate_derivation.facts != candidate_replay_derivation.facts
            || candidate_derivation.derivation_material_digest
                != candidate_replay_derivation.derivation_material_digest
            || baseline_derivation.facts != baseline_replay_derivation.facts
            || baseline_derivation.derivation_material_digest
                != baseline_replay_derivation.derivation_material_digest
            || candidate_derivation.corpus_semantic_digest
                != baseline_derivation.corpus_semantic_digest
            || record.projection.semantic_facts != candidate_derivation.facts
            || record.created_at_ms == 0
            || record.canonical_record_bytes as usize > MAX_M26_CANONICAL_GENERATION_BYTES
            || record.semantic_repair_generation_digest
                != record_digest(record, "semantic_repair_generation_digest")?
            || canonical_json_bytes(record)?.len() as u64 != record.canonical_record_bytes
        {
            return Err(conflict(
                "stored M26 Semantic Repair generation is malformed",
            ));
        }
        record.created_by.require_role(Role::Promoter)?;
        validate_digest(
            "M26 generation idempotency hash",
            &record.idempotency_key_hash,
        )?;
        let expected_request = BuildSemanticRepairGenerationRequest {
            target: record.target.clone(),
            expected_promotion_head_decision_id: record.promotion_head_decision_id.clone(),
        };
        if record.request_digest != canonical_digest(&expected_request)? {
            return Err(conflict(
                "stored M26 generation request binding is malformed",
            ));
        }
        Ok(())
    }
}
