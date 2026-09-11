//! Status.

use super::*;

impl JobManager {
    pub async fn operator_status(
        &self,
        state: &AppState,
        tenant: &str,
        incarnation: &str,
    ) -> Result<Value, CogniGraphError> {
        self.ensure_repository(tenant).await?;
        let (tenant_active, global_active) = {
            let active = self.active_jobs.lock().expect("active jobs lock");
            let tenant_active = active
                .iter()
                .filter(|(active_tenant, active_incarnation, _)| {
                    active_tenant == tenant && active_incarnation == incarnation
                })
                .count();
            (tenant_active, active.len())
        };
        let (ready, running) = {
            let scheduler = self.scheduler.lock().expect("job scheduler lock");
            (
                scheduler.by_tenant.get(tenant).map_or(0, VecDeque::len),
                scheduler
                    .running
                    .as_ref()
                    .is_some_and(|(running_tenant, _)| running_tenant == tenant),
            )
        };
        let catalog_error = {
            self.catalog_errors
                .lock()
                .expect("job catalog health lock")
                .get(tenant)
                .cloned()
        };
        let last_reconciliation = {
            self.last_reconciliation
                .lock()
                .expect("job reconciliation lock")
                .get(tenant)
                .cloned()
        };
        let effective_tenant_limit = self.tenant_active_limit(state, tenant).await?;
        Ok(json!({
            "tenant": tenant,
            "tenant_incarnation": incarnation,
            "paused": self.tenant_paused(tenant),
            "shutting_down": self.shutting_down.load(Ordering::Acquire),
            "queue": {
                "active": tenant_active,
                "ready": ready,
                "running": running,
                "global_active": global_active,
            },
            "limits": {
                "max_active_total": self.max_active_total.load(Ordering::Relaxed),
                "max_active_per_tenant": self.max_active_per_tenant.load(Ordering::Relaxed),
                "effective_tenant_max_active": effective_tenant_limit,
                "retention_secs": self.retention_secs.load(Ordering::Relaxed),
                "archive_batch_size": self.archive_batch_size.load(Ordering::Relaxed),
            },
            "catalog": {
                "ready": catalog_error.is_none(),
                "error": catalog_error,
                "last_reconciliation": last_reconciliation,
            }
        }))
    }
}
