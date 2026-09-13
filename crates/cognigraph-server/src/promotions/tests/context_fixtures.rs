//! Context fixtures.

use super::*;

pub(super) fn promotion_context(candidate: &str, spec: &EvalSpec) -> PromotionContext {
    let candidate_artifact = artifact(&format!("candidate-{candidate}"));
    let candidate_digest = candidate_artifact.digest.clone();
    let construction_config_digest = digest(&format!("construction-config:{candidate}"));
    let oracle = revision("oracle", "oracle-v1", "oracle-lineage-v1", None, None);
    PromotionContext {
        schema_version: PROMOTION_CONTEXT_SCHEMA_VERSION,
        target: PromotionTarget {
            space_type: SPACE.into(),
            channel: "stable".into(),
        },
        candidate: CandidateIdentity {
            kind: "semantic-neuron-bundle".into(),
            id: candidate.into(),
            revision: "1".into(),
            artifact: candidate_artifact,
            candidate_digest: candidate_digest.clone(),
        },
        effective_configuration: EffectiveConfiguration {
            construction_config_digest: construction_config_digest.clone(),
            evaluation_config_digest: digest("evaluation-config-v1"),
            eval_spec_digest: canonical_digest(spec).expect("EvalSpec digest"),
            preprocessing_digest: digest("preprocessing-v1"),
            scorer_id: M18_SCORER_ID.into(),
            scorer_version: M18_SCORER_VERSION.into(),
            scorer_artifact_digest: digest_bytes(M18_SCORER_ARTIFACT_IDENTITY.as_bytes()),
        },
        revisions: RevisionSet {
            corpus: revision("corpus", "corpus-v1", "corpus-lineage-v1", None, None),
            graph: revision(
                "graph",
                &format!("graph-{candidate}"),
                "graph-lineage-v1",
                Some(candidate_digest),
                Some(construction_config_digest),
            ),
            oracle: oracle.clone(),
        },
        case_manifest: CaseManifest {
            artifact: artifact("case-manifest-v1"),
            digest: digest("artifact:case-manifest-v1"),
            reviewed_case_union_digest: canonical_digest(spec).expect("EvalSpec digest"),
            exclusions: Vec::new(),
            exclusions_digest: canonical_digest(&Vec::<CaseExclusion>::new()).unwrap(),
            expected_distinct: 1,
            forbidden_distinct: 1,
        },
        policy: policy(),
        governance: None,
        artifact_attestations: None,
        consumption_plan: None,
        reproducibility: ReproducibilityContext {
            deterministic: true,
            source_commit: "0123456789abcdef".into(),
            source_tree_digest: digest("source-tree-v1"),
            dirty_tree: false,
            executable_digest: digest("server-executable-v1"),
            toolchain: "rust-test-toolchain".into(),
            target: "test-target".into(),
            backend: "native".into(),
            backend_version: "1".into(),
            command_digest: digest("evaluation-command-v1"),
            resolved_config_digest: digest(&format!("resolved-config:{candidate}")),
            seed: Some(7),
            provider: None,
            model: None,
        },
        oracle_separation: {
            let mut attestation = OracleSeparationAttestation {
                status: OracleAttestationStatus::Verified,
                promotion_oracle_digest: oracle.manifest_digest,
                stage_projections: vec![
                    OracleStageProjection {
                        stage: "candidate_build".into(),
                        read_set_digest: digest("candidate-build-read-set-v1"),
                        oracle_reads: 0,
                    },
                    OracleStageProjection {
                        stage: "candidate_tuning".into(),
                        read_set_digest: digest("candidate-tuning-read-set-v1"),
                        oracle_reads: 0,
                    },
                    OracleStageProjection {
                        stage: "construction".into(),
                        read_set_digest: digest("construction-read-set-v1"),
                        oracle_reads: 0,
                    },
                    OracleStageProjection {
                        stage: "evaluation".into(),
                        read_set_digest: digest("evaluation-read-set-v1"),
                        oracle_reads: 1,
                    },
                ],
                verifier_name: M18_ORACLE_VERIFIER_NAME.into(),
                verifier_version: M18_ORACLE_VERIFIER_VERSION.into(),
                verifier_artifact_digest: digest_bytes(
                    M18_ORACLE_VERIFIER_ARTIFACT_IDENTITY.as_bytes(),
                ),
                verifier_run_id: "oracle-run-1".into(),
                attested_by: M18_ORACLE_VERIFIER_NAME.into(),
                verified_at_ms: 1,
                attestation_uri: "urn:cognigraph:test:oracle-attestation".into(),
                attestation_digest: String::new(),
                overlapping_document_ids: 0,
                overlapping_case_ids: 0,
                overlapping_content_digests: 0,
                overlapping_artifact_digests: 0,
            };
            attestation.attestation_digest =
                record_digest(&attestation, "attestation_digest").unwrap();
            attestation
        },
    }
}
pub(super) fn source(
    candidate: &str,
    job: &str,
    found: u64,
    violations: u64,
) -> PromotionEvaluationSource {
    let spec = eval_spec();
    let context = promotion_context(candidate, &spec);
    let context_digest = context.digest().expect("context digest");
    PromotionEvaluationSource {
        job_id: digest(job).trim_start_matches("sha256:").into(),
        attempt: 1,
        recoveries: 0,
        actor: json!({"type": "system", "username": "test"}),
        input_digest: digest(&format!("input:{candidate}")),
        execution_payload_digest: digest(&format!("payload:{candidate}")),
        eval_spec_digest: canonical_digest(&spec).expect("EvalSpec digest"),
        context_digest,
        result_digest: digest(&format!("result:{found}:{violations}")),
        artifact_consumption: None,
        context,
        recall: CountMetric { found, total: 1 },
        restraint: ViolationMetric {
            violations,
            total: 1,
        },
        missing: if found == 0 {
            vec!["Meridian --SUPPLIES--> Compound X".into()]
        } else {
            Vec::new()
        },
        violations: if violations == 0 {
            Vec::new()
        } else {
            vec!["Meridian --OWNS--> Compound X".into()]
        },
        finished_at_ms: 1,
    }
}
pub(super) fn admin() -> PromotionActor {
    PromotionActor {
        user_key: "admin-1".into(),
        username: "admin".into(),
        role: "admin".into(),
    }
}
pub(super) struct TestGovernancePrincipal {
    pub(super) signing_key: SigningKeyMaterial,
    pub(super) actor: GovernanceActor,
    pub(super) record: GovernanceKeyRecord,
}
