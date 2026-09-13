//! Authority loading.

use super::*;

impl PromotionManager {
    pub(crate) async fn recover_governance_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let authority = self
            .load_governance_authority_locked(tenant, incarnation)
            .await?;
        self.validate_governance_authority(tenant, incarnation, &authority)
    }

    pub(super) async fn load_governance_authority_locked(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<GovernanceAuthority, CogniGraphError> {
        self.ensure_governance_repository(tenant).await?;
        // Recovery also runs for tenants created before M25. Lazily create the
        // append-only repair collections before their first keyset scan so an
        // otherwise healthy pre-M25 store remains startup-compatible.
        self.ensure_semantic_repair_repository(tenant).await?;
        let keys = self
            .scoped_records::<GovernanceKeyRecord>(
                GOVERNANCE_KEYS_COLLECTION,
                tenant,
                incarnation,
                "gk",
                MAX_KEYS_PER_TENANT + 1,
            )
            .await?;
        let revocations = self
            .scoped_records::<GovernanceKeyRevocation>(
                GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
                tenant,
                incarnation,
                "gkr",
                MAX_KEYS_PER_TENANT + 1,
            )
            .await?;
        let policies = self
            .scoped_records::<PolicyRevisionRecord>(
                POLICY_REVISIONS_COLLECTION,
                tenant,
                incarnation,
                "pr",
                MAX_POLICY_REVISIONS_PER_TENANT + 1,
            )
            .await?;
        let approvals = self
            .scoped_records::<PolicyApprovalRecord>(
                POLICY_APPROVALS_COLLECTION,
                tenant,
                incarnation,
                "pa",
                MAX_POLICY_REVISIONS_PER_TENANT + 1,
            )
            .await?;
        let artifacts = self
            .scoped_records::<ArtifactAttestationRecord>(
                ARTIFACT_ATTESTATIONS_COLLECTION,
                tenant,
                incarnation,
                "aa",
                MAX_ARTIFACT_ATTESTATIONS_PER_TENANT + 1,
            )
            .await?;
        let semantic_repair_revisions = self
            .scoped_records::<SemanticRepairRevisionRecord>(
                SEMANTIC_REPAIR_REVISIONS_COLLECTION,
                tenant,
                incarnation,
                "srr",
                MAX_SEMANTIC_REPAIR_REVISIONS_PER_TENANT + 1,
            )
            .await?;
        let semantic_repair_reviews = self
            .scoped_records::<SemanticRepairReviewRecord>(
                SEMANTIC_REPAIR_REVIEWS_COLLECTION,
                tenant,
                incarnation,
                "srrv",
                MAX_SEMANTIC_REPAIR_REVIEWS_PER_TENANT + 1,
            )
            .await?;
        let mut authority = GovernanceAuthority::default();
        for record in keys {
            insert_unique(
                &mut authority.keys,
                record.registration_id.clone(),
                record,
                "governance key registration",
            )?;
        }
        for record in revocations {
            insert_unique(
                &mut authority.revocations,
                record.registration_id.clone(),
                record,
                "governance key revocation",
            )?;
        }
        for record in policies {
            insert_unique(
                &mut authority.policies,
                record.policy_revision_id.clone(),
                record,
                "policy revision",
            )?;
        }
        for record in approvals {
            insert_unique(
                &mut authority.approvals,
                record.approval_id.clone(),
                record,
                "policy approval",
            )?;
        }
        for record in artifacts {
            insert_unique(
                &mut authority.artifacts,
                record.attestation_id.clone(),
                record,
                "artifact attestation",
            )?;
        }
        for record in semantic_repair_revisions {
            insert_unique(
                &mut authority.semantic_repair_revisions,
                record.semantic_repair_revision_id.clone(),
                record,
                "semantic repair revision",
            )?;
        }
        for record in semantic_repair_reviews {
            insert_unique(
                &mut authority.semantic_repair_reviews,
                record.semantic_repair_review_id.clone(),
                record,
                "semantic repair review",
            )?;
        }
        Ok(authority)
    }
}
