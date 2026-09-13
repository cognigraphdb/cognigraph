//! Snapshot import.

use super::*;

impl PromotionManager {
    pub(super) async fn remove_invalid_or_orphan_heads_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        authoritative_targets: &BTreeSet<(String, String)>,
    ) -> Result<usize, CogniGraphError> {
        let prefix = scoped_key(tenant, incarnation, "h", "");
        let fields = promotion_record_fields(HEADS_COLLECTION)?;
        let mut after = prefix.clone();
        let mut removed = 0usize;
        loop {
            let rows = self
                .backend
                .list_documents_after_key(HEADS_COLLECTION, Some(&after), &fields, 256)
                .await?;
            if rows.is_empty() {
                break;
            }
            let mut advanced = false;
            for value in rows {
                let key = value
                    .get("_key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| conflict("promotion head projection is missing `_key`"))?
                    .to_string();
                if !key.starts_with(&prefix) {
                    return Ok(removed);
                }
                after.clone_from(&key);
                advanced = true;
                let valid = serde_json::from_value::<PromotionHead>(value)
                    .ok()
                    .filter(|head| {
                        authoritative_targets.contains(&(
                            head.target.space_type.clone(),
                            head.target.channel.clone(),
                        ))
                    })
                    .map(|head| {
                        valid_head(&head, tenant, incarnation, &head.target, &key).unwrap_or(false)
                    })
                    .unwrap_or(false);
                if !valid && self.backend.delete_document(HEADS_COLLECTION, &key).await? {
                    removed = removed.saturating_add(1);
                    self.metrics.repairs.fetch_add(1, Ordering::Relaxed);
                }
            }
            if !advanced {
                break;
            }
        }
        Ok(removed)
    }

    /// Preflight immutable M18 records, remove untrusted imported head
    /// projections, perform the additive backend import while promotion writes
    /// are fenced, and rebuild heads from the authoritative decision chain.
    pub async fn import_snapshot(
        &self,
        tenant: &str,
        incarnation: &str,
        snapshot: &Value,
    ) -> Result<(), CogniGraphError> {
        self.ensure_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.preflight_snapshot_locked(tenant, incarnation, snapshot)
            .await?;
        self.preflight_materialized_repair_snapshot_locked(tenant, incarnation, snapshot)
            .await?;
        let mut sanitized = snapshot.clone();
        if let Some(collections) = sanitized
            .get_mut("collections")
            .and_then(Value::as_object_mut)
        {
            collections.remove(HEADS_COLLECTION);
            collections
                .remove(crate::materialized_repairs::SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION);
        }
        if let Err(error) = self.backend.import_snapshot(&sanitized).await {
            self.record_error(tenant, error.to_string());
            return Err(error);
        }

        let post_import: Result<(), CogniGraphError> = async {
            let evidence = self
                .all_scoped_records::<PromotionEvidence>(
                    EVIDENCE_COLLECTION,
                    tenant,
                    incarnation,
                    "e",
                )
                .await?;
            let decisions = self
                .all_scoped_records::<PromotionDecision>(
                    DECISIONS_COLLECTION,
                    tenant,
                    incarnation,
                    "d",
                )
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
            self.validate_decision_set(tenant, incarnation, &evidence_by_id, &decisions_by_id)?;
            self.recover_governance_locked(tenant, incarnation).await?;
            self.validate_historical_promotion_authority_locked(
                tenant,
                incarnation,
                &evidence_by_id,
                &decisions_by_id,
            )
            .await?;
            self.validate_stored_evidence_provenance(tenant, incarnation, &evidence_by_id)
                .await?;
            let mut targets = BTreeSet::new();
            for decision in &decisions {
                targets.insert((
                    decision.target.space_type.clone(),
                    decision.target.channel.clone(),
                ));
            }
            self.remove_invalid_or_orphan_heads_locked(tenant, incarnation, &targets)
                .await?;
            for (space_type, channel) in targets {
                self.reconcile_locked(
                    tenant,
                    incarnation,
                    &PromotionTarget {
                        space_type,
                        channel,
                    },
                    false,
                )
                .await?;
            }
            Box::pin(self.recover_materialized_repairs_locked(tenant, incarnation)).await?;
            Ok(())
        }
        .await;
        if let Err(error) = &post_import {
            self.record_error(tenant, error.to_string());
        }
        post_import
    }
}
