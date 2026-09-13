//! Cancellation retry.

use super::*;

impl JobManager {
    pub async fn cancel(
        self: &Arc<Self>,
        tenant: &str,
        incarnation: &str,
        id: &str,
        actor: JobActor,
        reason: Option<String>,
    ) -> Result<Mutation, CogniGraphError> {
        validate_reason(reason.as_deref())?;
        let _guard = self.transition_lock.lock().await;
        let mut job = self.get(tenant, incarnation, id).await?;
        if job.archived_at_ms.is_some() {
            return Err(CogniGraphError::DocumentConflict(format!(
                "archived job `{id}` is immutable"
            )));
        }
        let from = job.status;
        let event = if job.status == JobStatus::Queued {
            "canceled"
        } else {
            "cancel_requested"
        };
        match job.status {
            JobStatus::Queued => {
                job.status = JobStatus::Canceled;
                job.progress.phase = "canceled".into();
                job.finished_at_ms = Some(now_millis());
            }
            JobStatus::Running => {
                job.status = JobStatus::CancelRequested;
                job.progress.phase = "cancel_requested".into();
            }
            JobStatus::CancelRequested
            | JobStatus::Succeeded
            | JobStatus::Failed
            | JobStatus::Canceled => {
                return Ok(Mutation {
                    job,
                    changed: false,
                });
            }
        }
        job.updated_at_ms = now_millis();
        job.push_event(event, actor, Some(from), Some(job.status), reason);
        self.save_raw(tenant, &job).await?;
        self.metrics.cancel_requests.fetch_add(1, Ordering::Relaxed);
        if job.status == JobStatus::CancelRequested {
            self.metrics
                .cancel_requested
                .fetch_add(1, Ordering::Relaxed);
        }
        if job.status == JobStatus::Canceled {
            self.metrics.canceled.fetch_add(1, Ordering::Relaxed);
            self.release_active(tenant, incarnation, id);
            self.remove_scheduled(tenant, id);
        }
        Ok(Mutation { job, changed: true })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn retry(
        self: &Arc<Self>,
        state: AppState,
        tenant: &str,
        incarnation: &str,
        id: &str,
        actor: JobActor,
        idempotency_key: &str,
        mode: RetryMode,
        reason: Option<String>,
    ) -> Result<RetryMutation, CogniGraphError> {
        validate_idempotency_key(idempotency_key)?;
        validate_reason(reason.as_deref())?;
        let retry_hash = digest_bytes(idempotency_key.as_bytes());
        let request_digest = digest_json(&json!({
            "mode": mode,
            "reason": reason,
        }))?;
        let _guard = self.transition_lock.lock().await;
        let mut job = self.get(tenant, incarnation, id).await?;
        if job.archived_at_ms.is_some() {
            return Err(CogniGraphError::DocumentConflict(format!(
                "archived job `{id}` cannot be retried"
            )));
        }
        if self.tenant_paused(tenant) || self.shutting_down.load(Ordering::Acquire) {
            return Err(CogniGraphError::Forbidden(format!(
                "tenant `{tenant}` is not accepting durable jobs"
            )));
        }
        if !tenant_identity_active(&state, tenant, incarnation).await? {
            return Err(CogniGraphError::Forbidden(format!(
                "tenant `{tenant}` is suspended or has been replaced"
            )));
        }
        if let Some(existing) = job
            .retry_requests
            .iter()
            .find(|request| request.key_hash == retry_hash)
        {
            if existing.request_digest == request_digest {
                return Ok(RetryMutation {
                    job,
                    replayed: true,
                });
            }
            return Err(CogniGraphError::DocumentConflict(
                "retry idempotency key was already used for a different request".into(),
            ));
        }
        if job.retry_requests.len() >= MAX_RETRIES {
            return Err(CogniGraphError::DocumentConflict(format!(
                "job `{id}` reached the {MAX_RETRIES}-retry audit limit"
            )));
        }
        if !matches!(job.status, JobStatus::Failed | JobStatus::Canceled) {
            return Err(CogniGraphError::DocumentConflict(format!(
                "job `{id}` cannot be retried from status `{:?}`",
                job.status
            )));
        }
        let tenant_limit = self.tenant_active_limit(&state, tenant).await?;
        self.reserve_active(tenant, incarnation, id, tenant_limit)?;
        let from = job.status;
        job.retry_requests.push(RetryRequestRecord {
            key_hash: retry_hash,
            request_digest,
        });
        job.status = JobStatus::Queued;
        job.progress.phase = "queued".into();
        if mode == RetryMode::Restart {
            job.progress.completed = 0;
            job.progress.facts_grounded = 0;
            job.progress.side_views_written = 0;
        }
        job.result = None;
        job.error = None;
        job.finished_at_ms = None;
        job.updated_at_ms = now_millis();
        job.push_event(
            "retry_requested",
            actor,
            Some(from),
            Some(JobStatus::Queued),
            Some(match reason {
                Some(reason) => format!("mode={mode:?}; {reason}"),
                None => format!("mode={mode:?}"),
            }),
        );
        if let Err(error) = self.save_raw(tenant, &job).await {
            self.release_active(tenant, incarnation, id);
            return Err(error);
        }
        self.metrics.retries.fetch_add(1, Ordering::Relaxed);
        self.enqueue(&state, tenant.into(), id.into());
        drop(_guard);
        Ok(RetryMutation {
            job,
            replayed: false,
        })
    }

    pub fn can_manage(job: &JobRecord, user: Option<&User>) -> bool {
        match user {
            None => job.actor.actor_type == ActorType::Anonymous,
            Some(user) if user.role == Role::Admin => true,
            Some(user) => job.actor.user_key.as_deref() == Some(user.key.as_str()),
        }
    }
}
