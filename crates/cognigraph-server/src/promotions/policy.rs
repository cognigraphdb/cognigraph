//! Policy.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseExclusion {
    pub case_id: String,
    pub reason_code: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseManifest {
    pub artifact: ArtifactIdentity,
    pub digest: String,
    pub reviewed_case_union_digest: String,
    pub exclusions: Vec<CaseExclusion>,
    pub exclusions_digest: String,
    pub expected_distinct: u64,
    pub forbidden_distinct: u64,
}
impl CaseManifest {
    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        self.artifact.validate("case_manifest.artifact")?;
        validate_digest("case_manifest.digest", &self.digest)?;
        if self.digest != self.artifact.digest {
            return Err(validation(
                "case_manifest.digest does not match its artifact digest",
            ));
        }
        validate_digest(
            "case_manifest.reviewed_case_union_digest",
            &self.reviewed_case_union_digest,
        )?;
        validate_digest("case_manifest.exclusions_digest", &self.exclusions_digest)?;
        let mut previous_case: Option<&str> = None;
        for exclusion in &self.exclusions {
            validate_text(
                "case_manifest.exclusions.case_id",
                &exclusion.case_id,
                MAX_IDENTIFIER_BYTES,
            )?;
            validate_text(
                "case_manifest.exclusions.reason_code",
                &exclusion.reason_code,
                MAX_IDENTIFIER_BYTES,
            )?;
            if previous_case.is_some_and(|previous| previous >= exclusion.case_id.as_str()) {
                return Err(validation(
                    "case_manifest exclusions must be sorted by unique case_id",
                ));
            }
            previous_case = Some(&exclusion.case_id);
        }
        if canonical_digest(&self.exclusions)? != self.exclusions_digest {
            return Err(validation("case_manifest exclusions digest mismatch"));
        }
        if self.expected_distinct == 0 || self.forbidden_distinct == 0 {
            return Err(validation(
                "case_manifest requires positive expected_distinct and forbidden_distinct",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecallPolicy {
    pub min_expected_distinct: u64,
    pub min_ratio_numerator: u64,
    pub min_ratio_denominator: u64,
    pub max_missing: u64,
    pub max_additional_missing_vs_baseline: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestraintPolicy {
    pub min_forbidden_distinct: u64,
    pub min_ratio_numerator: u64,
    pub min_ratio_denominator: u64,
    pub max_violations: u64,
    pub max_additional_violations_vs_baseline: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExclusionPolicy {
    pub allowed_reason_codes: Vec<String>,
    pub max_count: u64,
    pub require_same_manifest_as_baseline: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPromotionPolicy {
    pub schema_version: u32,
    pub policy_id: String,
    pub policy_revision: String,
    pub source_digest: String,
    pub allowed_candidate_kinds: Vec<String>,
    pub allowed_candidate_differences: Vec<CandidateDifference>,
    pub evaluator_id: String,
    pub metric_semantics_version: String,
    pub recall: RecallPolicy,
    pub restraint: RestraintPolicy,
    pub exclusions: ExclusionPolicy,
    pub required_runs: u8,
    pub require_exact_replay: bool,
    pub oracle: OraclePolicy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDifference {
    CandidateIdentity,
    ConstructionConfiguration,
    GraphRevision,
    ResolvedConfiguration,
}
impl ResolvedPromotionPolicy {
    pub(crate) fn validate(&self) -> Result<(), CogniGraphError> {
        if self.schema_version != PROMOTION_POLICY_SCHEMA_VERSION {
            return Err(validation(format!(
                "unsupported promotion policy schema version {}",
                self.schema_version
            )));
        }
        validate_text("policy.policy_id", &self.policy_id, MAX_IDENTIFIER_BYTES)?;
        validate_text(
            "policy.policy_revision",
            &self.policy_revision,
            MAX_IDENTIFIER_BYTES,
        )?;
        validate_digest("policy.source_digest", &self.source_digest)?;
        validate_text(
            "policy.evaluator_id",
            &self.evaluator_id,
            MAX_IDENTIFIER_BYTES,
        )?;
        if self.evaluator_id != M18_EVALUATOR_ID {
            return Err(validation(
                "M18 v1 only supports evaluator_id `construct.evaluate`",
            ));
        }
        validate_text(
            "policy.metric_semantics_version",
            &self.metric_semantics_version,
            MAX_IDENTIFIER_BYTES,
        )?;
        if self.metric_semantics_version != M18_METRIC_SEMANTICS_VERSION {
            return Err(validation(
                "M18 v1 only supports the server's distinct-fact metric semantics",
            ));
        }
        if self.allowed_candidate_kinds.is_empty() {
            return Err(validation("policy.allowed_candidate_kinds is empty"));
        }
        validate_sorted_unique(
            "policy.allowed_candidate_kinds",
            &self.allowed_candidate_kinds,
        )?;
        if self.allowed_candidate_differences.is_empty() {
            return Err(validation("policy.allowed_candidate_differences is empty"));
        }
        if self
            .allowed_candidate_differences
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(validation(
                "policy.allowed_candidate_differences must be sorted and unique",
            ));
        }
        validate_ratio(
            "policy.recall",
            self.recall.min_ratio_numerator,
            self.recall.min_ratio_denominator,
        )?;
        validate_ratio(
            "policy.restraint",
            self.restraint.min_ratio_numerator,
            self.restraint.min_ratio_denominator,
        )?;
        if self.recall.min_expected_distinct == 0 || self.restraint.min_forbidden_distinct == 0 {
            return Err(validation(
                "policy denominator minima must both be positive",
            ));
        }
        if !self.exclusions.allowed_reason_codes.is_empty() || self.exclusions.max_count != 0 {
            return Err(validation(
                "M18 v1 does not permit exclusions until excluded-case scoring is implemented",
            ));
        }
        if !self.exclusions.require_same_manifest_as_baseline {
            return Err(validation(
                "M18 v1 requires the same case manifest and exclusions as the baseline",
            ));
        }
        if self.required_runs != 2 || !self.require_exact_replay {
            return Err(validation(
                "M18 v1 requires exactly two runs and exact replay",
            ));
        }
        self.oracle.validate()?;
        Ok(())
    }
}
