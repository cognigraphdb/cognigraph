//! Archival.

use super::*;

impl JobManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn archive_terminal(
        &self,
        tenant: &str,
        incarnation: &str,
        actor: JobActor,
        before_ms: Option<u64>,
        limit: Option<usize>,
        cursor: Option<&str>,
        dry_run: bool,
        reason: Option<String>,
    ) -> Result<ArchiveResult, CogniGraphError> {
        validate_reason(reason.as_deref())?;
        self.ensure_repository(tenant).await?;
        let limit = limit
            .unwrap_or_else(|| self.archive_batch_size.load(Ordering::Relaxed))
            .clamp(1, MAX_JOB_OPERATOR_BATCH);
        let before_ms = before_ms.unwrap_or_else(|| {
            now_millis().saturating_sub(
                self.retention_secs
                    .load(Ordering::Relaxed)
                    .saturating_mul(1_000),
            )
        });
        let fingerprint =
            digest_bytes(format!("{}\0{}\0{before_ms}\0{dry_run}", tenant, incarnation).as_bytes());
        let after = cursor
            .map(|cursor| decode_cursor(cursor, "archive", &fingerprint))
            .transpose()?;
        let scan_limit = limit.saturating_mul(16).clamp(64, MAX_JOB_CURSOR_SCAN);
        let fields = [
            "id",
            "tenant",
            "tenant_incarnation",
            "status",
            "finished_at_ms",
            "archived_at_ms",
        ]
        .map(str::to_string);
        let _guard = self.transition_lock.lock().await;
        let page = CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend.list_documents_after_key(
                    JOBS_COLLECTION,
                    after.as_deref(),
                    &fields,
                    scan_limit,
                ),
            )
            .await?;
        let page_len = page.len();
        let mut result = ArchiveResult {
            dry_run,
            scanned: 0,
            eligible: 0,
            archived: 0,
            next_cursor: None,
        };
        let mut last_key = after;
        for summary in page {
            let key = summary
                .get("_key")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CogniGraphError::BackendError(
                        "job archive scan returned a row without `_key`".into(),
                    )
                })?
                .to_string();
            last_key = Some(key.clone());
            result.scanned += 1;
            if summary.get("tenant").and_then(Value::as_str) != Some(tenant)
                || summary.get("tenant_incarnation").and_then(Value::as_str) != Some(incarnation)
            {
                continue;
            }
            let status: JobStatus =
                serde_json::from_value(summary.get("status").cloned().ok_or_else(|| {
                    CogniGraphError::BackendError(format!(
                        "job `{key}` is missing status during archival"
                    ))
                })?)?;
            let finished = summary.get("finished_at_ms").and_then(Value::as_u64);
            if !status.terminal()
                || finished.is_none_or(|finished| finished > before_ms)
                || summary
                    .get("archived_at_ms")
                    .is_some_and(|value| !value.is_null())
            {
                continue;
            }
            result.eligible += 1;
            if !dry_run {
                let mut job = self.get_raw(tenant, &key).await?.ok_or_else(|| {
                    CogniGraphError::DocumentNotFound {
                        collection: JOBS_COLLECTION.into(),
                        key: key.clone(),
                    }
                })?;
                let archived_at = now_millis();
                job.archived_at_ms = Some(archived_at);
                job.updated_at_ms = archived_at;
                job.push_event(
                    "archived",
                    actor.clone(),
                    Some(job.status),
                    Some(job.status),
                    reason.clone(),
                );
                let archived = self.copy_to_archive(tenant, &job).await?;
                if let Err(error) = self.upsert_catalog(tenant, &archived).await {
                    self.catalog_errors
                        .lock()
                        .expect("job catalog health lock")
                        .insert(tenant.into(), error.to_string());
                    return Err(error);
                }
                CURRENT_TENANT
                    .scope(
                        tenant.into(),
                        self.raw_backend.delete_document(JOBS_COLLECTION, &job.id),
                    )
                    .await?;
                self.metrics.archived.fetch_add(1, Ordering::Relaxed);
                result.archived += 1;
            }
            if result.eligible == limit {
                result.next_cursor = last_key
                    .as_deref()
                    .map(|key| encode_scoped_cursor("archive", &fingerprint, key));
                return Ok(result);
            }
        }
        if page_len == scan_limit {
            result.next_cursor = last_key
                .as_deref()
                .map(|key| encode_scoped_cursor("archive", &fingerprint, key));
        }
        Ok(result)
    }
}
