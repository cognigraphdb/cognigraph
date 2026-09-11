//! Contracts.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    #[serde(rename = "construct.ingest")]
    ConstructIngest,
    #[serde(rename = "construct.evaluate")]
    ConstructEvaluate,
    #[serde(rename = "construct.draft")]
    ConstructDraft,
    #[serde(rename = "sideviews.generate")]
    SideviewsGenerate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    CancelRequested,
    Succeeded,
    Failed,
    Canceled,
}
impl JobStatus {
    pub(crate) fn terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveFilter {
    #[default]
    Exclude,
    Include,
    Only,
}
impl ArchiveFilter {
    pub(super) fn matches(self, archived: bool) -> bool {
        match self {
            Self::Exclude => !archived,
            Self::Include => true,
            Self::Only => archived,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    User,
    Anonymous,
    System,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobActor {
    #[serde(rename = "type")]
    pub actor_type: ActorType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_key: Option<String>,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}
impl JobActor {
    pub fn request(user: Option<&User>) -> Self {
        match user {
            Some(user) => Self {
                actor_type: ActorType::User,
                user_key: Some(user.key.clone()),
                username: user.username.clone(),
                role: Some(user.role),
            },
            None => Self {
                actor_type: ActorType::Anonymous,
                user_key: None,
                username: "anonymous".into(),
                role: None,
            },
        }
    }

    pub(super) fn worker() -> Self {
        Self {
            actor_type: ActorType::System,
            user_key: None,
            username: "worker".into(),
            role: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub sequence: u64,
    pub event: String,
    pub at: u64,
    pub attempt: u32,
    pub actor: JobActor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_status: Option<JobStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_status: Option<JobStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobProgress {
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    #[serde(default)]
    pub facts_grounded: usize,
    /// Side-views written so far by a `sideviews.generate` job. Zero for every
    /// other kind; `#[serde(default)]` keeps pre-existing durable records
    /// deserializable.
    #[serde(default)]
    pub side_views_written: usize,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobSummaryRecord {
    pub(super) id: String,
    pub(super) schema_version: u32,
    pub(super) tenant: String,
    pub(super) tenant_incarnation: String,
    pub(super) kind: JobKind,
    pub(super) status: JobStatus,
    pub(super) actor: JobActor,
    pub(super) attempt: u32,
    pub(super) recoveries: u32,
    pub(super) created_at_ms: u64,
    pub(super) updated_at_ms: u64,
    pub(super) started_at_ms: Option<u64>,
    pub(super) finished_at_ms: Option<u64>,
    #[serde(default)]
    pub(super) archived_at_ms: Option<u64>,
    pub(super) progress: JobProgress,
}
impl JobSummaryRecord {
    pub fn public_value(&self) -> Value {
        json!({
            "id": self.id,
            "schema_version": self.schema_version,
            "tenant": self.tenant,
            "kind": self.kind,
            "status": self.status,
            "actor": self.actor,
            "attempt": self.attempt,
            "recoveries": self.recoveries,
            "created_at": self.created_at_ms,
            "updated_at": self.updated_at_ms,
            "started_at": self.started_at_ms,
            "finished_at": self.finished_at_ms,
            "archived_at": self.archived_at_ms,
            "progress": self.progress,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct JobCatalogRecord {
    #[serde(rename = "_key")]
    pub(super) key: String,
    pub(super) schema_version: u32,
    pub(super) job: JobSummaryRecord,
    pub(super) archived: bool,
}
impl JobCatalogRecord {
    pub(super) fn from_job(job: &JobRecord) -> Self {
        Self {
            key: catalog_key(job),
            schema_version: JOB_CATALOG_SCHEMA_VERSION,
            job: JobSummaryRecord::from(job),
            archived: job.archived_at_ms.is_some(),
        }
    }
}
impl From<&JobRecord> for JobSummaryRecord {
    fn from(job: &JobRecord) -> Self {
        Self {
            id: job.id.clone(),
            schema_version: job.schema_version,
            tenant: job.tenant.clone(),
            tenant_incarnation: job.tenant_incarnation.clone(),
            kind: job.kind,
            status: job.status,
            actor: job.actor.clone(),
            attempt: job.attempt,
            recoveries: job.recoveries,
            created_at_ms: job.created_at_ms,
            updated_at_ms: job.updated_at_ms,
            started_at_ms: job.started_at_ms,
            finished_at_ms: job.finished_at_ms,
            archived_at_ms: job.archived_at_ms,
            progress: job.progress.clone(),
        }
    }
}
