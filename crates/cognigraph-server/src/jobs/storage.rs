//! Storage.

use super::*;

impl JobManager {
    pub(super) async fn ensure_repository(&self, tenant: &str) -> Result<(), CogniGraphError> {
        let result: Result<(), CogniGraphError> = CURRENT_TENANT
            .scope(tenant.into(), async {
                for collection in [
                    JOBS_COLLECTION,
                    JOB_ARCHIVE_COLLECTION,
                    JOB_CATALOG_COLLECTION,
                ] {
                    self.raw_backend
                        .ensure_collection(collection, CollectionType::Document)
                        .await?;
                }
                Ok(())
            })
            .await;
        match result {
            Ok(()) => {
                self.collection_errors
                    .lock()
                    .expect("job collection health lock")
                    .remove(tenant);
                Ok(())
            }
            Err(error) => {
                self.collection_errors
                    .lock()
                    .expect("job collection health lock")
                    .insert(tenant.into(), error.to_string());
                Err(error)
            }
        }
    }

    pub(super) async fn create_raw(
        &self,
        tenant: &str,
        job: &JobRecord,
    ) -> Result<(), CogniGraphError> {
        let mut document = serde_json::to_value(job)?;
        document["_key"] = json!(job.id);
        let result = CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend.create_document(JOBS_COLLECTION, document),
            )
            .await
            .map(|_| ());
        self.record_data_error(tenant, &result);
        if result.is_ok() {
            self.sync_catalog_best_effort(tenant, job).await;
        }
        result
    }

    pub(super) async fn save_raw(
        &self,
        tenant: &str,
        job: &JobRecord,
    ) -> Result<(), CogniGraphError> {
        let mut document = serde_json::to_value(job)?;
        document["_key"] = json!(job.id);
        let result = CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend
                    .replace_document(JOBS_COLLECTION, &job.id, document),
            )
            .await
            .map(|_| ());
        self.record_data_error(tenant, &result);
        if result.is_ok() {
            self.sync_catalog_best_effort(tenant, job).await;
        }
        result
    }

    pub(super) async fn get_raw(
        &self,
        tenant: &str,
        id: &str,
    ) -> Result<Option<JobRecord>, CogniGraphError> {
        self.get_raw_from(tenant, JOBS_COLLECTION, id).await
    }

    pub(super) async fn get_raw_any(
        &self,
        tenant: &str,
        id: &str,
    ) -> Result<Option<JobRecord>, CogniGraphError> {
        // Copy-first archival can intentionally leave both records present if
        // deleting the hot copy fails. Prefer the immutable archive so public
        // lifecycle operations fail closed until reconciliation removes the
        // duplicate.
        if let Some(job) = self
            .get_raw_from(tenant, JOB_ARCHIVE_COLLECTION, id)
            .await?
        {
            validate_archive_record(&job)?;
            return Ok(Some(job));
        }
        self.get_raw(tenant, id).await
    }

    pub(super) async fn get_raw_from(
        &self,
        tenant: &str,
        collection: &str,
        id: &str,
    ) -> Result<Option<JobRecord>, CogniGraphError> {
        let result = CURRENT_TENANT
            .scope(tenant.into(), self.raw_backend.get_document(collection, id))
            .await
            .and_then(|document| {
                document
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(CogniGraphError::from)
            });
        self.record_data_error(tenant, &result);
        result
    }

    pub(super) async fn sync_catalog_best_effort(&self, tenant: &str, job: &JobRecord) {
        if let Err(error) = self.upsert_catalog(tenant, job).await {
            tracing::warn!(tenant, job_id = job.id, %error, "job catalog update deferred");
            self.catalog_errors
                .lock()
                .expect("job catalog health lock")
                .insert(tenant.into(), error.to_string());
        }
    }

    pub(super) async fn upsert_catalog(
        &self,
        tenant: &str,
        job: &JobRecord,
    ) -> Result<(), CogniGraphError> {
        let catalog = JobCatalogRecord::from_job(job);
        let key = catalog.key.clone();
        let document = serde_json::to_value(catalog)?;
        let existing = CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend.get_document(JOB_CATALOG_COLLECTION, &key),
            )
            .await?;
        let result: Result<(), CogniGraphError> = if existing.is_some() {
            CURRENT_TENANT
                .scope(
                    tenant.into(),
                    self.raw_backend
                        .replace_document(JOB_CATALOG_COLLECTION, &key, document),
                )
                .await
                .map(|_| ())
        } else {
            match CURRENT_TENANT
                .scope(
                    tenant.into(),
                    self.raw_backend
                        .create_document(JOB_CATALOG_COLLECTION, document.clone()),
                )
                .await
            {
                Err(CogniGraphError::DocumentConflict(_)) => CURRENT_TENANT
                    .scope(
                        tenant.into(),
                        self.raw_backend
                            .replace_document(JOB_CATALOG_COLLECTION, &key, document),
                    )
                    .await
                    .map(|_| ()),
                Ok(_) => Ok(()),
                Err(error) => Err(error),
            }
        };
        result
    }

    pub(super) async fn copy_to_archive(
        &self,
        tenant: &str,
        job: &JobRecord,
    ) -> Result<JobRecord, CogniGraphError> {
        let mut document = serde_json::to_value(job)?;
        document["_key"] = json!(job.id);
        match CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend
                    .create_document(JOB_ARCHIVE_COLLECTION, document),
            )
            .await
        {
            Ok(_) => Ok(job.clone()),
            Err(CogniGraphError::DocumentConflict(_)) => {
                let existing = self
                .get_raw_from(tenant, JOB_ARCHIVE_COLLECTION, &job.id)
                .await?
                .ok_or_else(|| {
                    CogniGraphError::BackendError(format!(
                        "archive insert for job `{}` conflicted but no archive record is readable",
                        job.id
                    ))
                })?;
                validate_archive_record(&existing)?;
                if !same_archived_lineage(job, &existing)? {
                    return Err(CogniGraphError::BackendError(format!(
                        "archive conflict for job `{}` does not match the authoritative job",
                        job.id
                    )));
                }
                Ok(existing)
            }
            Err(error) => Err(error),
        }
    }

    pub(super) async fn list_raw(&self, tenant: &str) -> Result<Vec<JobRecord>, CogniGraphError> {
        let result = CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend.list_documents(JOBS_COLLECTION, None, None),
            )
            .await
            .and_then(|documents| {
                documents
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(CogniGraphError::from)
            });
        self.record_data_error(tenant, &result);
        result
    }
}
