//! Tenant lifecycle.

use super::*;

impl JobManager {
    pub async fn pause_tenant(&self, tenant: &str) {
        {
            let _guard = self.transition_lock.lock().await;
            self.paused_tenants
                .lock()
                .expect("paused tenants lock")
                .insert(tenant.into());
            self.park_scheduled_tenant(tenant);
        }
        loop {
            let running = self
                .running_tenant
                .lock()
                .expect("running tenant lock")
                .clone();
            let scheduled = self
                .scheduled
                .lock()
                .expect("job schedule lock")
                .iter()
                .any(|(scheduled_tenant, _)| scheduled_tenant == tenant);
            if running.as_deref() != Some(tenant) && !scheduled {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    }

    pub fn resume_tenant(&self, tenant: &str) {
        self.paused_tenants
            .lock()
            .expect("paused tenants lock")
            .remove(tenant);
    }

    /// Drop process-local queue accounting after a tenant store has been
    /// retired. `pause_tenant` must run first so no worker can still commit to
    /// the quarantined store. The paused fence remains until a new tenant
    /// incarnation with the same name is explicitly resumed.
    pub fn retire_tenant(&self, tenant: &str) {
        self.park_scheduled_tenant(tenant);
        self.active_jobs
            .lock()
            .expect("active jobs lock")
            .retain(|(active_tenant, _, _)| active_tenant != tenant);
        self.collection_errors
            .lock()
            .expect("job collection health lock")
            .remove(tenant);
        self.data_errors
            .lock()
            .expect("job data health lock")
            .remove(tenant);
        self.catalog_errors
            .lock()
            .expect("job catalog health lock")
            .remove(tenant);
        self.last_reconciliation
            .lock()
            .expect("job reconciliation lock")
            .remove(tenant);
    }

    pub async fn shutdown(&self) {
        self.begin_shutdown();
        loop {
            let drained = {
                let scheduler = self.scheduler.lock().expect("job scheduler lock");
                self.scheduled.lock().expect("job schedule lock").is_empty()
                    && scheduler.running.is_none()
                    && !scheduler.dispatcher_alive
            };
            if drained {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    }

    pub fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
        let mut scheduler = self.scheduler.lock().expect("job scheduler lock");
        let queued = scheduler
            .by_tenant
            .values()
            .flat_map(|queue| queue.iter())
            .map(|work| (work.tenant.clone(), work.id.clone()))
            .collect::<Vec<_>>();
        scheduler.by_tenant.clear();
        scheduler.rotation.clear();
        let running = scheduler.running.clone();
        drop(scheduler);
        let mut scheduled = self.scheduled.lock().expect("job schedule lock");
        for key in queued {
            if running.as_ref() != Some(&key) {
                scheduled.remove(&key);
            }
        }
    }

    /// Fence an idle tenant for snapshot replacement without disturbing live
    /// work. The pause and active-job check share the transition lock, so a
    /// submission cannot slip between them. If work exists, the pause is
    /// rolled back before the worker can observe it.
    pub async fn pause_tenant_if_idle(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<bool, CogniGraphError> {
        self.ensure_repository(tenant).await?;
        let _guard = self.transition_lock.lock().await;
        self.paused_tenants
            .lock()
            .expect("paused tenants lock")
            .insert(tenant.into());
        let jobs = match self.list_raw(tenant).await {
            Ok(jobs) => jobs,
            Err(error) => {
                self.paused_tenants
                    .lock()
                    .expect("paused tenants lock")
                    .remove(tenant);
                return Err(error);
            }
        };
        let active = jobs.into_iter().any(|job| {
            job.tenant == tenant && job.tenant_incarnation == incarnation && !job.status.terminal()
        });
        if active {
            self.paused_tenants
                .lock()
                .expect("paused tenants lock")
                .remove(tenant);
        }
        Ok(!active)
    }
}
