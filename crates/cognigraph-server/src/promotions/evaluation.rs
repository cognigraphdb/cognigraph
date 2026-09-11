//! Evaluation.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountMetric {
    pub found: u64,
    pub total: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViolationMetric {
    pub violations: u64,
    pub total: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionEvaluationSource {
    pub job_id: String,
    pub attempt: u32,
    pub recoveries: u32,
    pub actor: Value,
    pub input_digest: String,
    pub execution_payload_digest: String,
    pub eval_spec_digest: String,
    pub context_digest: String,
    pub result_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_consumption: Option<Box<crate::artifact_consumption::ArtifactConsumptionReceipt>>,
    pub context: PromotionContext,
    pub recall: CountMetric,
    pub restraint: ViolationMetric,
    pub missing: Vec<String>,
    pub violations: Vec<String>,
    pub finished_at_ms: u64,
}
impl PromotionEvaluationSource {
    pub fn validate(&self) -> Result<(), CogniGraphError> {
        validate_record_id("job.id", &self.job_id)?;
        if self.attempt == 0 || self.finished_at_ms == 0 {
            return Err(validation(
                "promotion source job attempt and finish timestamp must be positive",
            ));
        }
        validate_digest("job.input_digest", &self.input_digest)?;
        validate_digest(
            "job.execution_payload_digest",
            &self.execution_payload_digest,
        )?;
        validate_digest("job.eval_spec_digest", &self.eval_spec_digest)?;
        validate_digest("job.context_digest", &self.context_digest)?;
        validate_digest("job.result_digest", &self.result_digest)?;
        if self.context.digest()? != self.context_digest {
            return Err(validation("job context digest mismatch"));
        }
        if self.context.effective_configuration.eval_spec_digest != self.eval_spec_digest {
            return Err(validation("job EvalSpec digest mismatch"));
        }
        match (self.context.schema_version, &self.artifact_consumption) {
            (
                M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M23_PROMOTION_CONTEXT_SCHEMA_VERSION,
                Some(receipt),
            ) => {
                receipt.validate(&self.context)?;
                receipt.validate_job_binding(
                    &crate::artifact_consumption::ConsumptionJobBinding {
                        tenant: &receipt.tenant,
                        tenant_incarnation: &receipt.tenant_incarnation,
                        job_id: &self.job_id,
                        attempt: self.attempt,
                        recoveries: self.recoveries,
                        input_digest: &self.input_digest,
                        execution_payload_digest: &self.execution_payload_digest,
                        eval_spec_digest: &self.eval_spec_digest,
                    },
                )?;
            }
            (
                M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M23_PROMOTION_CONTEXT_SCHEMA_VERSION,
                None,
            ) => {
                return Err(validation(
                    "M21-M23 evaluation source requires an artifact consumption receipt",
                ));
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(validation(
                    "pre-M21 evaluation source cannot carry an artifact consumption receipt",
                ));
            }
        }
        if self.recall.found > self.recall.total || self.restraint.violations > self.restraint.total
        {
            return Err(validation("job result counts are out of range"));
        }
        if self.missing.len() as u64 != self.recall.total - self.recall.found
            || self.violations.len() as u64 != self.restraint.violations
        {
            return Err(validation(
                "job result lists do not match recall/restraint counts",
            ));
        }
        if !is_sorted_unique(&self.missing) || !is_sorted_unique(&self.violations) {
            return Err(validation(
                "job missing/violation fact lists must be sorted and unique",
            ));
        }
        if let Some(receipt) = &self.artifact_consumption {
            let result = json!({
                "space_type": self.context.target.space_type,
                "recall": { "found": self.recall.found, "total": self.recall.total },
                "restraint": {
                    "violations": self.restraint.violations,
                    "total": self.restraint.total,
                },
                "recall_ok": self.recall.found == self.recall.total,
                "restraint_ok": self.restraint.violations == 0,
                "missing": self.missing,
                "violations": self.violations,
            });
            receipt.validate_evaluation_result(&result)?;
            let mut result_with_receipt = result;
            result_with_receipt
                .as_object_mut()
                .expect("M21 evaluation projection is an object")
                .insert(
                    "artifact_consumption".into(),
                    serde_json::to_value(receipt)?,
                );
            if canonical_digest(&result_with_receipt)? != self.result_digest {
                return Err(validation(
                    "job result digest does not match its M21 score projection and receipt",
                ));
            }
        }
        self.context.validate(
            &self.context.target.space_type,
            &self.eval_spec_digest,
            self.recall.total as usize,
            self.restraint.total as usize,
        )?;
        Ok(())
    }
}
