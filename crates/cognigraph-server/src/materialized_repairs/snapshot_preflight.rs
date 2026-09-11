//! Snapshot preflight.

use super::*;

impl PromotionManager {
    pub(crate) async fn preflight_materialized_repair_snapshot_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        snapshot: &Value,
    ) -> Result<(), CogniGraphError> {
        validate_snapshot_declared_collection_type(
            snapshot,
            SEMANTIC_REPAIR_GENERATIONS_COLLECTION,
            CollectionType::Document,
        )?;
        validate_snapshot_declared_collection_type(
            snapshot,
            SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION,
            CollectionType::Document,
        )?;
        validate_snapshot_declared_collection_type(
            snapshot,
            SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION,
            CollectionType::Document,
        )?;
        let incoming_generations =
            snapshot_documents(snapshot, SEMANTIC_REPAIR_GENERATIONS_COLLECTION)?;
        let incoming_decisions =
            snapshot_documents(snapshot, SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION)?;
        let incoming_heads =
            snapshot_documents(snapshot, SEMANTIC_REPAIR_DEPLOYMENT_HEADS_COLLECTION)?;
        if !self.backend.supports_atomic_batches() {
            if incoming_generations.is_none()
                && incoming_decisions.is_none()
                && incoming_heads.is_none()
            {
                self.require_non_atomic_materialized_repair_repository_absent()
                    .await?;
                return Ok(());
            }
            return require_native_atomic_backend(self);
        }
        let repository_present = self.materialized_repair_repository_present(tenant).await?;
        if incoming_generations.is_none() && incoming_decisions.is_none() && !repository_present {
            return Ok(());
        }
        require_native_atomic_backend(self)?;
        let mut generations = BTreeMap::<String, SemanticRepairGenerationRecord>::new();
        if repository_present {
            for record in self
                .all_semantic_repair_generations_locked(tenant, incarnation)
                .await?
            {
                self.validate_stored_semantic_repair_generation(&record, tenant, incarnation)
                    .await?;
                generations.insert(record.semantic_repair_generation_id.clone(), record);
            }
        }
        if let Some(documents) = incoming_generations {
            for (key, value) in documents {
                let mut clean = value.clone();
                strip_backend_metadata(&mut clean);
                let record: SemanticRepairGenerationRecord = serde_json::from_value(clean)?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot M26 generation map key and embedded key differ",
                    ));
                }
                if let Some(existing) = self
                    .backend
                    .get_document(SEMANTIC_REPAIR_GENERATIONS_COLLECTION, key)
                    .await?
                    && !same_stored_record(&existing, value)?
                {
                    return Err(conflict(format!(
                        "snapshot would overwrite immutable M26 generation `{key}`"
                    )));
                }
                let decision_key =
                    scoped_key(tenant, incarnation, "d", &record.promotion_head_decision_id);
                let decision: PromotionDecision =
                    snapshot_or_stored(self, snapshot, DECISIONS_COLLECTION, &decision_key)
                        .await?
                        .ok_or_else(|| conflict("snapshot M26 generation promotion is missing"))?;
                let evidence_key = scoped_key(tenant, incarnation, "e", &record.source_evidence_id);
                let evidence: PromotionEvidence =
                    snapshot_or_stored(self, snapshot, EVIDENCE_COLLECTION, &evidence_key)
                        .await?
                        .ok_or_else(|| conflict("snapshot M26 generation evidence is missing"))?;
                let selection = decision
                    .resulting_selection
                    .as_ref()
                    .ok_or_else(|| conflict("snapshot M26 source promotion has no selection"))?;
                let head = self.head_from_decision(&decision, selection)?;
                let semantic = self
                    .resolve_snapshot_semantic_repair_authority(
                        tenant,
                        incarnation,
                        snapshot,
                        &head,
                        &decision,
                        &evidence,
                    )
                    .await?;
                self.validate_semantic_repair_generation_against(
                    &record,
                    tenant,
                    incarnation,
                    &PinnedMaterializationAuthority {
                        head,
                        decision,
                        evidence,
                        semantic,
                    },
                )?;
                if let Some(existing) =
                    generations.insert(record.semantic_repair_generation_id.clone(), record.clone())
                    && existing != record
                {
                    return Err(conflict(
                        "snapshot contains a divergent duplicate M26 generation id",
                    ));
                }
            }
        }
        validate_generation_capacity_set(&generations)?;

        let mut decisions = BTreeMap::<String, SemanticRepairDeploymentDecision>::new();
        if repository_present {
            for record in self
                .all_deployment_decisions_locked(tenant, incarnation)
                .await?
            {
                self.validate_stored_deployment_decision(&record, tenant, incarnation)
                    .await?;
                decisions.insert(record.deployment_decision_id.clone(), record);
            }
        }
        if let Some(documents) = incoming_decisions {
            for (key, value) in documents {
                let mut clean = value.clone();
                strip_backend_metadata(&mut clean);
                let record: SemanticRepairDeploymentDecision = serde_json::from_value(clean)?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot M26 deployment map key and embedded key differ",
                    ));
                }
                if let Some(existing) = self
                    .backend
                    .get_document(SEMANTIC_REPAIR_DEPLOYMENT_DECISIONS_COLLECTION, key)
                    .await?
                    && !same_stored_record(&existing, value)?
                {
                    return Err(conflict(format!(
                        "snapshot would overwrite immutable M26 deployment `{key}`"
                    )));
                }
                let generation = generations
                    .get(&record.semantic_repair_generation_id)
                    .ok_or_else(|| conflict("snapshot M26 deployment generation is missing"))?;
                let promotion_key =
                    scoped_key(tenant, incarnation, "d", &record.promotion_head_decision_id);
                let promotion: PromotionDecision =
                    snapshot_or_stored(self, snapshot, DECISIONS_COLLECTION, &promotion_key)
                        .await?
                        .ok_or_else(|| conflict("snapshot M26 deployment promotion is missing"))?;
                let promotion_selection =
                    promotion.resulting_selection.as_ref().ok_or_else(|| {
                        conflict("snapshot M26 deployment promotion has no selection")
                    })?;
                let promotion_head = self.head_from_decision(&promotion, promotion_selection)?;
                let evidence_key =
                    scoped_key(tenant, incarnation, "e", &generation.source_evidence_id);
                let evidence: PromotionEvidence =
                    snapshot_or_stored(self, snapshot, EVIDENCE_COLLECTION, &evidence_key)
                        .await?
                        .ok_or_else(|| conflict("snapshot M26 deployment evidence is missing"))?;
                let revision_key = scoped_key(
                    tenant,
                    incarnation,
                    "srr",
                    &generation.semantic_repair_revision_id,
                );
                let revision: SemanticRepairRevisionRecord = snapshot_or_stored(
                    self,
                    snapshot,
                    SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                    &revision_key,
                )
                .await?
                .ok_or_else(|| conflict("snapshot M26 deployment revision is missing"))?;
                let review_key = scoped_key(
                    tenant,
                    incarnation,
                    "srrv",
                    &generation.semantic_repair_review_id,
                );
                let review: SemanticRepairReviewRecord = snapshot_or_stored(
                    self,
                    snapshot,
                    SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                    &review_key,
                )
                .await?
                .ok_or_else(|| conflict("snapshot M26 deployment review is missing"))?;
                let payload = &record.deployment_intent.statement.payload;
                let key_key =
                    scoped_key(tenant, incarnation, "gk", &payload.promoter_registration_id);
                let promoter: GovernanceKeyRecord =
                    snapshot_or_stored(self, snapshot, GOVERNANCE_KEYS_COLLECTION, &key_key)
                        .await?
                        .ok_or_else(|| conflict("snapshot M26 deployment key is missing"))?;
                let revocation_id =
                    key_revocation_id(tenant, incarnation, &payload.promoter_registration_id);
                let revocation_key = scoped_key(tenant, incarnation, "gkr", &revocation_id);
                let revocation: Option<GovernanceKeyRevocation> = snapshot_or_stored(
                    self,
                    snapshot,
                    GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
                    &revocation_key,
                )
                .await?;
                validate_deployment_decision_against(
                    &record,
                    tenant,
                    incarnation,
                    generation,
                    &promotion,
                    &promotion_head.projection_digest,
                    &evidence,
                    &revision,
                    &review,
                    &promoter,
                    revocation.as_ref(),
                )?;
                if let Some(existing) =
                    decisions.insert(record.deployment_decision_id.clone(), record.clone())
                    && existing != record
                {
                    return Err(conflict(
                        "snapshot contains a divergent duplicate M26 deployment id",
                    ));
                }
            }
        }
        let chains = validate_deployment_chain_set(decisions.into_values())?;
        for (space_type, chain) in chains {
            let decision = chain
                .last()
                .expect("validated snapshot M26 chain is non-empty");
            let generation = generations
                .get(&decision.semantic_repair_generation_id)
                .ok_or_else(|| conflict("snapshot M26 head generation is missing"))?;
            self.validate_snapshot_target_projection(snapshot, &space_type, &generation.projection)
                .await?;
        }
        Ok(())
    }
}
