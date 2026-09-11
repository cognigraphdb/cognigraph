//! Listing.

use super::*;

impl JobManager {
    #[allow(clippy::too_many_arguments)]
    pub async fn list_cursor(
        &self,
        tenant: &str,
        incarnation: &str,
        kind: Option<JobKind>,
        status: Option<JobStatus>,
        archived: ArchiveFilter,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<CursorPage, CogniGraphError> {
        if !(1..=200).contains(&limit) {
            return Err(CogniGraphError::ValidationError(
                "job cursor list requires limit 1-200".into(),
            ));
        }
        self.ensure_repository(tenant).await?;
        let scope = catalog_scope(tenant, incarnation);
        let fingerprint = list_cursor_fingerprint(&scope, kind, status, archived)?;
        let mut after = match cursor {
            Some(cursor) => decode_scoped_cursor(cursor, "list", &fingerprint, &scope)?,
            None => scope.clone(),
        };
        let fields = ["schema_version", "job", "archived"].map(str::to_string);
        let mut jobs = Vec::with_capacity(limit);
        let mut scanned = 0;

        while scanned < MAX_JOB_CURSOR_SCAN {
            let page_limit = 64.min(MAX_JOB_CURSOR_SCAN - scanned);
            let page = CURRENT_TENANT
                .scope(
                    tenant.into(),
                    self.raw_backend.list_documents_after_key(
                        JOB_CATALOG_COLLECTION,
                        Some(&after),
                        &fields,
                        page_limit,
                    ),
                )
                .await?;
            if page.is_empty() {
                return Ok(CursorPage {
                    jobs,
                    next_cursor: None,
                    scanned,
                });
            }
            let page_len = page.len();
            for document in page {
                let Some(key) = document.get("_key").and_then(Value::as_str) else {
                    let error = CogniGraphError::BackendError(
                        "job catalog row is missing its string `_key`".into(),
                    );
                    self.mark_catalog_error(tenant, format!("listing: {error}"));
                    return Err(error);
                };
                let key = key.to_string();
                if !key.starts_with(&scope) {
                    return Ok(CursorPage {
                        jobs,
                        next_cursor: None,
                        scanned,
                    });
                }
                after = key.clone();
                scanned += 1;
                let record: JobCatalogRecord = match serde_json::from_value(document) {
                    Ok(record) => record,
                    Err(error) => {
                        self.mark_catalog_error(
                            tenant,
                            format!("listing catalog row `{key}`: {error}"),
                        );
                        continue;
                    }
                };
                if let Err(error) = validate_catalog_record(&key, &record, tenant, incarnation) {
                    self.mark_catalog_error(
                        tenant,
                        format!("listing catalog row `{key}`: {error}"),
                    );
                    continue;
                }
                if kind.is_some_and(|expected| record.job.kind != expected)
                    || status.is_some_and(|expected| record.job.status != expected)
                    || !archived.matches(record.archived)
                {
                    continue;
                }
                jobs.push(record.job);
                if jobs.len() == limit {
                    return Ok(CursorPage {
                        jobs,
                        next_cursor: Some(encode_scoped_cursor("list", &fingerprint, &after)),
                        scanned,
                    });
                }
            }
            if page_len < page_limit {
                return Ok(CursorPage {
                    jobs,
                    next_cursor: None,
                    scanned,
                });
            }
        }

        Ok(CursorPage {
            jobs,
            next_cursor: Some(encode_scoped_cursor("list", &fingerprint, &after)),
            scanned,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list(
        &self,
        tenant: &str,
        incarnation: &str,
        kind: Option<JobKind>,
        status: Option<JobStatus>,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<JobSummaryRecord>, usize), CogniGraphError> {
        if !(1..=200).contains(&limit) || offset > MAX_JOB_LIST_OFFSET {
            return Err(CogniGraphError::ValidationError(format!(
                "job list requires limit 1-200 and offset 0-{MAX_JOB_LIST_OFFSET}"
            )));
        }
        self.ensure_repository(tenant).await?;
        const SCAN_PAGE: usize = 16;
        let window = offset + limit;
        let mut candidates = Vec::with_capacity(window.min(256));
        let mut total = 0;
        let mut scan_offset = 0;
        loop {
            let page = self
                .list_summary_page(tenant, SCAN_PAGE, scan_offset)
                .await?;
            let page_len = page.len();
            if page_len == 0 {
                break;
            }
            scan_offset += page_len;
            for job in page {
                if job.tenant != tenant
                    || job.tenant_incarnation != incarnation
                    || kind.is_some_and(|kind| job.kind != kind)
                    || status.is_some_and(|status| job.status != status)
                {
                    continue;
                }
                total += 1;
                candidates.push(job);
            }
            if candidates.len() > window.saturating_mul(2).max(1) {
                sort_job_summaries(&mut candidates);
                candidates.truncate(window);
            }
        }
        sort_job_summaries(&mut candidates);
        candidates.truncate(window);
        Ok((
            candidates.into_iter().skip(offset).take(limit).collect(),
            total,
        ))
    }
}
