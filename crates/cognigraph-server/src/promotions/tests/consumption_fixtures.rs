//! Consumption fixtures.

use super::*;

pub(super) async fn stored_consumed_context(
    state: &AppState,
    cas_root: &Path,
    attestor: &TestGovernancePrincipal,
    candidate: &str,
    binding: &PolicyGovernanceBinding,
    shared: Option<&ArtifactAttestationSet>,
) -> PromotionContext {
    let mut context = governed_context(candidate, binding);
    context.schema_version = M21_PROMOTION_CONTEXT_SCHEMA_VERSION;
    context.reproducibility.backend = "artifact-snapshot".into();
    context.reproducibility.backend_version = "1".into();
    context.reproducibility.executable_digest = current_executable_digest().await.unwrap();
    context.consumption_plan = Some(Box::new(ArtifactConsumptionPlan::supported_v1()));

    let (corpus_attestation_id, corpus_manifest_digest) = if let Some(shared) = shared {
        (
            shared.corpus.attestation_id.clone(),
            shared.corpus.manifest_digest.clone(),
        )
    } else {
        let corpus_manifest = staged_manifest(
            cas_root,
            ArtifactKind::Corpus,
            CORPUS_ARTIFACT_FORMAT,
            "corpus.txt",
            "text/plain",
            false,
            b"immutable promotion corpus\n",
        );
        let corpus = store_signed_manifest_attestation(
            state,
            attestor,
            "corpus",
            corpus_manifest,
            ArtifactSubject::Corpus {
                revision: context.revisions.corpus.clone(),
                preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
            },
        )
        .await;
        (corpus.attestation_id, corpus.manifest_digest)
    };

    let graph_artifact = EvaluationGraphArtifact {
        schema_version: 1,
        space_type: context.target.space_type.clone(),
        corpus_manifest_digest: corpus_manifest_digest.clone(),
        candidate_digest: context.candidate.candidate_digest.clone(),
        construction_config_digest: context
            .effective_configuration
            .construction_config_digest
            .clone(),
        facts: vec![VerifiedGraphFact {
            source: "Meridian".into(),
            relation: "SUPPLIES".into(),
            target: "Compound X".into(),
            evidence_chunk_id: format!("m21-{candidate}-chunk"),
        }],
    };
    let graph_manifest = staged_manifest(
        cas_root,
        ArtifactKind::Graph,
        GRAPH_ARTIFACT_FORMAT,
        GRAPH_ENTRYPOINT,
        "application/json",
        false,
        &serde_json::to_vec(&graph_artifact).unwrap(),
    );
    let graph = store_signed_manifest_attestation(
        state,
        attestor,
        &format!("graph-{candidate}"),
        graph_manifest,
        ArtifactSubject::Graph {
            revision: context.revisions.graph.clone(),
            candidate_digest: context.candidate.candidate_digest.clone(),
            construction_config_digest: context
                .effective_configuration
                .construction_config_digest
                .clone(),
            corpus_manifest_digest: corpus_manifest_digest.clone(),
            preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
        },
    )
    .await;

    let (oracle_attestation_id, scorer_attestation_id, verifier_attestation_id) =
        if let Some(shared) = shared {
            (
                shared.oracle.attestation_id.clone(),
                shared.scorer.attestation_id.clone(),
                shared.verifier.attestation_id.clone(),
            )
        } else {
            let oracle_artifact = PromotionOracleArtifact {
                schema_version: 1,
                case_manifest_digest: context.case_manifest.digest.clone(),
                corpus_manifest_digest: corpus_manifest_digest.clone(),
                eval_spec: eval_spec(),
            };
            let oracle_manifest = staged_manifest(
                cas_root,
                ArtifactKind::Oracle,
                ORACLE_ARTIFACT_FORMAT,
                ORACLE_ENTRYPOINT,
                "application/json",
                false,
                &serde_json::to_vec(&oracle_artifact).unwrap(),
            );
            let oracle = store_signed_manifest_attestation(
                state,
                attestor,
                "oracle",
                oracle_manifest,
                ArtifactSubject::Oracle {
                    revision: context.revisions.oracle.clone(),
                    eval_spec_digest: context.effective_configuration.eval_spec_digest.clone(),
                    case_manifest_digest: context.case_manifest.digest.clone(),
                    corpus_manifest_digest: corpus_manifest_digest.clone(),
                },
            )
            .await;
            let scorer = store_signed_manifest_attestation(
                state,
                attestor,
                "scorer",
                staged_executable_manifest(
                    cas_root,
                    ArtifactKind::Scorer,
                    &context.reproducibility.executable_digest,
                ),
                ArtifactSubject::Scorer {
                    scorer_id: context.effective_configuration.scorer_id.clone(),
                    scorer_version: context.effective_configuration.scorer_version.clone(),
                    semantics_identity_digest: context
                        .effective_configuration
                        .scorer_artifact_digest
                        .clone(),
                    metric_semantics_version: context.policy.metric_semantics_version.clone(),
                    executable_digest: context.reproducibility.executable_digest.clone(),
                },
            )
            .await;
            let verifier = store_signed_manifest_attestation(
                state,
                attestor,
                "verifier",
                staged_executable_manifest(
                    cas_root,
                    ArtifactKind::Verifier,
                    &context.reproducibility.executable_digest,
                ),
                ArtifactSubject::Verifier {
                    verifier_name: context.policy.oracle.verifier_name.clone(),
                    verifier_version: context.policy.oracle.verifier_version.clone(),
                    semantics_identity_digest: context
                        .policy
                        .oracle
                        .verifier_artifact_digest
                        .clone(),
                },
            )
            .await;
            (
                oracle.attestation_id,
                scorer.attestation_id,
                verifier.attestation_id,
            )
        };

    let set = state
        .promotions
        .resolve_artifact_bindings(
            TENANT,
            INCARNATION,
            &ResolveArtifactBindingsRequest {
                corpus_attestation_id,
                graph_attestation_id: graph.attestation_id,
                oracle_attestation_id,
                scorer_attestation_id,
                verifier_attestation_id,
            },
        )
        .await
        .unwrap();
    context.artifact_attestations = Some(set);
    context
        .validate(
            SPACE,
            &canonical_digest(&eval_spec()).unwrap(),
            context.case_manifest.expected_distinct.try_into().unwrap(),
            context.case_manifest.forbidden_distinct.try_into().unwrap(),
        )
        .unwrap();
    context
}
pub(super) async fn stored_derived_context(
    state: &AppState,
    cas_root: &Path,
    attestor: &TestGovernancePrincipal,
    candidate: &str,
    binding: &PolicyGovernanceBinding,
    shared: Option<&ArtifactAttestationSet>,
    forge_evidence_chunk: bool,
) -> PromotionContext {
    stored_reproducible_context(
        state,
        cas_root,
        attestor,
        candidate,
        binding,
        shared,
        forge_evidence_chunk,
        false,
        false,
    )
    .await
}
pub(super) async fn stored_prepared_context(
    state: &AppState,
    cas_root: &Path,
    attestor: &TestGovernancePrincipal,
    candidate: &str,
    binding: &PolicyGovernanceBinding,
    shared: Option<&ArtifactAttestationSet>,
    forge_prepared_output: bool,
) -> PromotionContext {
    stored_reproducible_context(
        state,
        cas_root,
        attestor,
        candidate,
        binding,
        shared,
        false,
        true,
        forge_prepared_output,
    )
    .await
}
