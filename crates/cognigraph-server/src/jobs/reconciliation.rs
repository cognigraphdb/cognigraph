//! Reconciliation.

use super::*;

impl JobManager {
    pub async fn reconcile(
        &self,
        tenant: &str,
        incarnation: &str,
        dry_run: bool,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<ReconcileResult, CogniGraphError> {
        let result = self
            .reconcile_inner(tenant, incarnation, dry_run, limit, cursor)
            .await;
        if let Err(error) = &result
            && !matches!(error, CogniGraphError::ValidationError(_))
        {
            self.mark_catalog_error(tenant, format!("reconciliation: {error}"));
        }
        result
    }

    pub(super) async fn reconcile_inner(
        &self,
        tenant: &str,
        incarnation: &str,
        dry_run: bool,
        limit: usize,
        cursor: Option<&str>,
    ) -> Result<ReconcileResult, CogniGraphError> {
        if !(1..=MAX_JOB_OPERATOR_BATCH).contains(&limit) {
            return Err(CogniGraphError::ValidationError(format!(
                "job reconciliation limit must be 1-{MAX_JOB_OPERATOR_BATCH}"
            )));
        }
        self.ensure_repository(tenant).await?;
        let scope = catalog_scope(tenant, incarnation);
        let fingerprint = digest_bytes(format!("{scope}\0{dry_run}").as_bytes());
        let position = cursor
            .map(|cursor| decode_cursor(cursor, "reconcile", &fingerprint))
            .transpose()?;
        let (phase, after) = parse_reconcile_position(position.as_deref(), &scope)?;
        let mut result = ReconcileResult {
            dry_run,
            phase: phase.into(),
            ..ReconcileResult::default()
        };
        let _guard = self.transition_lock.lock().await;
        match phase {
            "live" | "archive" => {
                let collection = if phase == "live" {
                    JOBS_COLLECTION
                } else {
                    JOB_ARCHIVE_COLLECTION
                };
                let fields = job_record_fields();
                let page = CURRENT_TENANT
                    .scope(
                        tenant.into(),
                        self.raw_backend.list_documents_after_key(
                            collection,
                            after.as_deref(),
                            &fields,
                            limit,
                        ),
                    )
                    .await?;
                let page_len = page.len();
                let mut last_key = after;
                for document in page {
                    let key = document
                        .get("_key")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            CogniGraphError::BackendError(format!(
                                "{collection} reconciliation row has no `_key`"
                            ))
                        })?
                        .to_string();
                    last_key = Some(key);
                    result.scanned += 1;
                    let job: JobRecord = serde_json::from_value(document)?;
                    if job.tenant != tenant || job.tenant_incarnation != incarnation {
                        continue;
                    }
                    self.validate_recoverable_job(tenant, incarnation, &job)?;
                    if phase == "archive" {
                        validate_archive_record(&job)?;
                    }
                    let expected = if phase == "live" {
                        match self
                            .get_raw_from(tenant, JOB_ARCHIVE_COLLECTION, &job.id)
                            .await?
                        {
                            Some(archived) if archive_matches_hot(&job, &archived)? => {
                                result.duplicates_resolved += 1;
                                if !dry_run {
                                    CURRENT_TENANT
                                        .scope(
                                            tenant.into(),
                                            self.raw_backend
                                                .delete_document(JOBS_COLLECTION, &job.id),
                                        )
                                        .await?;
                                }
                                archived
                            }
                            Some(_) => {
                                return Err(CogniGraphError::BackendError(format!(
                                    "job `{}` has an unsafe archive duplicate",
                                    job.id
                                )));
                            }
                            None => job,
                        }
                    } else {
                        job
                    };
                    self.repair_catalog_entry(tenant, &expected, dry_run, &mut result)
                        .await?;
                }
                result.next_cursor = if page_len == limit {
                    last_key.as_deref().map(|key| {
                        encode_scoped_cursor("reconcile", &fingerprint, &format!("{phase}~{key}"))
                    })
                } else {
                    let next = if phase == "live" {
                        "archive~".to_string()
                    } else {
                        format!("catalog~{scope}")
                    };
                    Some(encode_scoped_cursor("reconcile", &fingerprint, &next))
                };
            }
            "catalog" => {
                let fields = ["schema_version", "job", "archived"].map(str::to_string);
                if after.as_deref() == Some(scope.as_str())
                    && CURRENT_TENANT
                        .scope(
                            tenant.into(),
                            self.raw_backend
                                .get_document(JOB_CATALOG_COLLECTION, &scope),
                        )
                        .await?
                        .is_some()
                {
                    result.catalog_deleted += 1;
                    self.mark_catalog_error(
                        tenant,
                        format!("catalog row `{scope}` uses a noncanonical scope key"),
                    );
                    if !dry_run {
                        CURRENT_TENANT
                            .scope(
                                tenant.into(),
                                self.raw_backend
                                    .delete_document(JOB_CATALOG_COLLECTION, &scope),
                            )
                            .await?;
                        self.metrics.catalog_deletes.fetch_add(1, Ordering::Relaxed);
                    }
                }
                let page = CURRENT_TENANT
                    .scope(
                        tenant.into(),
                        self.raw_backend.list_documents_after_key(
                            JOB_CATALOG_COLLECTION,
                            after.as_deref(),
                            &fields,
                            limit,
                        ),
                    )
                    .await?;
                let page_len = page.len();
                let mut last_key = after;
                let mut scope_complete = false;
                for document in page {
                    let key = document
                        .get("_key")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            CogniGraphError::BackendError(
                                "catalog reconciliation row has no `_key`".into(),
                            )
                        })?
                        .to_string();
                    if !key.starts_with(&scope) {
                        scope_complete = true;
                        break;
                    }
                    last_key = Some(key.clone());
                    result.scanned += 1;
                    let catalog: JobCatalogRecord = match serde_json::from_value(document) {
                        Ok(catalog) => catalog,
                        Err(error) => {
                            self.mark_catalog_error(
                                tenant,
                                format!("malformed catalog row `{key}`: {error}"),
                            );
                            result.catalog_deleted += 1;
                            if !dry_run {
                                CURRENT_TENANT
                                    .scope(
                                        tenant.into(),
                                        self.raw_backend
                                            .delete_document(JOB_CATALOG_COLLECTION, &key),
                                    )
                                    .await?;
                                self.metrics.catalog_deletes.fetch_add(1, Ordering::Relaxed);
                            }
                            continue;
                        }
                    };
                    if let Err(error) = validate_catalog_record(&key, &catalog, tenant, incarnation)
                    {
                        self.mark_catalog_error(
                            tenant,
                            format!("invalid catalog row `{key}`: {error}"),
                        );
                    }
                    let live = self.get_raw(tenant, &catalog.job.id).await?.filter(|job| {
                        job.tenant == tenant && job.tenant_incarnation == incarnation
                    });
                    let archived = self
                        .get_raw_from(tenant, JOB_ARCHIVE_COLLECTION, &catalog.job.id)
                        .await?
                        .filter(|job| {
                            job.tenant == tenant && job.tenant_incarnation == incarnation
                        });
                    if let Some(archived) = archived.as_ref() {
                        validate_archive_record(archived)?;
                    }
                    let expected = match (live, archived) {
                        (Some(live), Some(archived)) if archive_matches_hot(&live, &archived)? => {
                            result.duplicates_resolved += 1;
                            if !dry_run {
                                CURRENT_TENANT
                                    .scope(
                                        tenant.into(),
                                        self.raw_backend.delete_document(JOBS_COLLECTION, &live.id),
                                    )
                                    .await?;
                            }
                            Some(archived)
                        }
                        (Some(_), Some(_)) => {
                            return Err(CogniGraphError::BackendError(format!(
                                "job `{}` has an unsafe archive duplicate",
                                catalog.job.id
                            )));
                        }
                        (Some(live), None) => Some(live),
                        (None, Some(archived)) => Some(archived),
                        (None, None) => None,
                    };
                    match expected {
                        Some(expected) => {
                            let canonical_key = catalog_key(&expected);
                            self.repair_catalog_entry(tenant, &expected, dry_run, &mut result)
                                .await?;
                            if key != canonical_key {
                                result.catalog_deleted += 1;
                                if !dry_run {
                                    CURRENT_TENANT
                                        .scope(
                                            tenant.into(),
                                            self.raw_backend
                                                .delete_document(JOB_CATALOG_COLLECTION, &key),
                                        )
                                        .await?;
                                    self.metrics.catalog_deletes.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                        None => {
                            result.catalog_deleted += 1;
                            if !dry_run {
                                CURRENT_TENANT
                                    .scope(
                                        tenant.into(),
                                        self.raw_backend
                                            .delete_document(JOB_CATALOG_COLLECTION, &key),
                                    )
                                    .await?;
                                self.metrics.catalog_deletes.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
                if !scope_complete && page_len == limit {
                    result.next_cursor = last_key.as_deref().map(|key| {
                        encode_scoped_cursor("reconcile", &fingerprint, &format!("catalog~{key}"))
                    });
                }
            }
            _ => unreachable!("validated reconciliation phase"),
        }
        self.metrics.reconciliations.fetch_add(1, Ordering::Relaxed);
        if result.next_cursor.is_none() && !dry_run {
            self.catalog_errors
                .lock()
                .expect("job catalog health lock")
                .remove(tenant);
        }
        self.last_reconciliation
            .lock()
            .expect("job reconciliation lock")
            .insert(tenant.into(), result.clone());
        Ok(result)
    }
}
