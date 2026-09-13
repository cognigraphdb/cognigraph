//! Receipt.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactConsumptionReceipt {
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub resolver: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub job_id: String,
    pub attempt: u32,
    pub recoveries: u32,
    pub input_digest: String,
    pub execution_payload_digest: String,
    pub eval_spec_digest: String,
    pub evaluation_result_digest: String,
    pub plan_digest: String,
    pub context_digest: String,
    pub artifact_set_digest: String,
    pub corpus: ConsumedArtifactReceipt,
    pub graph: ConsumedArtifactReceipt,
    pub oracle: ConsumedArtifactReceipt,
    pub scorer: ConsumedArtifactReceipt,
    pub verifier: ConsumedArtifactReceipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derivation: Option<Box<CorpusGraphDerivationReceipt>>,
    pub started_at_ms: u64,
    pub completed_at_ms: u64,
    pub material_digest: String,
    pub receipt_digest: String,
}
impl ArtifactConsumptionReceipt {
    pub fn validate(&self, context: &PromotionContext) -> Result<(), CogniGraphError> {
        let plan = context.consumption_plan.as_ref().ok_or_else(|| {
            validation("artifact consumption receipt requires an M21 consumption plan")
        })?;
        let artifacts = context.artifact_attestations.as_ref().ok_or_else(|| {
            validation("artifact consumption receipt requires five artifact attestations")
        })?;
        plan.validate()?;
        artifacts.validate()?;
        let generation_matches = match (
            context.schema_version,
            self.schema_version,
            plan.schema_version,
            self.derivation.as_deref(),
            plan.derivation.as_deref(),
            plan.preparation.as_deref(),
        ) {
            (
                crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION,
                M21_CONSUMPTION_RECEIPT_SCHEMA_VERSION,
                M21_CONSUMPTION_PLAN_SCHEMA_VERSION,
                None,
                None,
                None,
            ) => true,
            (
                crate::promotions::M22_PROMOTION_CONTEXT_SCHEMA_VERSION,
                M22_CONSUMPTION_RECEIPT_SCHEMA_VERSION,
                M22_CONSUMPTION_PLAN_SCHEMA_VERSION,
                Some(derivation),
                Some(derivation_plan),
                None,
            ) => {
                derivation.validate(
                    context,
                    derivation_plan,
                    None,
                    &self.corpus,
                    &self.graph,
                    &self.oracle,
                )?;
                true
            }
            (
                crate::promotions::M23_PROMOTION_CONTEXT_SCHEMA_VERSION,
                M23_CONSUMPTION_RECEIPT_SCHEMA_VERSION,
                M23_CONSUMPTION_PLAN_SCHEMA_VERSION,
                Some(derivation),
                Some(derivation_plan),
                Some(preparation_plan),
            ) => {
                derivation.validate(
                    context,
                    derivation_plan,
                    Some(preparation_plan),
                    &self.corpus,
                    &self.graph,
                    &self.oracle,
                )?;
                true
            }
            _ => false,
        };
        if !generation_matches
            || self.digest_algorithm != DIGEST_ALGORITHM
            || self.resolver != LOCAL_CAS_RESOLVER
            || self.plan_digest != plan.plan_digest
            || self.context_digest != context.digest()?
            || self.artifact_set_digest != artifacts.set_digest
            || self.started_at_ms == 0
            || self.completed_at_ms < self.started_at_ms
            || self.tenant.is_empty()
            || self.tenant_incarnation.is_empty()
            || self.job_id.is_empty()
            || self.attempt == 0
        {
            return Err(validation(
                "artifact consumption receipt has malformed generation, scope, plan, or timestamps",
            ));
        }
        for (label, digest) in [
            ("input_digest", &self.input_digest),
            ("execution_payload_digest", &self.execution_payload_digest),
            ("eval_spec_digest", &self.eval_spec_digest),
            ("evaluation_result_digest", &self.evaluation_result_digest),
        ] {
            validate_digest(label, digest)?;
        }
        if self.eval_spec_digest != context.effective_configuration.eval_spec_digest {
            return Err(validation(
                "artifact consumption receipt EvalSpec digest does not match its context",
            ));
        }
        self.corpus.validate_against(
            &artifacts.corpus,
            ArtifactKind::Corpus,
            ArtifactConsumptionPurpose::CorpusProvenance,
        )?;
        self.graph.validate_against(
            &artifacts.graph,
            ArtifactKind::Graph,
            ArtifactConsumptionPurpose::EvaluatedGraph,
        )?;
        self.oracle.validate_against(
            &artifacts.oracle,
            ArtifactKind::Oracle,
            ArtifactConsumptionPurpose::PromotionOracle,
        )?;
        self.scorer.validate_against(
            &artifacts.scorer,
            ArtifactKind::Scorer,
            ArtifactConsumptionPurpose::ScorerExecutable,
        )?;
        self.verifier.validate_against(
            &artifacts.verifier,
            ArtifactKind::Verifier,
            ArtifactConsumptionPurpose::VerifierExecutable,
        )?;
        validate_digest("material_digest", &self.material_digest)?;
        validate_digest("receipt_digest", &self.receipt_digest)?;
        if self.material_digest != self.expected_material_digest()?
            || self.receipt_digest != record_digest(self, "receipt_digest")?
        {
            return Err(validation("artifact consumption receipt digest mismatch"));
        }
        Ok(())
    }

    pub(crate) fn validate_job_binding(
        &self,
        binding: &ConsumptionJobBinding<'_>,
    ) -> Result<(), CogniGraphError> {
        if self.tenant != binding.tenant
            || self.tenant_incarnation != binding.tenant_incarnation
            || self.job_id != binding.job_id
            || self.attempt != binding.attempt
            || self.recoveries != binding.recoveries
            || self.input_digest != binding.input_digest
            || self.execution_payload_digest != binding.execution_payload_digest
            || self.eval_spec_digest != binding.eval_spec_digest
        {
            return Err(validation(
                "artifact consumption receipt does not match its durable evaluation job",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_evaluation_result(
        &self,
        result_without_receipt: &serde_json::Value,
    ) -> Result<(), CogniGraphError> {
        if self.evaluation_result_digest != canonical_digest(result_without_receipt)? {
            return Err(validation(
                "artifact consumption receipt does not match its evaluation result",
            ));
        }
        if let Some(derivation) = &self.derivation {
            let oracle_artifact = derivation.oracle_artifact()?;
            let facts = derivation
                .facts
                .iter()
                .map(|fact| Fact {
                    source: fact.source.clone(),
                    relation: fact.relation.clone(),
                    target: fact.target.clone(),
                })
                .collect::<HashSet<_>>();
            let outcome = evaluate_facts(
                &oracle_artifact.eval_spec.space_id,
                &oracle_artifact.eval_spec,
                &facts,
            );
            if evaluation_result_value(&outcome) != *result_without_receipt {
                return Err(validation(
                    "derivation receipt facts and signed oracle do not reproduce its evaluation result",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_resolved_eval_spec(
        &self,
        resolved_eval_spec: &EvalSpec,
    ) -> Result<(), CogniGraphError> {
        if let Some(derivation) = &self.derivation {
            let oracle_artifact = derivation.oracle_artifact()?;
            if serde_json::to_value(&oracle_artifact.eval_spec)?
                != serde_json::to_value(resolved_eval_spec)?
            {
                return Err(validation(
                    "derivation receipt oracle EvalSpec does not exactly match its durable job",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn expected_material_digest(&self) -> Result<String, CogniGraphError> {
        let mut material = json!({
            "schema_version": self.schema_version,
            "digest_algorithm": self.digest_algorithm,
            "resolver": self.resolver,
            "plan_digest": self.plan_digest,
            "context_digest": self.context_digest,
            "artifact_set_digest": self.artifact_set_digest,
            "corpus": self.corpus,
            "graph": self.graph,
            "oracle": self.oracle,
            "scorer": self.scorer,
            "verifier": self.verifier,
        });
        if let Some(derivation) = &self.derivation {
            material
                .as_object_mut()
                .expect("receipt material is an object")
                .insert("derivation".into(), serde_json::to_value(derivation)?);
        }
        canonical_digest(&material)
    }
}
