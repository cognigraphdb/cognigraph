//! Admission.

use super::*;

impl JobManager {
    pub async fn tenant_incarnation(
        state: &AppState,
        tenant: &str,
    ) -> Result<String, CogniGraphError> {
        if let Some(incarnation) = crate::tenancy::request_incarnation(tenant) {
            return Ok(incarnation);
        }
        let Some(auth) = &state.auth else {
            return Ok(if tenant == DEFAULT_TENANT {
                DEFAULT_TENANT.into()
            } else {
                format!("unmanaged:{tenant}")
            });
        };
        match auth.get_tenant(tenant).await? {
            Some(record) => Ok(effective_incarnation(&record)),
            None if tenant == DEFAULT_TENANT => Ok(DEFAULT_TENANT.into()),
            None => Err(CogniGraphError::Forbidden(format!(
                "unknown tenant `{tenant}`"
            ))),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn submit(
        self: &Arc<Self>,
        state: AppState,
        tenant: String,
        tenant_incarnation: String,
        actor: JobActor,
        idempotency_key: &str,
        kind: JobKind,
        input: Value,
    ) -> Result<Submission, CogniGraphError> {
        validate_idempotency_key(idempotency_key)?;
        let input_bytes = serde_json::to_vec(&input)?;
        if input_bytes.len() > MAX_JOB_INPUT_BYTES {
            return Err(CogniGraphError::ValidationError(format!(
                "job input exceeds {MAX_JOB_INPUT_BYTES} bytes"
            )));
        }
        let key_hash = digest_bytes(idempotency_key.as_bytes());
        let id = digest_bytes(format!("{tenant}\0{tenant_incarnation}\0{key_hash}").as_bytes());
        let input_digest = digest_json(&json!({ "kind": kind, "input": input }))?;

        self.ensure_repository(&tenant).await?;
        if let Some(existing) = self.get_raw_any(&tenant, &id).await? {
            return self.replay_or_conflict(existing, &tenant, &tenant_incarnation, &input_digest);
        }
        let preliminary_tenant_limit = self.tenant_active_limit(&state, &tenant).await?;
        self.check_capacity(&tenant, &tenant_incarnation, preliminary_tenant_limit)?;

        let payload = CURRENT_TENANT
            .scope(
                tenant.clone(),
                prepare_payload(
                    &state,
                    kind,
                    &input,
                    self.batch_size.load(Ordering::Relaxed),
                ),
            )
            .await?;
        let now = now_millis();
        let total = match &payload {
            JobPayload::Ingest { chunks, .. } => chunks.len(),
            JobPayload::Evaluate { .. } => 1,
            JobPayload::SideviewsGenerate { keys, .. } => keys.len(),
            JobPayload::Draft { documents, .. } => documents.len(),
        };
        let schema_version = match &payload {
            JobPayload::Evaluate {
                promotion_context: Some(context),
                ..
            } if context.schema_version
                == crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION =>
            {
                M23_JOB_SCHEMA_VERSION
            }
            JobPayload::Evaluate {
                promotion_context: Some(context),
                ..
            } if context.schema_version
                == crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION =>
            {
                M22_JOB_SCHEMA_VERSION
            }
            JobPayload::Evaluate {
                promotion_context: Some(context),
                ..
            } if context.schema_version
                == crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION =>
            {
                M21_JOB_SCHEMA_VERSION
            }
            _ => JOB_SCHEMA_VERSION,
        };
        let mut job = JobRecord {
            id: id.clone(),
            schema_version,
            tenant: tenant.clone(),
            tenant_incarnation,
            kind,
            status: JobStatus::Queued,
            idempotency_key_hash: key_hash,
            input_digest,
            actor: actor.clone(),
            attempt: 0,
            recoveries: 0,
            created_at_ms: now,
            updated_at_ms: now,
            started_at_ms: None,
            finished_at_ms: None,
            archived_at_ms: None,
            progress: JobProgress {
                phase: "queued".into(),
                completed: 0,
                total,
                facts_grounded: 0,
                side_views_written: 0,
            },
            input,
            result: None,
            error: None,
            events: Vec::new(),
            retry_requests: Vec::new(),
            payload,
        };
        job.push_event("submitted", actor, None, Some(JobStatus::Queued), None);

        #[cfg(test)]
        {
            self.admission_waiting.store(true, Ordering::Release);
            while self.admission_reservations_paused.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
            self.admission_waiting.store(false, Ordering::Release);
        }

        let _guard = self.transition_lock.lock().await;
        if self.tenant_paused(&tenant) || self.shutting_down.load(Ordering::Acquire) {
            return Err(CogniGraphError::Forbidden(format!(
                "tenant `{tenant}` is not accepting durable jobs"
            )));
        }
        if !tenant_identity_active(&state, &tenant, &job.tenant_incarnation).await? {
            return Err(CogniGraphError::Forbidden(format!(
                "tenant `{tenant}` is suspended or has been replaced"
            )));
        }
        if let Some(existing) = self.get_raw_any(&tenant, &id).await? {
            return self.replay_or_conflict(
                existing,
                &tenant,
                &job.tenant_incarnation,
                &job.input_digest,
            );
        }
        let final_tenant_limit = self.tenant_active_limit(&state, &tenant).await?;
        self.reserve_active(
            &tenant,
            &job.tenant_incarnation,
            &job.id,
            final_tenant_limit,
        )?;
        let created = match self.create_raw(&tenant, &job).await {
            Ok(()) => {
                self.metrics.submitted.fetch_add(1, Ordering::Relaxed);
                Ok(Submission {
                    job,
                    replayed: false,
                })
            }
            Err(CogniGraphError::DocumentConflict(_)) => {
                self.release_active(&tenant, &job.tenant_incarnation, &job.id);
                let existing = self.get_raw_any(&tenant, &id).await?.ok_or_else(|| {
                    CogniGraphError::BackendError(
                        "idempotent job insert conflicted but the record is unavailable".into(),
                    )
                })?;
                self.replay_or_conflict(
                    existing,
                    &tenant,
                    &job.tenant_incarnation,
                    &job.input_digest,
                )
            }
            Err(error) => {
                self.release_active(&tenant, &job.tenant_incarnation, &job.id);
                Err(error)
            }
        }?;
        if !created.replayed {
            self.enqueue(&state, tenant, id);
        }
        drop(_guard);
        Ok(created)
    }

    pub(super) fn replay_or_conflict(
        &self,
        existing: JobRecord,
        tenant: &str,
        incarnation: &str,
        input_digest: &str,
    ) -> Result<Submission, CogniGraphError> {
        if existing.tenant == tenant
            && existing.tenant_incarnation == incarnation
            && existing.input_digest == input_digest
        {
            self.metrics.replayed.fetch_add(1, Ordering::Relaxed);
            Ok(Submission {
                job: existing,
                replayed: true,
            })
        } else {
            self.metrics.conflicts.fetch_add(1, Ordering::Relaxed);
            Err(CogniGraphError::DocumentConflict(
                "idempotency key was already used for a different job request".into(),
            ))
        }
    }
}
