//! Capacity.

use super::*;

impl JobManager {
    pub(super) fn tenant_paused(&self, tenant: &str) -> bool {
        self.paused_tenants
            .lock()
            .expect("paused tenants lock")
            .contains(tenant)
    }

    pub(super) async fn tenant_active_limit(
        &self,
        state: &AppState,
        tenant: &str,
    ) -> Result<usize, CogniGraphError> {
        let host_limit = self.max_active_per_tenant.load(Ordering::Relaxed);
        let Some(auth) = &state.auth else {
            return Ok(host_limit);
        };
        let Some(record) = auth.get_tenant(tenant).await? else {
            return Ok(host_limit);
        };
        let Some(value) = record
            .quotas
            .as_ref()
            .and_then(|quotas| quotas.get("max_active_jobs"))
        else {
            return Ok(host_limit);
        };
        let configured = value.as_u64().ok_or_else(|| {
            CogniGraphError::ValidationError(format!(
                "tenant `{tenant}` quota `max_active_jobs` must be a non-negative integer"
            ))
        })?;
        let configured = usize::try_from(configured).unwrap_or(usize::MAX);
        Ok(configured.min(host_limit))
    }

    pub(super) fn check_capacity(
        &self,
        tenant: &str,
        incarnation: &str,
        tenant_limit: usize,
    ) -> Result<(), CogniGraphError> {
        let active = self.active_jobs.lock().expect("active jobs lock");
        self.check_capacity_locked(&active, tenant, incarnation, tenant_limit)
    }

    pub(super) fn check_capacity_locked(
        &self,
        active: &HashSet<(String, String, String)>,
        tenant: &str,
        incarnation: &str,
        tenant_limit: usize,
    ) -> Result<(), CogniGraphError> {
        let total_limit = self.max_active_total.load(Ordering::Relaxed);
        if active.len() >= total_limit {
            self.metrics
                .backpressured_global
                .fetch_add(1, Ordering::Relaxed);
            return Err(CogniGraphError::CapacityExceeded(format!(
                "global durable-job capacity {total_limit} is full"
            )));
        }
        let tenant_active = active
            .iter()
            .filter(|(active_tenant, active_incarnation, _)| {
                active_tenant == tenant && active_incarnation == incarnation
            })
            .count();
        if tenant_active >= tenant_limit {
            self.metrics
                .backpressured_tenant
                .fetch_add(1, Ordering::Relaxed);
            return Err(CogniGraphError::CapacityExceeded(format!(
                "tenant `{tenant}` durable-job capacity {tenant_limit} is full"
            )));
        }
        Ok(())
    }

    pub(super) fn reserve_active(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
        tenant_limit: usize,
    ) -> Result<(), CogniGraphError> {
        let mut active = self.active_jobs.lock().expect("active jobs lock");
        let key = (tenant.to_string(), incarnation.to_string(), id.to_string());
        if active.contains(&key) {
            return Ok(());
        }
        self.check_capacity_locked(&active, tenant, incarnation, tenant_limit)?;
        active.insert(key);
        Ok(())
    }

    pub(super) fn release_active(&self, tenant: &str, incarnation: &str, id: &str) {
        self.active_jobs.lock().expect("active jobs lock").remove(&(
            tenant.to_string(),
            incarnation.to_string(),
            id.to_string(),
        ));
    }

    pub(super) fn release_cancel_requested(&self, from: JobStatus) {
        if from == JobStatus::CancelRequested {
            let _ = self.metrics.cancel_requested.fetch_update(
                Ordering::Relaxed,
                Ordering::Relaxed,
                |value| Some(value.saturating_sub(1)),
            );
        }
    }
}
