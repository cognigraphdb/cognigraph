//! Context.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionContext {
    pub schema_version: u32,
    pub target: PromotionTarget,
    pub candidate: CandidateIdentity,
    pub effective_configuration: EffectiveConfiguration,
    pub revisions: RevisionSet,
    pub case_manifest: CaseManifest,
    pub policy: ResolvedPromotionPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<PolicyGovernanceBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_attestations: Option<crate::artifact_attestations::ArtifactAttestationSet>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumption_plan: Option<Box<crate::artifact_consumption::ArtifactConsumptionPlan>>,
    pub reproducibility: ReproducibilityContext,
    pub oracle_separation: OracleSeparationAttestation,
}
impl PromotionContext {
    pub fn validate(
        &self,
        space_type: &str,
        eval_spec_digest: &str,
        expected_distinct: usize,
        forbidden_distinct: usize,
    ) -> Result<(), CogniGraphError> {
        if !matches!(
            self.schema_version,
            PROMOTION_CONTEXT_SCHEMA_VERSION
                | M19_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M20_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M23_PROMOTION_CONTEXT_SCHEMA_VERSION
        ) {
            return Err(validation(format!(
                "unsupported promotion context schema version {}",
                self.schema_version
            )));
        }
        match (
            self.schema_version,
            &self.governance,
            &self.artifact_attestations,
        ) {
            (PROMOTION_CONTEXT_SCHEMA_VERSION, None, None) => {}
            (M19_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(binding), None) => binding.validate()?,
            (
                M20_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M23_PROMOTION_CONTEXT_SCHEMA_VERSION,
                Some(binding),
                Some(artifacts),
            ) => {
                binding.validate()?;
                artifacts.validate()?;
                crate::artifact_attestations::validate_context_subject_bindings(self, artifacts)?;
            }
            (PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_), _) => {
                return Err(validation(
                    "M18 promotion contexts cannot carry an M19 governance binding",
                ));
            }
            (M19_PROMOTION_CONTEXT_SCHEMA_VERSION, None, _) => {
                return Err(validation(
                    "M19 promotion contexts require an exact policy approval binding",
                ));
            }
            (M19_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_), Some(_)) => {
                return Err(validation(
                    "M19 promotion contexts cannot carry M20 artifact attestations",
                ));
            }
            (M20_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_), None) => {
                return Err(validation(
                    "M20 promotion contexts require exact five-slot artifact attestations",
                ));
            }
            (M20_PROMOTION_CONTEXT_SCHEMA_VERSION, None, _) => {
                return Err(validation(
                    "M20 promotion contexts require signed policy governance",
                ));
            }
            (M21_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_), None) => {
                return Err(validation(
                    "M21 promotion contexts require exact five-slot artifact attestations",
                ));
            }
            (M21_PROMOTION_CONTEXT_SCHEMA_VERSION, None, _) => {
                return Err(validation(
                    "M21 promotion contexts require signed policy governance",
                ));
            }
            (M22_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_), None) => {
                return Err(validation(
                    "M22 promotion contexts require exact five-slot artifact attestations",
                ));
            }
            (M22_PROMOTION_CONTEXT_SCHEMA_VERSION, None, _) => {
                return Err(validation(
                    "M22 promotion contexts require signed policy governance",
                ));
            }
            (M23_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_), None) => {
                return Err(validation(
                    "M23 promotion contexts require exact five-slot artifact attestations",
                ));
            }
            (M23_PROMOTION_CONTEXT_SCHEMA_VERSION, None, _) => {
                return Err(validation(
                    "M23 promotion contexts require signed policy governance",
                ));
            }
            (PROMOTION_CONTEXT_SCHEMA_VERSION, None, Some(_)) => {
                return Err(validation(
                    "M18 promotion contexts cannot carry M20 artifact attestations",
                ));
            }
            _ => unreachable!("schema version was checked above"),
        }
        match (self.schema_version, &self.consumption_plan) {
            (M21_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(plan))
                if plan.schema_version
                    == crate::artifact_consumption::M21_CONSUMPTION_PLAN_SCHEMA_VERSION =>
            {
                plan.validate()?
            }
            (M21_PROMOTION_CONTEXT_SCHEMA_VERSION, None) => {
                return Err(validation(
                    "M21 promotion contexts require a pinned artifact consumption plan",
                ));
            }
            (M21_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_)) => {
                return Err(validation(
                    "M21 promotion contexts require consumption plan generation 1",
                ));
            }
            (M22_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(plan))
                if plan.schema_version
                    == crate::artifact_consumption::M22_CONSUMPTION_PLAN_SCHEMA_VERSION =>
            {
                plan.validate()?
            }
            (M22_PROMOTION_CONTEXT_SCHEMA_VERSION, None) => {
                return Err(validation(
                    "M22 promotion contexts require a pinned derivation-capable artifact consumption plan",
                ));
            }
            (M22_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_)) => {
                return Err(validation(
                    "M22 promotion contexts require consumption plan generation 2",
                ));
            }
            (M23_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(plan))
                if plan.schema_version
                    == crate::artifact_consumption::M23_CONSUMPTION_PLAN_SCHEMA_VERSION =>
            {
                plan.validate()?;
                let preparation = plan
                    .preparation
                    .as_deref()
                    .expect("validated M23 plan carries preparation");
                if self.effective_configuration.preprocessing_digest != preparation.plan_digest {
                    return Err(validation(
                        "M23 preprocessing digest must equal the pinned preparation plan digest",
                    ));
                }
            }
            (M23_PROMOTION_CONTEXT_SCHEMA_VERSION, None) => {
                return Err(validation(
                    "M23 promotion contexts require a pinned preparation-and-derivation artifact consumption plan",
                ));
            }
            (M23_PROMOTION_CONTEXT_SCHEMA_VERSION, Some(_)) => {
                return Err(validation(
                    "M23 promotion contexts require consumption plan generation 3",
                ));
            }
            (_, None) => {}
            (_, Some(_)) => {
                return Err(validation(
                    "promotion context versions before M21 cannot carry a consumption plan",
                ));
            }
        }
        self.target.validate()?;
        if self.target.space_type != space_type {
            return Err(validation(
                "promotion target space_type does not match evaluation space",
            ));
        }
        self.candidate.validate()?;
        self.effective_configuration.validate()?;
        if self.effective_configuration.eval_spec_digest != eval_spec_digest {
            return Err(validation(
                "promotion context eval_spec_digest does not match the resolved EvalSpec",
            ));
        }
        self.revisions.corpus.validate("corpus")?;
        self.revisions.graph.validate("graph")?;
        self.revisions.oracle.validate("oracle")?;
        if self.revisions.corpus.kind != "corpus"
            || self.revisions.graph.kind != "graph"
            || self.revisions.oracle.kind != "oracle"
        {
            return Err(validation(
                "revision attestation kinds must be corpus, graph, and oracle in their respective positions",
            ));
        }
        if self.candidate.candidate_digest != self.candidate.artifact.digest {
            return Err(validation(
                "candidate_digest must equal the candidate artifact content digest",
            ));
        }
        self.case_manifest.validate()?;
        if self.case_manifest.expected_distinct != expected_distinct as u64
            || self.case_manifest.forbidden_distinct != forbidden_distinct as u64
            || self.case_manifest.reviewed_case_union_digest != eval_spec_digest
        {
            return Err(validation(
                "case_manifest identity or distinct counts do not match the resolved EvalSpec",
            ));
        }
        self.policy.validate()?;
        if let Some(binding) = &self.governance
            && binding.resolved_policy_digest != canonical_digest(&self.policy)?
        {
            return Err(validation(
                "governance resolved_policy_digest does not match the frozen promotion policy",
            ));
        }
        if !self.case_manifest.exclusions.is_empty() {
            return Err(validation(
                "M18 v1 requires an empty case_manifest exclusions list",
            ));
        }
        if !self
            .policy
            .allowed_candidate_kinds
            .contains(&self.candidate.kind)
        {
            return Err(validation(
                "candidate kind is not allowed by the frozen promotion policy",
            ));
        }
        self.reproducibility.validate()?;
        if self.revisions.graph.candidate_digest.as_deref()
            != Some(self.candidate.candidate_digest.as_str())
            || self.revisions.graph.configuration_digest.as_deref()
                != Some(
                    self.effective_configuration
                        .construction_config_digest
                        .as_str(),
                )
        {
            return Err(validation(
                "graph revision attestation does not bind the candidate and construction configuration",
            ));
        }
        self.oracle_separation.validate_structure()?;
        if self.oracle_separation.promotion_oracle_digest != self.revisions.oracle.manifest_digest {
            return Err(validation(
                "oracle attestation does not bind the frozen oracle revision",
            ));
        }
        Ok(())
    }

    pub fn validate_runtime_backend(&self, backend_name: &str) -> Result<(), CogniGraphError> {
        if matches!(
            self.schema_version,
            M21_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M22_PROMOTION_CONTEXT_SCHEMA_VERSION
                | M23_PROMOTION_CONTEXT_SCHEMA_VERSION
        ) {
            if self.reproducibility.backend != "artifact-snapshot" {
                return Err(validation(
                    "M21-M23 reproducibility backend must be `artifact-snapshot`",
                ));
            }
            return Ok(());
        }
        let expected = if backend_name.starts_with("native") {
            "native"
        } else {
            backend_name
        };
        if self.reproducibility.backend != expected {
            return Err(validation(format!(
                "promotion reproducibility backend `{}` does not match active backend `{expected}`",
                self.reproducibility.backend
            )));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, CogniGraphError> {
        canonical_digest(self)
    }
}
