//! Recovery.

use super::*;

impl JobManager {
    pub async fn recover_tenant(
        self: &Arc<Self>,
        state: AppState,
        tenant: String,
        incarnation: String,
    ) -> Result<usize, CogniGraphError> {
        self.reconcile_fully(&tenant, &incarnation).await?;
        self.recover_tenant_prepared(state, tenant, incarnation, false)
            .await
    }

    /// Rebuild the tenant's derived queue state while it remains fenced, then
    /// resume admissions and scheduling atomically under the transition lock.
    pub async fn resume_and_recover(
        self: &Arc<Self>,
        state: AppState,
        tenant: String,
        incarnation: String,
    ) -> Result<usize, CogniGraphError> {
        self.reconcile_fully(&tenant, &incarnation).await?;
        self.recover_tenant_prepared(state, tenant, incarnation, true)
            .await
    }

    pub(super) async fn recover_tenant_prepared(
        self: &Arc<Self>,
        state: AppState,
        tenant: String,
        incarnation: String,
        resume: bool,
    ) -> Result<usize, CogniGraphError> {
        self.ensure_repository(&tenant).await?;
        let _guard = self.transition_lock.lock().await;
        let worker_active = self
            .running_tenant
            .lock()
            .expect("running tenant lock")
            .as_deref()
            == Some(tenant.as_str())
            || self
                .scheduled
                .lock()
                .expect("job schedule lock")
                .iter()
                .any(|(scheduled_tenant, _)| scheduled_tenant == &tenant);
        if worker_active {
            return Err(CogniGraphError::DocumentConflict(format!(
                "tenant `{tenant}` cannot be recovered while its worker is active"
            )));
        }
        let mut recovered = 0;
        let mut ready = Vec::new();
        let mut active = HashSet::new();
        for mut job in self.list_raw(&tenant).await? {
            if job.tenant != tenant || job.tenant_incarnation != incarnation {
                continue;
            }
            self.validate_recoverable_job(&tenant, &incarnation, &job)?;
            match job.status {
                JobStatus::Running => {
                    let from = job.status;
                    job.status = JobStatus::Queued;
                    job.progress.phase = "queued".into();
                    job.recoveries = job.recoveries.saturating_add(1);
                    job.updated_at_ms = now_millis();
                    job.push_event(
                        "recovered",
                        JobActor::worker(),
                        Some(from),
                        Some(JobStatus::Queued),
                        Some("process restarted before a terminal checkpoint".into()),
                    );
                    self.save_raw(&tenant, &job).await?;
                    self.metrics.recovered.fetch_add(1, Ordering::Relaxed);
                    active.insert((tenant.clone(), incarnation.clone(), job.id.clone()));
                    ready.push((job.created_at_ms, job.id));
                    recovered += 1;
                }
                JobStatus::CancelRequested => {
                    let from = job.status;
                    job.status = JobStatus::Canceled;
                    job.progress.phase = "canceled".into();
                    job.finished_at_ms = Some(now_millis());
                    job.updated_at_ms = now_millis();
                    job.push_event(
                        "recovered_cancellation",
                        JobActor::worker(),
                        Some(from),
                        Some(JobStatus::Canceled),
                        Some("cancellation was pending when the process stopped".into()),
                    );
                    self.save_raw(&tenant, &job).await?;
                    self.metrics.canceled.fetch_add(1, Ordering::Relaxed);
                }
                JobStatus::Queued => {
                    active.insert((tenant.clone(), incarnation.clone(), job.id.clone()));
                    ready.push((job.created_at_ms, job.id));
                    recovered += 1;
                }
                JobStatus::Succeeded | JobStatus::Failed | JobStatus::Canceled => {}
            }
        }
        {
            let mut active_jobs = self.active_jobs.lock().expect("active jobs lock");
            active_jobs.retain(|(active_tenant, active_incarnation, _)| {
                active_tenant != &tenant || active_incarnation != &incarnation
            });
            active_jobs.extend(active);
        }
        if resume {
            self.paused_tenants
                .lock()
                .expect("paused tenants lock")
                .remove(&tenant);
        }
        ready.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        for (_, id) in ready {
            self.enqueue(&state, tenant.clone(), id);
        }
        self.data_errors
            .lock()
            .expect("job data health lock")
            .remove(&tenant);
        Ok(recovered)
    }
}
