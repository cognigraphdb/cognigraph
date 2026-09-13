//! Decision queries.

use super::*;

impl PromotionManager {
    pub async fn get_decision(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<PromotionDecision, CogniGraphError> {
        validate_record_id("decision id", id)?;
        self.ensure_repository(tenant).await?;
        let key = scoped_key(tenant, incarnation, "d", id);
        let record = self
            .get_authority_raw(tenant, DECISIONS_COLLECTION, &key)
            .await?
            .ok_or_else(|| not_found("promotion decision", id))?;
        self.validate_stored_decision(&record, tenant, incarnation)?;
        Ok(record)
    }

    pub async fn list_decisions(
        &self,
        tenant: &str,
        incarnation: &str,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<PromotionPage<PromotionDecision>, CogniGraphError> {
        self.list_records(
            DECISIONS_COLLECTION,
            tenant,
            incarnation,
            "d",
            limit,
            cursor,
            |record| self.validate_stored_decision(record, tenant, incarnation),
        )
        .await
    }

    pub async fn current(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
    ) -> Result<Option<PromotionHead>, CogniGraphError> {
        target.validate()?;
        self.ensure_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.reconcile_locked(tenant, incarnation, target, false)
            .await?;
        self.current_raw(tenant, incarnation, target).await
    }

    pub async fn reconcile(
        &self,
        tenant: &str,
        incarnation: &str,
        target: &PromotionTarget,
        dry_run: bool,
    ) -> Result<ReconcileResult, CogniGraphError> {
        target.validate()?;
        self.ensure_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.ensure_tenant_active(tenant)?;
        self.reconcile_locked(tenant, incarnation, target, dry_run)
            .await
    }
}
