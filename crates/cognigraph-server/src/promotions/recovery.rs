//! Recovery.

use super::*;

impl PromotionManager {
    pub async fn recover_tenant(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<usize, CogniGraphError> {
        self.ensure_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        let evidence = self
            .all_scoped_records::<PromotionEvidence>(EVIDENCE_COLLECTION, tenant, incarnation, "e")
            .await?;
        let decisions = self
            .all_scoped_records::<PromotionDecision>(DECISIONS_COLLECTION, tenant, incarnation, "d")
            .await?;
        let evidence_by_id = evidence
            .into_iter()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let decisions_by_id = decisions
            .iter()
            .cloned()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        if let Err(error) = self.recover_governance_locked(tenant, incarnation).await {
            self.record_error(tenant, error.to_string());
            return Err(error);
        }
        if let Err(error) =
            self.validate_decision_set(tenant, incarnation, &evidence_by_id, &decisions_by_id)
        {
            self.record_error(tenant, error.to_string());
            return Err(error);
        }
        if let Err(error) = self
            .validate_historical_promotion_authority_locked(
                tenant,
                incarnation,
                &evidence_by_id,
                &decisions_by_id,
            )
            .await
        {
            self.record_error(tenant, error.to_string());
            return Err(error);
        }
        if let Err(error) = self
            .validate_stored_evidence_provenance(tenant, incarnation, &evidence_by_id)
            .await
        {
            self.record_error(tenant, error.to_string());
            return Err(error);
        }
        let mut targets = BTreeSet::new();
        for decision in &decisions {
            targets.insert((
                decision.target.space_type.clone(),
                decision.target.channel.clone(),
            ));
        }
        let mut repaired = self
            .remove_invalid_or_orphan_heads_locked(tenant, incarnation, &targets)
            .await?;
        for (space_type, channel) in targets {
            let result = self
                .reconcile_locked(
                    tenant,
                    incarnation,
                    &PromotionTarget {
                        space_type,
                        channel,
                    },
                    false,
                )
                .await?;
            repaired += usize::from(result.head_changed);
        }
        match Box::pin(self.recover_materialized_repairs_locked(tenant, incarnation)).await {
            Ok(materialized_repairs) => repaired = repaired.saturating_add(materialized_repairs),
            Err(error) => {
                self.record_error(tenant, error.to_string());
                return Err(error);
            }
        }
        self.errors
            .lock()
            .expect("promotion health lock")
            .remove(tenant);
        Ok(repaired)
    }
}
