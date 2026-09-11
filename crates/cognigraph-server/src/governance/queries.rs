//! Queries.

use super::*;

impl PromotionManager {
    pub async fn list_governance_keys(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<GovernanceKeyRecord>, CogniGraphError> {
        let page = self
            .list_governance_records(
                GOVERNANCE_KEYS_COLLECTION,
                tenant,
                incarnation,
                "gk",
                limit,
                cursor,
            )
            .await?;
        for record in &page.records {
            self.validate_stored_key_record(record, tenant, incarnation)?;
        }
        Ok(page)
    }

    pub async fn get_governance_key(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<GovernanceKeyRecord, CogniGraphError> {
        self.ensure_governance_repository(tenant).await?;
        self.get_key_locked(tenant, incarnation, id).await
    }

    pub async fn list_governance_revocations(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<GovernanceKeyRevocation>, CogniGraphError> {
        let page = self
            .list_governance_records(
                GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
                tenant,
                incarnation,
                "gkr",
                limit,
                cursor,
            )
            .await?;
        for record in &page.records {
            self.validate_stored_revocation_record(record, tenant, incarnation)?;
        }
        Ok(page)
    }

    pub async fn get_governance_revocation(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<GovernanceKeyRevocation, CogniGraphError> {
        self.ensure_governance_repository(tenant).await?;
        let key = scoped_key(tenant, incarnation, "gkr", id);
        let record = self
            .get_authority_raw::<GovernanceKeyRevocation>(
                tenant,
                GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
                &key,
            )
            .await?
            .ok_or_else(|| not_found("governance key revocation", id))?;
        self.validate_stored_revocation_record(&record, tenant, incarnation)?;
        Ok(record)
    }

    pub async fn list_policy_revisions(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<PolicyRevisionRecord>, CogniGraphError> {
        let page = self
            .list_governance_records(
                POLICY_REVISIONS_COLLECTION,
                tenant,
                incarnation,
                "pr",
                limit,
                cursor,
            )
            .await?;
        for record in &page.records {
            self.validate_stored_policy_record(record, tenant, incarnation)
                .await?;
        }
        Ok(page)
    }

    pub async fn get_policy_revision(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PolicyRevisionRecord, CogniGraphError> {
        self.ensure_governance_repository(tenant).await?;
        let record = self.get_policy_locked(tenant, incarnation, id).await?;
        Ok(record)
    }

    pub async fn get_policy_approval(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PolicyApprovalRecord, CogniGraphError> {
        self.ensure_governance_repository(tenant).await?;
        self.get_approval_locked(tenant, incarnation, id).await
    }

    pub(crate) async fn get_key_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<GovernanceKeyRecord, CogniGraphError> {
        let key = scoped_key(tenant, incarnation, "gk", id);
        let record = self
            .get_authority_raw::<GovernanceKeyRecord>(tenant, GOVERNANCE_KEYS_COLLECTION, &key)
            .await?
            .ok_or_else(|| not_found("governance key", id))?;
        self.validate_stored_key_record(&record, tenant, incarnation)?;
        Ok(record)
    }

    pub(crate) async fn get_active_key_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
        purpose: KeyPurpose,
        at_ms: u64,
    ) -> Result<GovernanceKeyRecord, CogniGraphError> {
        let record = self.get_key_locked(tenant, incarnation, id).await?;
        if record.verification_key.purpose != purpose
            || record.registered_at_ms > at_ms
            || at_ms < record.not_before_ms
            || record.not_after_ms.is_some_and(|until| at_ms >= until)
        {
            return Err(CogniGraphError::Forbidden(
                "governance key is inactive or has the wrong purpose".into(),
            ));
        }
        let revocation_id = key_revocation_id(tenant, incarnation, id);
        let revocation_key = scoped_key(tenant, incarnation, "gkr", &revocation_id);
        if let Some(revocation) = self
            .get_authority_raw::<GovernanceKeyRevocation>(
                tenant,
                GOVERNANCE_KEY_REVOCATIONS_COLLECTION,
                &revocation_key,
            )
            .await?
        {
            self.validate_stored_revocation_record(&revocation, tenant, incarnation)?;
            if revocation.recorded_at_ms <= at_ms && revocation.effective_at_ms <= at_ms {
                return Err(CogniGraphError::Forbidden(
                    "governance key is revoked".into(),
                ));
            }
        }
        Ok(record)
    }

    pub(super) async fn get_policy_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PolicyRevisionRecord, CogniGraphError> {
        let key = scoped_key(tenant, incarnation, "pr", id);
        let record = self
            .get_authority_raw::<PolicyRevisionRecord>(tenant, POLICY_REVISIONS_COLLECTION, &key)
            .await?
            .ok_or_else(|| not_found("policy revision", id))?;
        self.validate_stored_policy_record(&record, tenant, incarnation)
            .await?;
        Ok(record)
    }

    pub(super) async fn get_approval_locked(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PolicyApprovalRecord, CogniGraphError> {
        let key = scoped_key(tenant, incarnation, "pa", id);
        let record = self
            .get_authority_raw::<PolicyApprovalRecord>(tenant, POLICY_APPROVALS_COLLECTION, &key)
            .await?
            .ok_or_else(|| not_found("policy approval", id))?;
        self.validate_stored_approval_record(&record, tenant, incarnation)
            .await?;
        Ok(record)
    }
}
