//! Recovery.

use super::*;

impl PromotionManager {
    pub(crate) async fn recover_materialized_repairs_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<usize, CogniGraphError> {
        if !self.backend.supports_atomic_batches() {
            self.require_non_atomic_materialized_repair_repository_absent()
                .await?;
            return Ok(0);
        }
        if !self.materialized_repair_repository_present(tenant).await? {
            return Ok(0);
        }
        require_native_atomic_backend(self)?;
        let generations = self
            .validated_generation_set_locked(tenant, incarnation)
            .await?;
        let chains = self
            .validated_deployment_chains_locked(tenant, incarnation)
            .await?;
        if !chains.is_empty() {
            self.ensure_materialization_target_collections(tenant)
                .await?;
        }
        let heads = self
            .scoped_deployment_head_values(tenant, incarnation)
            .await?;

        let mut expected_head_keys = BTreeSet::new();
        let mut planned_entity_inserts = BTreeSet::new();
        let mut repairs = Vec::<Vec<BatchOp>>::new();
        for (space_type, chain) in &chains {
            let decision = chain
                .last()
                .expect("M26 validated deployment chain is non-empty");
            let expected_head = self.deployment_head_from_decision(decision)?;
            expected_head_keys.insert(expected_head.key.clone());
            let generation = generations
                .get(&decision.semantic_repair_generation_id)
                .ok_or_else(|| conflict("M26 deployment references a missing generation"))?;
            let target_matches = self
                .active_target_matches_projection(space_type, &generation.projection)
                .await?;
            let stored_head = heads.get(&expected_head.key).and_then(|value| {
                serde_json::from_value::<SemanticRepairDeploymentHead>(value.clone()).ok()
            });
            let head_matches = stored_head.as_ref() == Some(&expected_head);
            if target_matches && head_matches {
                continue;
            }
            let mut ops = if target_matches {
                Vec::new()
            } else {
                self.target_replacement_ops(space_type, &generation.projection)
                    .await?
            };
            ops.retain(|op| match op {
                BatchOp::Insert { collection, doc } if collection == "entities" => row_key(doc)
                    .map(|key| planned_entity_inserts.insert(key.to_string()))
                    .unwrap_or(true),
                _ => true,
            });
            ops.push(if heads.contains_key(&expected_head.key) {
                BatchOp::Replace {
                    collection: SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION.into(),
                    key: expected_head.key.clone(),
                    doc: serde_json::to_value(&expected_head)?,
                }
            } else {
                BatchOp::Insert {
                    collection: SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION.into(),
                    doc: serde_json::to_value(&expected_head)?,
                }
            });
            repairs.push(ops);
        }
        for key in heads
            .keys()
            .filter(|key| !expected_head_keys.contains(*key))
        {
            repairs.push(vec![BatchOp::Delete {
                collection: SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION.into(),
                key: key.clone(),
            }]);
        }
        let repaired = repairs.len();
        for ops in repairs {
            self.backend.execute_batch(ops).await?;
        }
        self.record_m26_reconciliation(repaired);
        Ok(repaired)
    }

    pub(super) async fn deployment_decisions_for_space_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        space_type: &str,
    ) -> Result<Vec<SemanticRepairDeploymentDecision>, CogniGraphError> {
        // Decision ids are content hashes rather than space-prefixed keys, so
        // a per-space quota cannot be established from the first page of the
        // tenant key range. Apply the complete scope filter before the bound.
        let predicates = [
            FieldPredicate {
                path: vec!["tenant".into()],
                op: PredicateOp::Eq,
                value: json!(tenant),
            },
            FieldPredicate {
                path: vec!["tenant_incarnation".into()],
                op: PredicateOp::Eq,
                value: json!(incarnation),
            },
            FieldPredicate {
                path: vec!["space_type".into()],
                op: PredicateOp::Eq,
                value: json!(space_type),
            },
        ];
        let mut records = Vec::new();
        for mut value in self
            .backend
            .list_documents_filtered(
                SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
                &predicates,
                None,
                Some(MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE + 1),
                None,
            )
            .await?
        {
            strip_backend_metadata(&mut value);
            records.push(serde_json::from_value::<SemanticRepairDeploymentDecision>(
                value,
            )?);
        }
        if records.len() > MAX_M26_DEPLOYMENT_DECISIONS_PER_SPACE {
            return Err(conflict(
                "M26 deployment decision quota is already exceeded",
            ));
        }
        Ok(records)
    }
}
