//! Evaluation fixtures.

use super::*;

pub(super) fn digest(label: &str) -> String {
    digest_bytes(label.as_bytes())
}
pub(super) fn artifact(label: &str) -> ArtifactIdentity {
    ArtifactIdentity {
        uri: format!("urn:cognigraph:test:{label}"),
        media_type: "application/json".into(),
        bytes: 1,
        digest: digest(&format!("artifact:{label}")),
    }
}
pub(super) fn revision(
    kind: &str,
    label: &str,
    source_lineage: &str,
    candidate_digest: Option<String>,
    configuration_digest: Option<String>,
) -> RevisionAttestation {
    RevisionAttestation {
        kind: kind.into(),
        revision_id: format!("{label}-revision"),
        manifest_uri: format!("urn:cognigraph:test:{label}:manifest"),
        manifest_digest: digest(&format!("manifest:{label}")),
        source_lineage: source_lineage.into(),
        immutability_method: "content-addressed".into(),
        issuer: "test-suite".into(),
        issued_at_ms: 1,
        verification_uri: format!("urn:cognigraph:test:{label}:verification"),
        verification_digest: digest(&format!("verification:{label}")),
        candidate_digest,
        configuration_digest,
        immutable: true,
    }
}
pub(super) fn eval_spec() -> EvalSpec {
    EvalSpec {
        space_id: SPACE.into(),
        questions: vec![EvalQuestion {
            id: "q1".into(),
            question: "What does Meridian supply?".into(),
            expected_facts: vec!["Meridian --SUPPLIES--> Compound X".into()],
            forbidden_facts: vec!["Meridian --OWNS--> Compound X".into()],
        }],
    }
}
pub(super) fn policy() -> ResolvedPromotionPolicy {
    ResolvedPromotionPolicy {
        schema_version: PROMOTION_POLICY_SCHEMA_VERSION,
        policy_id: "strict-v1".into(),
        policy_revision: "1".into(),
        source_digest: digest("policy-source"),
        allowed_candidate_kinds: vec!["semantic-neuron-bundle".into()],
        allowed_candidate_differences: vec![
            CandidateDifference::CandidateIdentity,
            CandidateDifference::ConstructionConfiguration,
            CandidateDifference::GraphRevision,
            CandidateDifference::ResolvedConfiguration,
        ],
        evaluator_id: M18_EVALUATOR_ID.into(),
        metric_semantics_version: M18_METRIC_SEMANTICS_VERSION.into(),
        recall: RecallPolicy {
            min_expected_distinct: 1,
            min_ratio_numerator: 1,
            min_ratio_denominator: 1,
            max_missing: 0,
            max_additional_missing_vs_baseline: 0,
        },
        restraint: RestraintPolicy {
            min_forbidden_distinct: 1,
            min_ratio_numerator: 1,
            min_ratio_denominator: 1,
            max_violations: 0,
            max_additional_violations_vs_baseline: 0,
        },
        exclusions: ExclusionPolicy {
            allowed_reason_codes: Vec::new(),
            max_count: 0,
            require_same_manifest_as_baseline: true,
        },
        required_runs: 2,
        require_exact_replay: true,
        oracle: OraclePolicy {
            required_status: OracleAttestationStatus::Verified,
            verifier_name: M18_ORACLE_VERIFIER_NAME.into(),
            verifier_version: M18_ORACLE_VERIFIER_VERSION.into(),
            verifier_artifact_digest: digest_bytes(
                M18_ORACLE_VERIFIER_ARTIFACT_IDENTITY.as_bytes(),
            ),
            required_stages: vec![
                OracleStageRule {
                    stage: "candidate_build".into(),
                    allow_oracle_reads: false,
                },
                OracleStageRule {
                    stage: "candidate_tuning".into(),
                    allow_oracle_reads: false,
                },
                OracleStageRule {
                    stage: "construction".into(),
                    allow_oracle_reads: false,
                },
                OracleStageRule {
                    stage: "evaluation".into(),
                    allow_oracle_reads: true,
                },
            ],
        },
    }
}
