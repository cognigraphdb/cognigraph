//! Catalog repair.

use super::*;

impl JobManager {
    pub(super) async fn reconcile_fully(
        &self,
        tenant: &str,
        incarnation: &str,
    ) -> Result<(), CogniGraphError> {
        let mut cursor = None;
        loop {
            let result = self
                .reconcile(
                    tenant,
                    incarnation,
                    false,
                    MAX_JOB_OPERATOR_BATCH,
                    cursor.as_deref(),
                )
                .await?;
            let Some(next) = result.next_cursor else {
                return Ok(());
            };
            cursor = Some(next);
        }
    }

    pub(super) async fn repair_catalog_entry(
        &self,
        tenant: &str,
        job: &JobRecord,
        dry_run: bool,
        result: &mut ReconcileResult,
    ) -> Result<(), CogniGraphError> {
        let expected = JobCatalogRecord::from_job(job);
        let existing = CURRENT_TENANT
            .scope(
                tenant.into(),
                self.raw_backend
                    .get_document(JOB_CATALOG_COLLECTION, &expected.key),
            )
            .await?;
        match existing {
            None => result.catalog_created += 1,
            Some(document) => {
                match serde_json::from_value::<JobCatalogRecord>(document) {
                    Ok(current) if current == expected => return Ok(()),
                    Ok(_) => {}
                    Err(error) => {
                        self.mark_catalog_error(
                            tenant,
                            format!(
                                "malformed canonical catalog row `{}`: {error}",
                                expected.key
                            ),
                        );
                    }
                }
                result.catalog_updated += 1;
            }
        }
        if !dry_run {
            self.upsert_catalog(tenant, job).await?;
            self.metrics.catalog_upserts.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
}
