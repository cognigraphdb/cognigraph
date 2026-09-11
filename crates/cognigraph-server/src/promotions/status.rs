//! Status.

use super::*;

impl PromotionManager {
    pub async fn operator_status(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<Value, CogniGraphError> {
        self.ensure_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        let evidence = self
            .all_scoped_records::<PromotionEvidence>(EVIDENCE_COLLECTION, tenant, incarnation, "e")
            .await?;
        let decisions = self
            .all_scoped_records::<PromotionDecision>(DECISIONS_COLLECTION, tenant, incarnation, "d")
            .await?;
        let heads_result = self
            .all_scoped_records::<PromotionHead>(HEADS_COLLECTION, tenant, incarnation, "h")
            .await;
        if let Err(error) = &heads_result {
            self.record_error(tenant, error.to_string());
        }
        let heads = heads_result?;
        let evidence_by_id = evidence
            .iter()
            .cloned()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let decisions_by_id = decisions
            .iter()
            .cloned()
            .map(|record| (record.id.clone(), record))
            .collect::<BTreeMap<_, _>>();
        let mut integrity_error = self
            .validate_decision_set(tenant, incarnation, &evidence_by_id, &decisions_by_id)
            .err()
            .map(|error| error.to_string());
        if integrity_error.is_none()
            && let Err(error) = self.recover_governance_locked(tenant, incarnation).await
        {
            integrity_error = Some(error.to_string());
        }
        let (artifact_attestations, artifact_attestations_by_kind) = match self
            .artifact_attestation_counts_locked(tenant, incarnation)
            .await
        {
            Ok(counts) => counts,
            Err(error) => {
                if integrity_error.is_none() {
                    integrity_error = Some(error.to_string());
                }
                (
                    0,
                    BTreeMap::from([
                        ("corpus", 0),
                        ("graph", 0),
                        ("oracle", 0),
                        ("scorer", 0),
                        ("verifier", 0),
                    ]),
                )
            }
        };
        if integrity_error.is_none()
            && let Err(error) = self
                .validate_historical_promotion_authority_locked(
                    tenant,
                    incarnation,
                    &evidence_by_id,
                    &decisions_by_id,
                )
                .await
        {
            integrity_error = Some(error.to_string());
        }
        if integrity_error.is_none()
            && let Err(error) = self
                .validate_stored_evidence_provenance(tenant, incarnation, &evidence_by_id)
                .await
        {
            integrity_error = Some(error.to_string());
        }
        let mut targets = decisions
            .iter()
            .map(|decision| {
                (
                    decision.target.space_type.clone(),
                    decision.target.channel.clone(),
                )
            })
            .collect::<BTreeSet<_>>();
        for head in &heads {
            targets.insert((head.target.space_type.clone(), head.target.channel.clone()));
            if integrity_error.is_none()
                && !valid_head(head, tenant, incarnation, &head.target, &head.key)?
            {
                integrity_error = Some("promotion head projection is malformed".into());
            }
        }
        let mut repair_required = false;
        if integrity_error.is_none() {
            for (space_type, channel) in targets {
                match self
                    .reconcile_locked(
                        tenant,
                        incarnation,
                        &PromotionTarget {
                            space_type,
                            channel,
                        },
                        true,
                    )
                    .await
                {
                    Ok(result) => repair_required |= result.head_changed,
                    Err(error) => {
                        integrity_error = Some(error.to_string());
                        break;
                    }
                }
            }
        }
        let mut materialized_repair_status = json!({
            "enabled": false,
            "generations": 0,
            "deployment_decisions": 0,
            "active_heads": 0,
            "repair_required": false,
            "full_validation_performed": false,
            "native_atomic_only": true,
        });
        if integrity_error.is_none() {
            match Box::pin(self.materialized_repair_status_locked(tenant, incarnation)).await {
                Ok(status) => {
                    repair_required |= status.repair_required;
                    materialized_repair_status = serde_json::to_value(status)?;
                }
                Err(error) => integrity_error = Some(error.to_string()),
            }
        }
        if repair_required && integrity_error.is_none() {
            integrity_error = Some(
                "derived promotion or M26 deployment state differs from immutable authority".into(),
            );
        }
        if let Some(error) = &integrity_error {
            self.record_error(tenant, error.clone());
        }
        let health_error = self
            .errors
            .lock()
            .expect("promotion health lock")
            .get(tenant)
            .cloned();
        Ok(json!({
            "tenant": tenant,
            "tenant_incarnation": incarnation,
            "healthy": health_error.is_none(),
            "health_error": health_error,
            "evidence_bundles": evidence.len(),
            "m20_evidence_bundles": evidence
                .iter()
                .filter(|record| record.schema_version == M20_PROMOTION_EVIDENCE_SCHEMA_VERSION)
                .count(),
            "m21_evidence_bundles": evidence
                .iter()
                .filter(|record| record.schema_version == M21_PROMOTION_EVIDENCE_SCHEMA_VERSION)
                .count(),
            "m22_evidence_bundles": evidence
                .iter()
                .filter(|record| record.schema_version == M22_PROMOTION_EVIDENCE_SCHEMA_VERSION)
                .count(),
            "m23_evidence_bundles": evidence
                .iter()
                .filter(|record| record.schema_version == M23_PROMOTION_EVIDENCE_SCHEMA_VERSION)
                .count(),
            "decisions": decisions.len(),
            "active_heads": heads.len(),
            "artifact_attestations": artifact_attestations,
            "artifact_attestations_by_kind": artifact_attestations_by_kind,
            "verified_semantic_repair": materialized_repair_status,
            "full_validation_performed": true,
            "repair_required": repair_required,
            "singleton_writer": true,
            "high_availability": false,
        }))
    }
}
