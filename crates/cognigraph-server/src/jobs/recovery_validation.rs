//! Recovery validation.

use super::*;

impl JobManager {
    pub(super) fn validate_recoverable_job(
        &self,
        tenant: &str,
        incarnation: &str,
        job: &JobRecord,
    ) -> Result<(), CogniGraphError> {
        match (&job.payload, job.schema_version) {
            (
                JobPayload::Evaluate {
                    eval,
                    promotion_context: Some(context),
                },
                schema_version,
            ) if matches!(
                (schema_version, context.schema_version),
                (
                    M21_JOB_SCHEMA_VERSION,
                    crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                ) | (
                    M22_JOB_SCHEMA_VERSION,
                    crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                ) | (
                    M23_JOB_SCHEMA_VERSION,
                    crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION
                )
            ) =>
            {
                let (expected, forbidden) = validate_promotion_eval_spec(eval)?;
                let eval_digest = crate::promotions::canonical_digest(eval)?;
                context.validate(&eval.space_id, &eval_digest, expected, forbidden)?;
                context.validate_runtime_backend(self.raw_backend.backend_name())?;
                if job.status == JobStatus::Succeeded {
                    self.promotion_evaluation_source_from_job(tenant, incarnation, job.clone())?;
                } else if job
                    .result
                    .as_ref()
                    .and_then(|result| result.get("artifact_consumption"))
                    .is_some()
                {
                    return Err(CogniGraphError::DocumentConflict(format!(
                        "non-succeeded verified-artifact job `{}` carries a consumption receipt",
                        job.id
                    )));
                }
            }
            (_, JOB_SCHEMA_VERSION) => {
                if matches!(
                    &job.payload,
                    JobPayload::Evaluate {
                        promotion_context: Some(context),
                        ..
                    } if matches!(
                        context.schema_version,
                        crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                            | crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                            | crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION
                    )
                ) {
                    return Err(CogniGraphError::DocumentConflict(format!(
                        "legacy job `{}` carries a verified-artifact promotion context",
                        job.id
                    )));
                }
                if job
                    .result
                    .as_ref()
                    .and_then(|result| result.get("artifact_consumption"))
                    .is_some()
                {
                    return Err(CogniGraphError::DocumentConflict(format!(
                        "legacy job `{}` carries a verified-artifact consumption receipt",
                        job.id
                    )));
                }
            }
            _ => {
                return Err(CogniGraphError::DocumentConflict(format!(
                    "job `{}` has unsupported schema/payload generation",
                    job.id
                )));
            }
        }
        Ok(())
    }

    /// Validate one incoming Native snapshot job before any backend import.
    ///
    /// M21-M23 job schemas are potential promotion authority even when no evidence
    /// references it yet, so hot and archived records must fail closed on
    /// malformed contexts, receipts, or result bindings at preflight time.
    pub(crate) fn validate_snapshot_job_value(
        &self,
        tenant: &str,
        incarnation: &str,
        key: &str,
        value: &Value,
        archived: bool,
    ) -> Result<JobRecord, CogniGraphError> {
        let job: JobRecord = serde_json::from_value(value.clone()).map_err(|error| {
            CogniGraphError::DocumentConflict(format!("snapshot job `{key}` is malformed: {error}"))
        })?;
        if job.id != key || job.tenant != tenant || job.tenant_incarnation != incarnation {
            return Err(CogniGraphError::DocumentConflict(format!(
                "snapshot job `{key}` has a foreign or mismatched identity"
            )));
        }
        if archived {
            validate_archive_record(&job)?;
        } else if job.archived_at_ms.is_some() {
            return Err(CogniGraphError::DocumentConflict(format!(
                "hot snapshot job `{key}` is marked archived"
            )));
        }
        self.validate_recoverable_job(tenant, incarnation, &job)?;
        Ok(job)
    }
}
