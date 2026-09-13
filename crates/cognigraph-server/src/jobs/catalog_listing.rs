//! Catalog listing.

use super::*;

impl JobManager {
    pub(super) async fn list_summary_page(
        &self,
        tenant: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<JobSummaryRecord>, CogniGraphError> {
        let fields = [
            "id",
            "schema_version",
            "tenant",
            "tenant_incarnation",
            "kind",
            "status",
            "actor",
            "attempt",
            "recoveries",
            "created_at_ms",
            "updated_at_ms",
            "started_at_ms",
            "finished_at_ms",
            "progress",
        ]
        .map(str::to_string);
        let result = CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend.list_documents_projected(
                    JOBS_COLLECTION,
                    &fields,
                    Some(limit),
                    Some(offset),
                ),
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

    pub(super) fn record_data_error<T>(&self, tenant: &str, result: &Result<T, CogniGraphError>) {
        if let Err(error) = result
            && !matches!(error, CogniGraphError::DocumentConflict(_))
        {
            self.data_errors
                .lock()
                .expect("job data health lock")
                .insert(tenant.into(), error.to_string());
        }
    }

    pub(super) fn mark_catalog_error(&self, tenant: &str, error: String) {
        self.catalog_errors
            .lock()
            .expect("job catalog health lock")
            .insert(tenant.into(), error);
    }
}
