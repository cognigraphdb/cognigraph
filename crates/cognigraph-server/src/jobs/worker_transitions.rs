//! Worker transitions.

use super::*;

impl JobManager {
    pub(super) async fn pause_or_cancel(
        &self,
        runtime: &JobRuntime,
        job: &mut JobRecord,
    ) -> Result<bool, CogniGraphError> {
        let _guard = self.transition_lock.lock().await;
        *job = self.get_raw(&job.tenant, &job.id).await?.ok_or_else(|| {
            CogniGraphError::DocumentNotFound {
                collection: "jobs".into(),
                key: job.id.clone(),
            }
        })?;
        if job.status == JobStatus::CancelRequested {
            let tenant = job.tenant.clone();
            self.finish_canceled(&tenant, job, "canceled before the next operation")
                .await?;
            return Ok(true);
        }
        if self.shutting_down.load(Ordering::Acquire)
            || self.tenant_paused(&job.tenant)
            || !runtime_identity_active(runtime, job).await?
        {
            let tenant = job.tenant.clone();
            self.requeue_interrupted(&tenant, job).await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) async fn finish_succeeded(
        &self,
        tenant: &str,
        job: &mut JobRecord,
        result: Value,
    ) -> Result<(), CogniGraphError> {
        let from = job.status;
        job.status = JobStatus::Succeeded;
        job.progress.phase = "succeeded".into();
        job.progress.completed = job.progress.total;
        job.result = Some(result);
        job.error = None;
        job.updated_at_ms = now_millis();
        job.finished_at_ms = Some(job.updated_at_ms);
        let message = (from == JobStatus::CancelRequested)
            .then(|| "cancellation arrived after the final operation committed".into());
        job.push_event(
            "succeeded",
            JobActor::worker(),
            Some(from),
            Some(JobStatus::Succeeded),
            message,
        );
        self.save_raw(tenant, job).await?;
        self.metrics.succeeded.fetch_add(1, Ordering::Relaxed);
        self.release_active(tenant, &job.tenant_incarnation, &job.id);
        self.release_cancel_requested(from);
        Ok(())
    }

    pub(super) async fn finish_canceled(
        &self,
        tenant: &str,
        job: &mut JobRecord,
        message: &str,
    ) -> Result<(), CogniGraphError> {
        let from = job.status;
        job.status = JobStatus::Canceled;
        job.progress.phase = "canceled".into();
        job.updated_at_ms = now_millis();
        job.finished_at_ms = Some(job.updated_at_ms);
        job.push_event(
            "canceled",
            JobActor::worker(),
            Some(from),
            Some(JobStatus::Canceled),
            Some(message.into()),
        );
        self.save_raw(tenant, job).await?;
        self.metrics.canceled.fetch_add(1, Ordering::Relaxed);
        self.release_active(tenant, &job.tenant_incarnation, &job.id);
        self.release_cancel_requested(from);
        Ok(())
    }

    pub(super) async fn requeue_interrupted(
        &self,
        tenant: &str,
        job: &mut JobRecord,
    ) -> Result<(), CogniGraphError> {
        let from = job.status;
        job.status = JobStatus::Queued;
        job.progress.phase = "queued".into();
        job.updated_at_ms = now_millis();
        job.push_event(
            "interrupted",
            JobActor::worker(),
            Some(from),
            Some(JobStatus::Queued),
            Some("worker paused at a durable boundary".into()),
        );
        self.save_raw(tenant, job).await
    }

    pub(super) async fn fail_job(
        &self,
        tenant: &str,
        id: &str,
        message: String,
    ) -> Result<(), CogniGraphError> {
        let _guard = self.transition_lock.lock().await;
        let Some(mut job) = self.get_raw(tenant, id).await? else {
            return Ok(());
        };
        if job.status.terminal() {
            return Ok(());
        }
        let from = job.status;
        job.status = JobStatus::Failed;
        job.progress.phase = "failed".into();
        job.updated_at_ms = now_millis();
        job.finished_at_ms = Some(job.updated_at_ms);
        job.error = Some(json!({
            "code": "execution_failed",
            "message": message,
            "retryable": true,
        }));
        job.push_event(
            "failed",
            JobActor::worker(),
            Some(from),
            Some(JobStatus::Failed),
            None,
        );
        self.save_raw(tenant, &job).await?;
        self.metrics.failed.fetch_add(1, Ordering::Relaxed);
        self.release_active(tenant, &job.tenant_incarnation, &job.id);
        self.release_cancel_requested(from);
        Ok(())
    }
}
