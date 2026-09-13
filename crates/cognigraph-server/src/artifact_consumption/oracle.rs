//! Oracle.

use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct PromotionOracleArtifact {
    pub schema_version: u32,
    pub case_manifest_digest: String,
    pub corpus_manifest_digest: String,
    pub eval_spec: EvalSpec,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PromotionOracleArtifactWire {
    pub(super) schema_version: u32,
    pub(super) case_manifest_digest: String,
    pub(super) corpus_manifest_digest: String,
    pub(super) eval_spec: PromotionOracleEvalSpec,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PromotionOracleEvalSpec {
    pub(super) space_id: String,
    #[serde(default)]
    pub(super) questions: Vec<PromotionOracleEvalQuestion>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PromotionOracleEvalQuestion {
    pub(super) id: String,
    #[serde(default)]
    pub(super) question: String,
    #[serde(default)]
    pub(super) expected_facts: Vec<String>,
    #[serde(default)]
    pub(super) forbidden_facts: Vec<String>,
}
impl From<PromotionOracleEvalSpec> for EvalSpec {
    fn from(value: PromotionOracleEvalSpec) -> Self {
        Self {
            space_id: value.space_id,
            questions: value
                .questions
                .into_iter()
                .map(|question| cognigraph_construct::EvalQuestion {
                    id: question.id,
                    question: question.question,
                    expected_facts: question.expected_facts,
                    forbidden_facts: question.forbidden_facts,
                })
                .collect(),
        }
    }
}
pub(crate) fn parse_strict_eval_spec(
    value: serde_json::Value,
) -> Result<EvalSpec, serde_json::Error> {
    serde_json::from_value::<PromotionOracleEvalSpec>(value).map(Into::into)
}
impl<'de> Deserialize<'de> for PromotionOracleArtifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = PromotionOracleArtifactWire::deserialize(deserializer)?;
        Ok(Self {
            schema_version: value.schema_version,
            case_manifest_digest: value.case_manifest_digest,
            corpus_manifest_digest: value.corpus_manifest_digest,
            eval_spec: value.eval_spec.into(),
        })
    }
}
impl PromotionOracleArtifact {
    pub(crate) fn validate(
        &self,
        context: &PromotionContext,
        corpus_manifest_digest: &str,
        resolved_eval_spec: &EvalSpec,
    ) -> Result<(), CogniGraphError> {
        if self.schema_version != 1
            || self.case_manifest_digest != context.case_manifest.digest
            || self.corpus_manifest_digest != corpus_manifest_digest
            || self.eval_spec.space_id != context.target.space_type
            // Canonical digests normalize strings. Evaluation matching does
            // not, so require exact resolved values as well as the
            // canonical context digest to prevent Unicode substitution.
            || serde_json::to_value(&self.eval_spec)? != serde_json::to_value(resolved_eval_spec)?
            || canonical_digest(&self.eval_spec)?
                != context.effective_configuration.eval_spec_digest
        {
            return Err(validation(
                "promotion oracle artifact does not match its M21 context",
            ));
        }
        validate_digest("oracle.case_manifest_digest", &self.case_manifest_digest)?;
        validate_digest(
            "oracle.corpus_manifest_digest",
            &self.corpus_manifest_digest,
        )?;
        validate_eval_spec_text(&self.eval_spec)
    }
}
