//! Evidence contracts.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegisterEvidenceRequest {
    pub candidate_original_job_id: String,
    pub candidate_replay_job_id: String,
    pub baseline_original_job_id: String,
    pub baseline_replay_job_id: String,
    pub expected_head_decision_id: Option<String>,
}
impl<'de> Deserialize<'de> for RegisterEvidenceRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            candidate_original_job_id: String,
            candidate_replay_job_id: String,
            baseline_original_job_id: String,
            baseline_replay_job_id: String,
            expected_head_decision_id: Option<String>,
        }
        let value = Value::deserialize(deserializer)?;
        if !value
            .as_object()
            .is_some_and(|object| object.contains_key("expected_head_decision_id"))
        {
            return Err(serde::de::Error::missing_field("expected_head_decision_id"));
        }
        let wire: Wire = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            candidate_original_job_id: wire.candidate_original_job_id,
            candidate_replay_job_id: wire.candidate_replay_job_id,
            baseline_original_job_id: wire.baseline_original_job_id,
            baseline_replay_job_id: wire.baseline_replay_job_id,
            expected_head_decision_id: wire.expected_head_decision_id,
        })
    }
}
impl RegisterEvidenceRequest {
    pub fn validate(&self) -> Result<(), CogniGraphError> {
        let ids = [
            &self.candidate_original_job_id,
            &self.candidate_replay_job_id,
            &self.baseline_original_job_id,
            &self.baseline_replay_job_id,
        ];
        for id in ids {
            validate_record_id("evaluation job id", id)?;
        }
        if ids.into_iter().collect::<HashSet<_>>().len() != 4 {
            return Err(validation(
                "evidence registration requires four distinct job ids",
            ));
        }
        if let Some(id) = &self.expected_head_decision_id {
            validate_record_id("expected_head_decision_id", id)?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRunRole {
    CandidateOriginal,
    CandidateReplay,
    BaselineOriginal,
    BaselineReplay,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRun {
    pub role: EvidenceRunRole,
    pub source: PromotionEvaluationSource,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    pub passed: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionGateAssessment {
    pub pair_reproducibility: GateResult,
    pub baseline_comparability: GateResult,
    pub denominators: GateResult,
    pub recall: GateResult,
    pub restraint: GateResult,
    pub oracle_separation: GateResult,
    pub overall_passed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceArtifactAuthority {
    pub candidate: crate::artifact_attestations::ArtifactAttestationSet,
    pub baseline: crate::artifact_attestations::ArtifactAttestationSet,
    pub authority_digest: String,
}
impl EvidenceArtifactAuthority {
    pub(super) fn validate(&self) -> Result<(), CogniGraphError> {
        self.candidate.validate()?;
        self.baseline.validate()?;
        validate_digest("artifact authority digest", &self.authority_digest)?;
        if record_digest(self, "authority_digest")? != self.authority_digest {
            return Err(validation("artifact authority digest mismatch"));
        }
        Ok(())
    }
}
pub(super) fn artifact_authority_conflicts_with_policy(
    authority: &EvidenceArtifactAuthority,
    governance: &PolicyGovernanceBinding,
) -> bool {
    let candidate_principals = authority.candidate.attestor_principals();
    let baseline_principals = authority.baseline.attestor_principals();
    [
        governance.author_principal_id.as_str(),
        governance.approver_principal_id.as_str(),
    ]
    .into_iter()
    .any(|principal| {
        candidate_principals.contains(principal) || baseline_principals.contains(principal)
    })
}
impl PromotionGateAssessment {
    pub(super) fn finalize(mut self) -> Self {
        self.overall_passed = self.pair_reproducibility.passed
            && self.baseline_comparability.passed
            && self.denominators.passed
            && self.recall.passed
            && self.restraint.passed
            && self.oracle_separation.passed;
        self
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionEvidence {
    #[serde(rename = "_key")]
    pub key: String,
    pub schema_version: u32,
    pub digest_algorithm: String,
    pub id: String,
    pub tenant: String,
    pub tenant_incarnation: String,
    pub target: PromotionTarget,
    pub created_at_ms: u64,
    pub created_by: PromotionActor,
    pub idempotency_key_hash: String,
    pub request_digest: String,
    pub evidence_digest: String,
    pub expected_head_decision_id: Option<String>,
    pub rollback_target_evidence_id: Option<String>,
    pub candidate_digest: String,
    pub baseline_candidate_digest: String,
    pub policy: ResolvedPromotionPolicy,
    pub policy_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance: Option<PolicyGovernanceBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_attestations: Option<EvidenceArtifactAuthority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_consumption:
        Option<Box<crate::artifact_consumption::EvidenceConsumptionAuthority>>,
    pub runs: Vec<EvidenceRun>,
    pub gates: PromotionGateAssessment,
}
impl PromotionEvidence {
    pub fn public_value(&self) -> Result<Value, CogniGraphError> {
        public_record_value(self)
    }
}
