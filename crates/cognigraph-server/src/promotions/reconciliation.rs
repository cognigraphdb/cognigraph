//! Reconciliation.

use super::*;

impl PromotionManager {
    pub(crate) async fn reconcile_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        dry_run: bool,
    ) -> Result<ReconcileResult, CogniGraphError> {
        let decisions = self.target_decisions(tenant, incarnation, target).await?;
        if let Err(error) = self
            .validate_target_authority_locked(tenant, incarnation, target, &decisions)
            .await
        {
            self.record_error(tenant, error.to_string());
            return Err(error);
        }
        let mut actionable = decisions
            .into_iter()
            .filter(|decision| decision.resulting_selection.is_some())
            .collect::<Vec<_>>();
        if actionable.len() > MAX_RECONCILE_DECISIONS {
            return Err(validation(format!(
                "promotion target reconciliation exceeds {MAX_RECONCILE_DECISIONS} actionable decisions"
            )));
        }
        let decisions_scanned = actionable.len();
        for decision in &actionable {
            self.validate_stored_decision(decision, tenant, incarnation)?;
        }
        actionable.sort_by_key(|decision| {
            decision
                .resulting_selection
                .as_ref()
                .map(|selection| selection.generation)
                .unwrap_or_default()
        });
        let mut predecessor: Option<String> = None;
        let mut prior_evidence: Option<String> = None;
        let mut expected_generation = 1u64;
        for decision in &actionable {
            let selection = decision
                .resulting_selection
                .as_ref()
                .expect("actionable decision has selection");
            if selection.generation != expected_generation
                || decision.predecessor_decision_id != predecessor
                || decision.expected_head_decision_id != predecessor
                || selection.prior_evidence_id != prior_evidence
            {
                let error = conflict("promotion decision chain is forked or non-contiguous");
                self.record_error(tenant, error.to_string());
                return Err(error);
            }
            let evidence = self
                .get_evidence_locked(tenant, incarnation, &selection.evidence_id)
                .await?;
            if evidence.target != *target
                || evidence.evidence_digest != selection.evidence_digest
                || evidence.candidate_digest != selection.candidate_digest
                || evidence.policy_digest != selection.policy_digest
                || decision.evidence_id != evidence.id
                || decision.evidence_digest != evidence.evidence_digest
                || decision.policy_digest != evidence.policy_digest
                || decision.gate_assessment_digest != canonical_digest(&evidence.gates)?
            {
                let error = conflict("promotion decision selection does not match its evidence");
                self.record_error(tenant, error.to_string());
                return Err(error);
            }
            predecessor = Some(decision.id.clone());
            prior_evidence = Some(selection.evidence_id.clone());
            expected_generation = expected_generation
                .checked_add(1)
                .ok_or_else(|| conflict("promotion generation is exhausted"))?;
        }
        let expected = actionable
            .last()
            .map(|decision| {
                self.head_from_decision(
                    decision,
                    decision
                        .resulting_selection
                        .as_ref()
                        .expect("actionable decision has selection"),
                )
            })
            .transpose()?;
        let (current, malformed_projection) = self
            .head_for_reconciliation(tenant, incarnation, target)
            .await?;
        let changed = malformed_projection || current != expected;
        if changed && !dry_run {
            match &expected {
                Some(head) => self.save_head(head).await?,
                None => {
                    let key = head_key(tenant, incarnation, target)?;
                    self.backend.delete_document(HEADS_COLLECTION, &key).await?;
                }
            }
            self.metrics.repairs.fetch_add(1, Ordering::Relaxed);
        }
        self.metrics.reconciliations.fetch_add(1, Ordering::Relaxed);
        Ok(ReconcileResult {
            target: target.clone(),
            dry_run,
            decisions_scanned,
            head_changed: changed,
            head: expected,
        })
    }

    pub(super) async fn validate_target_authority_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        decisions: &[PromotionDecision],
    ) -> Result<(), CogniGraphError> {
        let decisions_by_id = decisions
            .iter()
            .cloned()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let mut evidence_by_id = BTreeMap::new();
        for decision in decisions {
            if !evidence_by_id.contains_key(&decision.evidence_id) {
                let evidence = self
                    .get_evidence_locked(tenant, incarnation, &decision.evidence_id)
                    .await?;
                evidence_by_id.insert(evidence.id.clone(), evidence);
            }
        }
        self.validate_decision_set(tenant, incarnation, &evidence_by_id, &decisions_by_id)?;
        self.validate_historical_promotion_authority_for_target_locked(
            tenant,
            incarnation,
            &evidence_by_id,
            &decisions_by_id,
            target,
        )
        .await?;
        self.validate_stored_evidence_provenance(tenant, incarnation, &evidence_by_id)
            .await
    }
}
