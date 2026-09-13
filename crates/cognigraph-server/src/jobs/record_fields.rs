//! Record fields.

pub(super) fn job_record_fields() -> Vec<String> {
    [
        "id",
        "schema_version",
        "tenant",
        "tenant_incarnation",
        "kind",
        "status",
        "idempotency_key_hash",
        "input_digest",
        "actor",
        "attempt",
        "recoveries",
        "created_at_ms",
        "updated_at_ms",
        "started_at_ms",
        "finished_at_ms",
        "archived_at_ms",
        "progress",
        "input",
        "result",
        "error",
        "events",
        "retry_requests",
        "_execution",
    ]
    .map(str::to_string)
    .to_vec()
}
