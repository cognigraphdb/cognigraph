//! Authority pinning.

use super::*;

impl PromotionManager {
    pub(super) async fn pin_materialization_authority_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        expected_head_decision_id: &str,
    ) -> Result<PinnedMaterializationAuthority, CogniGraphError> {
        target.validate()?;
        validate_record_id(
            "expected_promotion_head_decision_id",
            expected_head_decision_id,
        )?;
        self.ensure_tenant_active(tenant)?;
        self.ensure_mutations_healthy(tenant)?;
        self.reconcile_locked(tenant, incarnation, target, false)
            .await?;
        let semantic = self
            .resolve_current_semantic_repair_authority_locked(tenant, incarnation, target)
            .await?;
        let head = self
            .current_raw(tenant, incarnation, target)
            .await?
            .ok_or_else(|| conflict("M26 generation requires a current promotion head"))?;
        if head.applied_decision_id != expected_head_decision_id {
            return Err(conflict(
                "expected_promotion_head_decision_id does not match the current promotion head",
            ));
        }
        let decision = self
            .get_decision(tenant, incarnation, &head.applied_decision_id)
            .await?;
        let evidence = self
            .get_evidence(tenant, incarnation, &head.selection.evidence_id)
            .await?;
        if !evidence.gates.overall_passed
            || evidence.target != *target
            || evidence.id != head.selection.evidence_id
            || evidence.evidence_digest != head.selection.evidence_digest
            || evidence.candidate_digest != head.selection.candidate_digest
            || evidence.candidate_digest != semantic.revision.candidate_digest
            || decision.target != *target
            || decision.evidence_id != evidence.id
            || decision.evidence_digest != evidence.evidence_digest
            || !matches!(
                decision.action,
                PromotionAction::Promote | PromotionAction::Rollback
            )
            || decision.resulting_selection.as_ref() != Some(&head.selection)
            || decision.governance.is_none()
        {
            return Err(conflict(
                "M26 current promotion, evidence, and approved Semantic Repair authority disagree",
            ));
        }
        let candidate_original = source_by_role(&evidence, EvidenceRunRole::CandidateOriginal)?;
        let candidate_replay = source_by_role(&evidence, EvidenceRunRole::CandidateReplay)?;
        let baseline_original = source_by_role(&evidence, EvidenceRunRole::BaselineOriginal)?;
        let baseline_replay = source_by_role(&evidence, EvidenceRunRole::BaselineReplay)?;
        if !matches!(
            candidate_original.context.schema_version,
            crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                | crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION
        ) || candidate_replay.context.schema_version != candidate_original.context.schema_version
            || baseline_original.context.schema_version != candidate_original.context.schema_version
            || baseline_replay.context.schema_version != candidate_original.context.schema_version
        {
            return Err(conflict(
                "M26 generation requires one consistent M22 or M23 evaluation authority",
            ));
        }
        let candidate_original_derivation = derivation_for(candidate_original)?;
        let candidate_replay_derivation = derivation_for(candidate_replay)?;
        let baseline_original_derivation = derivation_for(baseline_original)?;
        let baseline_replay_derivation = derivation_for(baseline_replay)?;
        if candidate_original_derivation.derivation_material_digest
            != candidate_replay_derivation.derivation_material_digest
            || candidate_original_derivation.facts != candidate_replay_derivation.facts
            || baseline_original_derivation.derivation_material_digest
                != baseline_replay_derivation.derivation_material_digest
            || baseline_original_derivation.facts != baseline_replay_derivation.facts
            || candidate_original_derivation.corpus_semantic_digest
                != baseline_original_derivation.corpus_semantic_digest
        {
            return Err(conflict(
                "M26 candidate/baseline replay or prepared-corpus authority is not exact",
            ));
        }
        self.validate_evidence_artifact_authority_active(
            tenant,
            incarnation,
            &evidence,
            now_millis(),
        )
        .await?;
        Ok(PinnedMaterializationAuthority {
            head,
            decision,
            evidence,
            semantic,
        })
    }
}
