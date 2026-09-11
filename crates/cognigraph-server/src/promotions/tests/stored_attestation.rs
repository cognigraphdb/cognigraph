//! Stored attestation.

use super::*;

pub(super) async fn stored_attested_context(
    state: &AppState,
    attestor: &TestGovernancePrincipal,
    candidate: &str,
    binding: &PolicyGovernanceBinding,
    shared: Option<&ArtifactAttestationSet>,
) -> PromotionContext {
    let mut context = governed_context(candidate, binding);
    context.schema_version = M20_PROMOTION_CONTEXT_SCHEMA_VERSION;

    let (corpus_attestation_id, corpus_manifest_digest) = if let Some(shared) = shared {
        (
            shared.corpus.attestation_id.clone(),
            shared.corpus.manifest_digest.clone(),
        )
    } else {
        let corpus = store_signed_artifact_attestation(
            state,
            attestor,
            ArtifactKind::Corpus,
            "corpus",
            ArtifactSubject::Corpus {
                revision: context.revisions.corpus.clone(),
                preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
            },
        )
        .await;
        (corpus.attestation_id, corpus.manifest_digest)
    };

    let graph = store_signed_artifact_attestation(
        state,
        attestor,
        ArtifactKind::Graph,
        &format!("graph-{candidate}"),
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
            let oracle = store_signed_artifact_attestation(
                state,
                attestor,
                ArtifactKind::Oracle,
                "oracle",
                ArtifactSubject::Oracle {
                    revision: context.revisions.oracle.clone(),
                    eval_spec_digest: context.effective_configuration.eval_spec_digest.clone(),
                    case_manifest_digest: context.case_manifest.digest.clone(),
                    corpus_manifest_digest: corpus_manifest_digest.clone(),
                },
            )
            .await;
            let scorer = store_signed_artifact_attestation(
                state,
                attestor,
                ArtifactKind::Scorer,
                "scorer",
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
            let verifier = store_signed_artifact_attestation(
                state,
                attestor,
                ArtifactKind::Verifier,
                "verifier",
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
