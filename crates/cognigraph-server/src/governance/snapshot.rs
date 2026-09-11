//! Snapshot.

use super::*;

impl PromotionManager {
    pub(super) async fn reject_snapshot_overwrite(
        &self,
        collection: &str,
        key: &str,
        incoming: &Value,
    ) -> Result<(), CogniGraphError> {
        if let Some(existing) = self.backend.get_document(collection, key).await?
            && !same_stored_record(&existing, incoming)?
        {
            return Err(conflict(format!(
                "snapshot would overwrite immutable governance record `{collection}/{key}`"
            )));
        }
        Ok(())
    }

    /// Validate the additive union of destination and incoming M19 authority.
    /// This runs before the backend sees any snapshot bytes.
    pub(crate) async fn preflight_governance_snapshot_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        snapshot: &Value,
        evidence: &BTreeMap<String, PromotionEvidence>,
        decisions: &BTreeMap<String, PromotionDecision>,
    ) -> Result<(), CogniGraphError> {
        let mut authority = self
            .load_governance_authority_locked(tenant, incarnation)
            .await?;

        if let Some(documents) = snapshot_documents(snapshot, GOVERNANCE_KEYS_COLLECTION)? {
            for (key, value) in documents {
                let record: GovernanceKeyRecord = deserialize_stored(value.clone())?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot governance key map key and embedded key differ",
                    ));
                }
                self.reject_snapshot_overwrite(GOVERNANCE_KEYS_COLLECTION, key, value)
                    .await?;
                insert_unique(
                    &mut authority.keys,
                    record.registration_id.clone(),
                    record,
                    "governance key registration",
                )?;
            }
        }
        if let Some(documents) =
            snapshot_documents(snapshot, GOVERNANCE_KEY_REVOCATIONS_COLLECTION)?
        {
            for (key, value) in documents {
                let record: GovernanceKeyRevocation = deserialize_stored(value.clone())?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot key revocation map key and embedded key differ",
                    ));
                }
                self.reject_snapshot_overwrite(GOVERNANCE_KEY_REVOCATIONS_COLLECTION, key, value)
                    .await?;
                insert_unique(
                    &mut authority.revocations,
                    record.registration_id.clone(),
                    record,
                    "governance key revocation",
                )?;
            }
        }
        if let Some(documents) = snapshot_documents(snapshot, POLICY_REVISIONS_COLLECTION)? {
            for (key, value) in documents {
                let record: PolicyRevisionRecord = deserialize_stored(value.clone())?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot policy revision map key and embedded key differ",
                    ));
                }
                self.reject_snapshot_overwrite(POLICY_REVISIONS_COLLECTION, key, value)
                    .await?;
                insert_unique(
                    &mut authority.policies,
                    record.policy_revision_id.clone(),
                    record,
                    "policy revision",
                )?;
            }
        }
        if let Some(documents) = snapshot_documents(snapshot, POLICY_APPROVALS_COLLECTION)? {
            for (key, value) in documents {
                let record: PolicyApprovalRecord = deserialize_stored(value.clone())?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot policy approval map key and embedded key differ",
                    ));
                }
                self.reject_snapshot_overwrite(POLICY_APPROVALS_COLLECTION, key, value)
                    .await?;
                insert_unique(
                    &mut authority.approvals,
                    record.approval_id.clone(),
                    record,
                    "policy approval",
                )?;
            }
        }
        if let Some(documents) = snapshot_documents(snapshot, ARTIFACT_ATTESTATIONS_COLLECTION)? {
            for (key, value) in documents {
                let record: ArtifactAttestationRecord = deserialize_stored(value.clone())?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot artifact attestation map key and embedded key differ",
                    ));
                }
                self.reject_snapshot_overwrite(ARTIFACT_ATTESTATIONS_COLLECTION, key, value)
                    .await?;
                insert_unique(
                    &mut authority.artifacts,
                    record.attestation_id.clone(),
                    record,
                    "artifact attestation",
                )?;
            }
        }
        if let Some(documents) = snapshot_documents(snapshot, SEMANTIC_REPAIR_REVISIONS_COLLECTION)?
        {
            for (key, value) in documents {
                let record: SemanticRepairRevisionRecord =
                    deserialize_semantic_repair_stored(value.clone())?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot semantic repair revision map key and embedded key differ",
                    ));
                }
                self.reject_snapshot_overwrite(SEMANTIC_REPAIR_REVISIONS_COLLECTION, key, value)
                    .await?;
                insert_unique(
                    &mut authority.semantic_repair_revisions,
                    record.semantic_repair_revision_id.clone(),
                    record,
                    "semantic repair revision",
                )?;
            }
        }
        if let Some(documents) = snapshot_documents(snapshot, SEMANTIC_REPAIR_REVIEWS_COLLECTION)? {
            for (key, value) in documents {
                let record: SemanticRepairReviewRecord =
                    deserialize_semantic_repair_stored(value.clone())?;
                if record.key != *key {
                    return Err(conflict(
                        "snapshot semantic repair review map key and embedded key differ",
                    ));
                }
                self.reject_snapshot_overwrite(SEMANTIC_REPAIR_REVIEWS_COLLECTION, key, value)
                    .await?;
                insert_unique(
                    &mut authority.semantic_repair_reviews,
                    record.semantic_repair_review_id.clone(),
                    record,
                    "semantic repair review",
                )?;
            }
        }

        self.validate_governance_authority(tenant, incarnation, &authority)?;
        self.validate_semantic_repair_bases_against_decisions(&authority, decisions, None)?;
        self.validate_promotions_against_authority(evidence, decisions, &authority)
    }
}
