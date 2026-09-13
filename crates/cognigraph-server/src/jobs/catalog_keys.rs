//! Catalog keys.

use super::*;

pub(super) fn catalog_scope(tenant: &str, incarnation: &str) -> String {
    digest_bytes(format!("{tenant}\0{incarnation}").as_bytes())
}
pub(super) fn catalog_key(job: &JobRecord) -> String {
    catalog_key_fields(
        &job.tenant,
        &job.tenant_incarnation,
        job.created_at_ms,
        &job.id,
    )
}
pub(super) fn catalog_key_from_summary(job: &JobSummaryRecord) -> String {
    catalog_key_fields(
        &job.tenant,
        &job.tenant_incarnation,
        job.created_at_ms,
        &job.id,
    )
}
pub(super) fn catalog_key_fields(
    tenant: &str,
    incarnation: &str,
    created_at_ms: u64,
    id: &str,
) -> String {
    format!(
        "{}-{:016x}-{}",
        catalog_scope(tenant, incarnation),
        u64::MAX - created_at_ms,
        digest_bytes(id.as_bytes())
    )
}
pub(super) fn validate_catalog_record(
    scanned_key: &str,
    record: &JobCatalogRecord,
    tenant: &str,
    incarnation: &str,
) -> Result<(), String> {
    if record.key != scanned_key {
        return Err("embedded and scanned keys differ".into());
    }
    if record.schema_version != JOB_CATALOG_SCHEMA_VERSION {
        return Err(format!(
            "unsupported catalog schema version {}",
            record.schema_version
        ));
    }
    if !matches!(
        record.job.schema_version,
        JOB_SCHEMA_VERSION
            | M21_JOB_SCHEMA_VERSION
            | M22_JOB_SCHEMA_VERSION
            | M23_JOB_SCHEMA_VERSION
    ) {
        return Err(format!(
            "unsupported job schema version {}",
            record.job.schema_version
        ));
    }
    if record.job.tenant != tenant || record.job.tenant_incarnation != incarnation {
        return Err("catalog row belongs to another tenant incarnation".into());
    }
    if record.archived != record.job.archived_at_ms.is_some() {
        return Err("archive marker disagrees with the projected job summary".into());
    }
    if catalog_key_from_summary(&record.job) != scanned_key {
        return Err("catalog row is stored under a noncanonical key".into());
    }
    Ok(())
}
