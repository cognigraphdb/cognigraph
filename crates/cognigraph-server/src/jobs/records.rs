//! Records.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: String,
    pub schema_version: u32,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub kind: JobKind,
    pub status: JobStatus,
    pub idempotency_key_hash: String,
    pub input_digest: String,
    pub actor: JobActor,
    pub attempt: u32,
    pub recoveries: u32,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at_ms: Option<u64>,
    pub progress: JobProgress,
    pub input: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    pub events: Vec<JobEvent>,
    #[serde(default)]
    pub(super) retry_requests: Vec<RetryRequestRecord>,
    #[serde(rename = "_execution")]
    pub(super) payload: JobPayload,
}
impl JobRecord {
    /// Public representation. Internal hashes, retry tokens, and the frozen
    /// execution payload remain protected while the original input and audit
    /// history stay inspectable.
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
            "input": self.input,
            "result": self.result,
            "error": self.error,
            "events": self.events,
        })
    }

    pub(super) fn push_event(
        &mut self,
        event: &str,
        actor: JobActor,
        from_status: Option<JobStatus>,
        to_status: Option<JobStatus>,
        message: Option<String>,
    ) {
        self.events.push(JobEvent {
            sequence: self.events.len() as u64,
            event: event.into(),
            at: now_millis(),
            attempt: self.attempt,
            actor,
            from_status,
            to_status,
            message,
        });
    }
}
