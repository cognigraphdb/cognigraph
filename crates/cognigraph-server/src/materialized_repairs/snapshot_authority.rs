//! Snapshot authority.

use super::*;

impl PromotionManager {
    pub(super) async fn resolve_snapshot_semantic_repair_authority(
        &self,
        tenant: &str,
        incarnation: &str,
        snapshot: &Value,
        head: &PromotionHead,
        applied: &PromotionDecision,
        evidence: &PromotionEvidence,
    ) -> Result<ResolvedSemanticRepairAuthority, CogniGraphError> {
        if head.tenant != tenant
            || head.tenant_incarnation != incarnation
            || applied.id != head.applied_decision_id
            || applied.target != head.target
            || applied.resulting_selection.as_ref() != Some(&head.selection)
            || evidence.id != applied.evidence_id
            || evidence.evidence_digest != applied.evidence_digest
            || evidence.target != head.target
            || evidence.candidate_digest != head.selection.candidate_digest
        {
            return Err(conflict(
                "snapshot M26 generation does not resolve to its exact promotion authority",
            ));
        }
        let applied_promoter = snapshot_promotion_principal(applied)?.to_string();
        let selected_candidate = source_by_role(evidence, EvidenceRunRole::CandidateOriginal)?
            .context
            .candidate
            .clone();
        if selected_candidate.candidate_digest != head.selection.candidate_digest {
            return Err(conflict(
                "snapshot M26 promotion evidence has another candidate identity",
            ));
        }

        let mut compatible_bases = Vec::<(Option<String>, String)>::new();
        match applied.action {
            PromotionAction::Promote => compatible_bases.push((
                applied.predecessor_decision_id.clone(),
                applied_promoter.clone(),
            )),
            PromotionAction::Rollback => {
                let mut cursor = applied.predecessor_decision_id.clone();
                let mut seen = BTreeSet::new();
                while let Some(decision_id) = cursor {
                    if !seen.insert(decision_id.clone()) || seen.len() > MAX_RECONCILE_DECISIONS {
                        return Err(conflict(
                            "snapshot M26 promotion predecessor chain is cyclic or over capacity",
                        ));
                    }
                    let key = scoped_key(tenant, incarnation, "d", &decision_id);
                    let decision: PromotionDecision =
                        snapshot_or_stored(self, snapshot, DECISIONS_COLLECTION, &key)
                            .await?
                            .ok_or_else(|| {
                                conflict("snapshot M26 promotion predecessor is missing")
                            })?;
                    if decision.id != decision_id
                        || decision.tenant != tenant
                        || decision.tenant_incarnation != incarnation
                        || decision.target != head.target
                    {
                        return Err(conflict(
                            "snapshot M26 promotion predecessor has another identity",
                        ));
                    }
                    if decision.action == PromotionAction::Promote
                        && decision
                            .resulting_selection
                            .as_ref()
                            .is_some_and(|selection| {
                                same_snapshot_selected_authority(selection, &head.selection)
                            })
                    {
                        compatible_bases.push((
                            decision.predecessor_decision_id.clone(),
                            snapshot_promotion_principal(&decision)?.to_string(),
                        ));
                    }
                    cursor = decision.predecessor_decision_id.clone();
                }
            }
            PromotionAction::Reject | PromotionAction::Blocked => {}
        }
        if compatible_bases.is_empty() {
            return Err(conflict(
                "snapshot M26 promotion has no compatible Semantic Repair base",
            ));
        }

        let mut approved = Vec::new();
        for (base_decision_id, base_promoter) in compatible_bases {
            let revision_id = semantic_repair_revision_id(
                tenant,
                incarnation,
                &head.target,
                base_decision_id.as_deref(),
                &head.selection.candidate_digest,
            )?;
            let revision_key = scoped_key(tenant, incarnation, "srr", &revision_id);
            let Some(revision): Option<SemanticRepairRevisionRecord> = snapshot_or_stored(
                self,
                snapshot,
                SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                &revision_key,
            )
            .await?
            else {
                continue;
            };
            if revision.semantic_repair_revision_id != revision_id
                || revision.target != head.target
                || revision.base_promotion_head_decision_id != base_decision_id
                || revision.candidate_digest != head.selection.candidate_digest
                || revision.candidate.kind != selected_candidate.kind
                || revision.candidate.id != selected_candidate.id
                || revision.candidate.revision != selected_candidate.revision
            {
                return Err(conflict(
                    "snapshot M26 Semantic Repair natural identity is incompatible",
                ));
            }
            let review_id = semantic_repair_review_id(
                tenant,
                incarnation,
                &revision.semantic_repair_revision_id,
            );
            let review_key = scoped_key(tenant, incarnation, "srrv", &review_id);
            let Some(review): Option<SemanticRepairReviewRecord> = snapshot_or_stored(
                self,
                snapshot,
                SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                &review_key,
            )
            .await?
            else {
                continue;
            };
            if review.semantic_repair_review_id != review_id
                || review.semantic_repair_revision_id != revision.semantic_repair_revision_id
                || review.semantic_repair_revision_digest
                    != revision.semantic_repair_revision_digest
                || review.target != revision.target
                || review.base_promotion_head_decision_id
                    != revision.base_promotion_head_decision_id
                || review.candidate_digest != revision.candidate_digest
                || review.author_principal_id != revision.author_principal_id
            {
                return Err(conflict(
                    "snapshot M26 Semantic Repair review is incompatible",
                ));
            }
            if review.decision != SemanticRepairReviewDecision::Approve {
                continue;
            }
            validate_snapshot_resolution_separation(
                &applied_promoter,
                &revision.author_principal_id,
                &review.approver_principal_id,
            )?;
            validate_snapshot_resolution_separation(
                &base_promoter,
                &revision.author_principal_id,
                &review.approver_principal_id,
            )?;
            approved.push(ResolvedSemanticRepairAuthority { revision, review });
        }
        if approved.len() != 1 {
            return Err(conflict(format!(
                "snapshot M26 resolution requires exactly one approved compatible revision, found {}",
                approved.len()
            )));
        }
        Ok(approved.remove(0))
    }
}
