//! Dispatch.

use super::*;

impl JobManager {
    pub(super) fn enqueue(self: &Arc<Self>, state: &AppState, tenant: String, id: String) {
        if self.shutting_down.load(Ordering::Acquire) || self.tenant_paused(&tenant) {
            return;
        }
        let key = (tenant.clone(), id.clone());
        let mut scheduler = self.scheduler.lock().expect("job scheduler lock");
        let mut scheduled = self.scheduled.lock().expect("job schedule lock");
        if !scheduled.insert(key) {
            return;
        }
        scheduler
            .by_tenant
            .entry(tenant.clone())
            .or_default()
            .push_back(ScheduledWork {
                runtime: JobRuntime::from_state(state),
                tenant: tenant.clone(),
                id,
            });
        let tenant_running = scheduler
            .running
            .as_ref()
            .is_some_and(|(running_tenant, _)| running_tenant == &tenant);
        if !tenant_running && !scheduler.rotation.iter().any(|queued| queued == &tenant) {
            scheduler.rotation.push_back(tenant);
        }
        let spawn = !scheduler.dispatcher_alive;
        if spawn {
            scheduler.dispatcher_alive = true;
        }
        drop(scheduled);
        drop(scheduler);
        if spawn {
            let manager = self.clone();
            tokio::spawn(async move { manager.dispatch_loop().await });
        }
    }

    pub(super) async fn dispatch_loop(self: Arc<Self>) {
        loop {
            let work = {
                let mut scheduler = self.scheduler.lock().expect("job scheduler lock");
                if self.shutting_down.load(Ordering::Acquire) {
                    scheduler.dispatcher_alive = false;
                    return;
                }
                let Some(tenant) = scheduler.rotation.pop_front() else {
                    scheduler.dispatcher_alive = false;
                    return;
                };
                let work = scheduler
                    .by_tenant
                    .get_mut(&tenant)
                    .and_then(VecDeque::pop_front);
                if scheduler
                    .by_tenant
                    .get(&tenant)
                    .is_some_and(VecDeque::is_empty)
                {
                    scheduler.by_tenant.remove(&tenant);
                }
                let Some(work) = work else {
                    continue;
                };
                scheduler.running = Some((work.tenant.clone(), work.id.clone()));
                work
            };
            self.metrics.dispatches.fetch_add(1, Ordering::Relaxed);
            let manager = self.clone();
            let runtime = work.runtime.clone();
            let tenant = work.tenant.clone();
            let id = work.id.clone();
            self.metrics.running.fetch_add(1, Ordering::Relaxed);
            *self.running_tenant.lock().expect("running tenant lock") = Some(work.tenant.clone());
            let execution =
                tokio::spawn(async move { manager.execute_work(runtime, tenant, id).await }).await;
            self.metrics.running.fetch_sub(1, Ordering::Relaxed);
            *self.running_tenant.lock().expect("running tenant lock") = None;
            let outcome = match execution {
                Ok(outcome) => outcome,
                Err(error) => {
                    tracing::error!(
                        tenant = work.tenant,
                        job_id = work.id,
                        %error,
                        "durable job worker panicked"
                    );
                    let _ = self
                        .fail_job(
                            &work.tenant,
                            &work.id,
                            format!("worker task failed: {error}"),
                        )
                        .await;
                    DispatchOutcome::Complete
                }
            };
            #[cfg(test)]
            {
                let delay = self.cleanup_delay_ms.load(Ordering::Acquire);
                if delay > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                }
            }
            self.finish_dispatch(work, outcome).await;
        }
    }

    pub(super) async fn execute_work(
        self: Arc<Self>,
        runtime: JobRuntime,
        tenant: String,
        id: String,
    ) -> DispatchOutcome {
        #[cfg(test)]
        if self.worker_claims_paused.load(Ordering::Acquire) {
            return DispatchOutcome::Parked;
        }
        let result: Result<DispatchOutcome, CogniGraphError> = CURRENT_TENANT
            .scope(tenant.clone(), self.run_job(&runtime, &tenant, &id))
            .await;
        match result {
            Ok(outcome) => outcome,
            Err(error) => {
                tracing::warn!(tenant, job_id = id, error = %error, "durable job failed");
                let _ = self.fail_job(&tenant, &id, error.to_string()).await;
                DispatchOutcome::Complete
            }
        }
    }

    pub(super) async fn finish_dispatch(&self, work: ScheduledWork, outcome: DispatchOutcome) {
        let outcome = {
            let _guard = self.transition_lock.lock().await;
            match outcome {
                DispatchOutcome::Continue
                    if self.shutting_down.load(Ordering::Acquire)
                        || self.tenant_paused(&work.tenant) =>
                {
                    if let Ok(Some(mut job)) = self.get_raw(&work.tenant, &work.id).await
                        && job.status == JobStatus::Running
                    {
                        let _ = self.requeue_interrupted(&work.tenant, &mut job).await;
                    }
                    DispatchOutcome::Parked
                }
                DispatchOutcome::Complete
                    if !self.shutting_down.load(Ordering::Acquire)
                        && !self.tenant_paused(&work.tenant) =>
                {
                    match self.get_raw(&work.tenant, &work.id).await {
                        Ok(Some(job))
                            if job.status == JobStatus::Queued
                                && runtime_identity_active(&work.runtime, &job)
                                    .await
                                    .unwrap_or(false) =>
                        {
                            DispatchOutcome::Continue
                        }
                        _ => DispatchOutcome::Complete,
                    }
                }
                outcome => outcome,
            }
        };

        let mut scheduler = self.scheduler.lock().expect("job scheduler lock");
        scheduler.running = None;
        if outcome == DispatchOutcome::Continue
            && !self.shutting_down.load(Ordering::Acquire)
            && !self.tenant_paused(&work.tenant)
        {
            scheduler
                .by_tenant
                .entry(work.tenant.clone())
                .or_default()
                .push_front(work.clone());
        } else {
            self.scheduled
                .lock()
                .expect("job schedule lock")
                .remove(&(work.tenant.clone(), work.id.clone()));
        }
        if scheduler
            .by_tenant
            .get(&work.tenant)
            .is_some_and(|queue| !queue.is_empty())
            && !scheduler
                .rotation
                .iter()
                .any(|queued| queued == &work.tenant)
        {
            scheduler.rotation.push_back(work.tenant);
        }
    }
}
