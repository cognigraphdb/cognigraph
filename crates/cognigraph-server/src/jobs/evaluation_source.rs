//! Evaluation source.

use super::*;

impl JobManager {
    pub async fn get(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<JobRecord, CogniGraphError> {
        self.ensure_repository(tenant).await?;
        let job = self.get_raw_any(tenant, id).await?.ok_or_else(|| {
            CogniGraphError::DocumentNotFound {
                collection: "jobs".into(),
                key: id.into(),
            }
        })?;
        if job.tenant != tenant || job.tenant_incarnation != incarnation {
            return Err(CogniGraphError::DocumentNotFound {
                collection: "jobs".into(),
                key: id.into(),
            });
        }
        Ok(job)
    }

    /// Load the authoritative promotion-grade projection of one succeeded
    /// deterministic evaluation. Legacy jobs and nonterminal/other job kinds
    /// fail closed; callers never supply score projections themselves.
    pub async fn promotion_evaluation_source(
        &self,
        tenant: &str,
        incarnation: &str,
        id: &str,
    ) -> Result<crate::promotions::PromotionEvaluationSource, CogniGraphError> {
        let job = self.get(tenant, incarnation, id).await?;
        self.promotion_evaluation_source_from_job(tenant, incarnation, job)
    }

    pub(crate) fn promotion_evaluation_source_from_snapshot_value(
        &self,
        tenant: &str,
        incarnation: &str,
        value: Value,
        archived: bool,
    ) -> Result<crate::promotions::PromotionEvaluationSource, CogniGraphError> {
        let job: JobRecord = serde_json::from_value(value)?;
        if job.tenant != tenant || job.tenant_incarnation != incarnation {
            return Err(CogniGraphError::DocumentConflict(
                "snapshot promotion source job belongs to another tenant incarnation".into(),
            ));
        }
        if archived {
            validate_archive_record(&job)?;
        }
        self.promotion_evaluation_source_from_job(tenant, incarnation, job)
    }

    pub(super) fn promotion_evaluation_source_from_job(
        &self,
        tenant: &str,
        incarnation: &str,
        job: JobRecord,
    ) -> Result<crate::promotions::PromotionEvaluationSource, CogniGraphError> {
        let id = job.id.clone();
        if job.tenant != tenant || job.tenant_incarnation != incarnation {
            return Err(CogniGraphError::DocumentConflict(format!(
                "job `{id}` belongs to another tenant incarnation"
            )));
        }
        if job.kind != JobKind::ConstructEvaluate || job.status != JobStatus::Succeeded {
            return Err(CogniGraphError::DocumentConflict(format!(
                "job `{id}` is not a succeeded construct.evaluate job"
            )));
        }
        let JobPayload::Evaluate {
            eval,
            promotion_context: Some(context),
        } = &job.payload
        else {
            return Err(CogniGraphError::DocumentConflict(format!(
                "job `{id}` has no frozen M18 PromotionContext"
            )));
        };
        match (job.schema_version, context.schema_version) {
            (M23_JOB_SCHEMA_VERSION, crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION)
            | (M22_JOB_SCHEMA_VERSION, crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION)
            | (M21_JOB_SCHEMA_VERSION, crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION)
            | (JOB_SCHEMA_VERSION, crate::promotions::PROMOTION_CONTEXT_SCHEMA_VERSION)
            | (JOB_SCHEMA_VERSION, crate::promotions::M19_PROMOTION_CONTEXT_SCHEMA_VERSION)
            | (JOB_SCHEMA_VERSION, crate::promotions::M20_PROMOTION_CONTEXT_SCHEMA_VERSION) => {}
            _ => {
                return Err(CogniGraphError::DocumentConflict(format!(
                    "job `{id}` schema does not match its promotion-context generation"
                )));
            }
        }
        let (expected_distinct, forbidden_distinct) = validate_promotion_eval_spec(eval)?;
        let eval_spec_digest = crate::promotions::canonical_digest(eval)?;
        context.validate(
            &eval.space_id,
            &eval_spec_digest,
            expected_distinct,
            forbidden_distinct,
        )?;
        context.validate_runtime_backend(self.raw_backend.backend_name())?;
        let result = job.result.as_ref().ok_or_else(|| {
            CogniGraphError::DocumentConflict(format!(
                "succeeded evaluation job `{id}` has no result"
            ))
        })?;
        let artifact_consumption: Option<
            Box<crate::artifact_consumption::ArtifactConsumptionReceipt>,
        > = result
            .get("artifact_consumption")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| {
                CogniGraphError::DocumentConflict(format!(
                    "evaluation job `{id}` has malformed artifact consumption receipt: {error}"
                ))
            })?;
        if artifact_consumption.as_deref().is_some_and(|receipt| {
            receipt.tenant != tenant || receipt.tenant_incarnation != incarnation
        }) {
            return Err(CogniGraphError::DocumentConflict(format!(
                "evaluation job `{id}` consumption receipt belongs to another tenant incarnation"
            )));
        }
        if let Some(receipt) = &artifact_consumption {
            receipt.validate_resolved_eval_spec(eval)?;
            let mut evaluation_result = result.clone();
            evaluation_result
                .as_object_mut()
                .ok_or_else(|| {
                    CogniGraphError::DocumentConflict(format!(
                        "evaluation job `{id}` result is not an object"
                    ))
                })?
                .remove("artifact_consumption");
            receipt.validate_evaluation_result(&evaluation_result)?;
        }
        let count = |pointer: &str| {
            result
                .pointer(pointer)
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    CogniGraphError::DocumentConflict(format!(
                        "evaluation job `{id}` has malformed result field `{pointer}`"
                    ))
                })
        };
        let strings = |field: &str| {
            result
                .get(field)
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    CogniGraphError::DocumentConflict(format!(
                        "evaluation job `{id}` has malformed result field `{field}`"
                    ))
                })?
                .iter()
                .map(|value| {
                    value.as_str().map(str::to_string).ok_or_else(|| {
                        CogniGraphError::DocumentConflict(format!(
                            "evaluation job `{id}` result `{field}` contains a non-string"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        };
        if result.get("space_type").and_then(Value::as_str) != Some(eval.space_id.as_str()) {
            return Err(CogniGraphError::DocumentConflict(format!(
                "evaluation job `{id}` result space does not match its frozen EvalSpec"
            )));
        }
        let context_digest = context.digest()?;
        let execution_payload_digest = crate::promotions::canonical_digest(&job.payload)?;
        let result_digest = crate::promotions::canonical_digest(result)?;
        let recomputed_input_digest = digest_json(&json!({
            "kind": &job.kind,
            "input": &job.input,
        }))?;
        let stored_input_digest = job
            .input_digest
            .strip_prefix("sha256:")
            .unwrap_or(&job.input_digest);
        if stored_input_digest != recomputed_input_digest {
            let error = CogniGraphError::BackendError(format!(
                "authoritative evaluation job `{id}` input digest mismatch"
            ));
            self.data_errors
                .lock()
                .expect("job data health lock")
                .insert(tenant.into(), error.to_string());
            return Err(error);
        }
        let input_digest = if job.input_digest.starts_with("sha256:") {
            job.input_digest.clone()
        } else {
            format!("sha256:{}", job.input_digest)
        };
        let source = crate::promotions::PromotionEvaluationSource {
            job_id: job.id,
            attempt: job.attempt,
            recoveries: job.recoveries,
            actor: serde_json::to_value(&job.actor)?,
            input_digest,
            execution_payload_digest,
            eval_spec_digest,
            context_digest,
            result_digest,
            artifact_consumption,
            context: context.as_ref().clone(),
            recall: crate::promotions::CountMetric {
                found: count("/recall/found")?,
                total: count("/recall/total")?,
            },
            restraint: crate::promotions::ViolationMetric {
                violations: count("/restraint/violations")?,
                total: count("/restraint/total")?,
            },
            missing: crate::promotions::sorted_fact_lines(strings("missing")?),
            violations: crate::promotions::sorted_fact_lines(strings("violations")?),
            finished_at_ms: job.finished_at_ms.ok_or_else(|| {
                CogniGraphError::DocumentConflict(format!(
                    "succeeded evaluation job `{id}` has no finished timestamp"
                ))
            })?,
        };
        source.validate()?;
        Ok(source)
    }
}
