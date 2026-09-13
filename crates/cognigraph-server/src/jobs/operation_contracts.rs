//! Operation contracts.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryMode {
    Resume,
    Restart,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RetryRequestRecord {
    pub(super) key_hash: String,
    pub(super) request_digest: String,
}
#[derive(Debug)]
pub struct Submission {
    pub job: JobRecord,
    pub replayed: bool,
}
#[derive(Debug)]
pub struct Mutation {
    pub job: JobRecord,
    pub changed: bool,
}
#[derive(Debug)]
pub struct RetryMutation {
    pub job: JobRecord,
    pub replayed: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct CursorPage {
    pub jobs: Vec<JobSummaryRecord>,
    pub next_cursor: Option<String>,
    pub scanned: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct ArchiveResult {
    pub dry_run: bool,
    pub scanned: usize,
    pub eligible: usize,
    pub archived: usize,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReconcileResult {
    pub dry_run: bool,
    pub phase: String,
    pub scanned: usize,
    pub catalog_created: usize,
    pub catalog_updated: usize,
    pub catalog_deleted: usize,
    pub duplicates_resolved: usize,
    pub next_cursor: Option<String>,
}
