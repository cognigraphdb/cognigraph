//! Archive validation.

use super::*;

pub(super) fn sort_job_summaries(jobs: &mut [JobSummaryRecord]) {
    jobs.sort_by(|left, right| {
        right
            .created_at_ms
            .cmp(&left.created_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
}
pub(crate) fn validate_archive_record(job: &JobRecord) -> Result<(), CogniGraphError> {
    if !matches!(
        job.schema_version,
        JOB_SCHEMA_VERSION
            | M21_JOB_SCHEMA_VERSION
            | M22_JOB_SCHEMA_VERSION
            | M23_JOB_SCHEMA_VERSION
    ) {
        return Err(CogniGraphError::BackendError(format!(
            "archive job `{}` has unsupported schema version {}",
            job.id, job.schema_version
        )));
    }
    let Some(archived_at) = job.archived_at_ms else {
        return Err(CogniGraphError::BackendError(format!(
            "archive job `{}` is unmarked",
            job.id
        )));
    };
    if !job.status.terminal() {
        return Err(CogniGraphError::BackendError(format!(
            "archive job `{}` is nonterminal",
            job.id
        )));
    }
    let valid_event = job.events.last().is_some_and(|event| {
        event.event == "archived"
            && event.sequence == job.events.len().saturating_sub(1) as u64
            && event.at >= archived_at
            && event.from_status == Some(job.status)
            && event.to_status == Some(job.status)
    });
    if !valid_event {
        return Err(CogniGraphError::BackendError(format!(
            "archive job `{}` has no valid terminal archive event",
            job.id
        )));
    }
    Ok(())
}
pub(super) fn normalized_archive_lineage(job: &JobRecord) -> Result<Value, CogniGraphError> {
    let mut normalized = job.clone();
    normalized.archived_at_ms = None;
    normalized.updated_at_ms = 0;
    if normalized
        .events
        .last()
        .is_some_and(|event| event.event == "archived")
    {
        normalized.events.pop();
    }
    serde_json::to_value(normalized).map_err(CogniGraphError::from)
}
pub(super) fn same_archived_lineage(
    left: &JobRecord,
    right: &JobRecord,
) -> Result<bool, CogniGraphError> {
    validate_archive_record(left)?;
    validate_archive_record(right)?;
    Ok(normalized_archive_lineage(left)? == normalized_archive_lineage(right)?)
}
pub(crate) fn archive_matches_hot(
    hot: &JobRecord,
    archive: &JobRecord,
) -> Result<bool, CogniGraphError> {
    validate_archive_record(archive)?;
    if !hot.status.terminal() || hot.archived_at_ms.is_some() {
        return Ok(false);
    }
    Ok(normalized_archive_lineage(hot)? == normalized_archive_lineage(archive)?)
}
