//! Derivation variant.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn stored_reproducible_context_variant(
    state: &AppState,
    cas_root: &Path,
    attestor: &TestGovernancePrincipal,
    candidate: &str,
    binding: &PolicyGovernanceBinding,
    shared: Option<&ArtifactAttestationSet>,
    forge_evidence_chunk: bool,
    prepare_from_raw: bool,
    forge_prepared_output: bool,
    variant: ReproducibleContextVariant,
) -> PromotionContext {
    let artifact_scope = candidate;
    let mut context = governed_context(candidate, binding);
    context.schema_version = if prepare_from_raw {
        M23_PROMOTION_CONTEXT_SCHEMA_VERSION
    } else {
        M22_PROMOTION_CONTEXT_SCHEMA_VERSION
    };
    context.reproducibility.backend = "artifact-snapshot".into();
    context.reproducibility.backend_version = "1".into();
    context.reproducibility.executable_digest = current_executable_digest().await.unwrap();
    context.consumption_plan = Some(Box::new(if prepare_from_raw {
        ArtifactConsumptionPlan::supported_v3()
    } else {
        ArtifactConsumptionPlan::supported_v2()
    }));
    if prepare_from_raw {
        context.effective_configuration.preprocessing_digest = context
            .consumption_plan
            .as_ref()
            .unwrap()
            .preparation
            .as_ref()
            .unwrap()
            .plan_digest
            .clone();
    }

    let construction_space = ConstructionSpaceType {
        id: SPACE.into(),
        name: "Pharma".into(),
        version: 1,
        description: "M22 deterministic test construction".into(),
        entities: vec![
            ConstructionEntity {
                name: "Compound X".into(),
                entity_type: "compound".into(),
                aliases: Vec::new(),
            },
            ConstructionEntity {
                name: "Meridian".into(),
                entity_type: "organization".into(),
                aliases: Vec::new(),
            },
        ],
        relation_rules: variant.relation_rules(),
    };
    let candidate_artifact = ConstructionCandidateArtifact {
        schema_version: 1,
        kind: context.candidate.kind.clone(),
        id: context.candidate.id.clone(),
        revision: context.candidate.revision.clone(),
        base_space_type: construction_space.clone(),
        accepted_neurons: Vec::new(),
    };
    let candidate_bytes = cognigraph_governance::canonical_json_bytes(&candidate_artifact).unwrap();
    let candidate_digest = digest_bytes(&candidate_bytes);
    context.candidate.artifact = ArtifactIdentity {
        uri: format!("urn:cognigraph:test:m22:candidate:{candidate}"),
        media_type: "application/json".into(),
        bytes: candidate_bytes.len() as u64,
        digest: candidate_digest.clone(),
    };
    context.candidate.candidate_digest = candidate_digest.clone();

    let effective_space = cognigraph_construct::SpaceType {
        id: construction_space.id.clone(),
        name: construction_space.name.clone(),
        version: construction_space.version,
        description: construction_space.description.clone(),
        entities: construction_space
            .entities
            .iter()
            .map(|entity| cognigraph_construct::EntityDef {
                name: entity.name.clone(),
                entity_type: entity.entity_type.clone(),
                aliases: entity.aliases.clone(),
            })
            .collect(),
        relation_rules: construction_space
            .relation_rules
            .iter()
            .map(|rule| cognigraph_construct::RelationRule {
                source: rule.source.clone(),
                relation: rule.relation.clone(),
                target: rule.target.clone(),
                when_any: rule.when_any.clone(),
                require_in_sentence: rule.require_in_sentence.clone(),
                trigger_provenance: BTreeMap::new(),
            })
            .collect(),
    };
    let resolved = ResolvedConstructionConfig {
        schema_version: 1,
        space_type: &effective_space,
        vetoes: Vec::new(),
    };
    let construction_config_digest = canonical_digest(&resolved).unwrap();
    context.effective_configuration.construction_config_digest = construction_config_digest.clone();
    context.reproducibility.resolved_config_digest = construction_config_digest.clone();
    context.revisions.graph.candidate_digest = Some(candidate_digest.clone());
    context.revisions.graph.configuration_digest = Some(construction_config_digest.clone());

    let (corpus_attestation_id, corpus_manifest_digest, corpus_semantic_digest, corpus_chunk_id) =
        if let Some(shared) = shared.filter(|_| !forge_prepared_output) {
            let record = state
                .promotions
                .get_artifact_attestation(TENANT, INCARNATION, &shared.corpus.attestation_id)
                .await
                .unwrap();
            assert_eq!(
                record.manifest.artifact_format,
                if prepare_from_raw {
                    M23_CORPUS_ARTIFACT_FORMAT
                } else {
                    M22_CORPUS_ARTIFACT_FORMAT
                }
            );
            let entry = &record.manifest.entries[0];
            let bytes = fs::read(staged_blob_path(cas_root, &entry.blob_digest)).unwrap();
            let artifact: PreparedChunkCorpusArtifact = serde_json::from_slice(&bytes).unwrap();
            (
                shared.corpus.attestation_id.clone(),
                shared.corpus.manifest_digest.clone(),
                canonical_digest(&artifact).unwrap(),
                artifact.chunks.first().unwrap().id.clone(),
            )
        } else {
            let (mut corpus_artifact, documents_bytes) = if prepare_from_raw {
                let raw_bytes = b"\xef\xbb\xbfMeridian\r\nsupplies\tCompound X.".to_vec();
                let preparation = context
                    .consumption_plan
                    .as_ref()
                    .unwrap()
                    .preparation
                    .as_ref()
                    .unwrap();
                let raw_artifact = RawDocumentSetArtifact {
                    schema_version: 1,
                    space_type: SPACE.into(),
                    corpus_revision_id: context.revisions.corpus.revision_id.clone(),
                    preparation_plan_digest: preparation.plan_digest.clone(),
                    documents: vec![RawDocumentArtifact {
                        id: "meridian-supply".into(),
                        title: "Meridian supply".into(),
                        media_type: "text/plain; charset=utf-8".into(),
                        byte_length: raw_bytes.len() as u64,
                        blob_digest: digest_bytes(&raw_bytes),
                        content_base64url: URL_SAFE_NO_PAD.encode(&raw_bytes),
                    }],
                };
                let prepared = cognigraph_construct::prepare_documents(
                    &[cognigraph_construct::RawDocumentBytes {
                        id: "meridian-supply".into(),
                        title: "Meridian supply".into(),
                        bytes: raw_bytes,
                    }],
                    cognigraph_construct::PreparationOptions {
                        max_document_count: preparation.max_documents as usize,
                        max_document_bytes: preparation.max_raw_document_bytes as usize,
                        max_total_document_bytes: preparation.max_total_raw_document_bytes as usize,
                        max_normalized_document_bytes: preparation.max_normalized_document_bytes
                            as usize,
                        max_total_normalized_bytes: preparation.max_total_normalized_bytes as usize,
                        max_chunk_bytes: preparation.max_chunk_bytes as usize,
                        max_chunk_count: preparation.max_chunks as usize,
                        max_total_prepared_bytes: preparation.max_total_prepared_text_bytes
                            as usize,
                        yield_every_documents: preparation.yield_every_documents as usize,
                    },
                )
                .await
                .unwrap();
                (
                    PreparedChunkCorpusArtifact {
                        schema_version: 1,
                        space_type: SPACE.into(),
                        corpus_revision_id: context.revisions.corpus.revision_id.clone(),
                        preprocessing_digest: preparation.plan_digest.clone(),
                        chunks: prepared
                            .into_iter()
                            .map(|chunk| PreparedChunkArtifact {
                                id: chunk.id,
                                title: chunk.title,
                                text: chunk.text,
                            })
                            .collect(),
                    },
                    Some(cognigraph_governance::canonical_json_bytes(&raw_artifact).unwrap()),
                )
            } else {
                (
                    PreparedChunkCorpusArtifact {
                        schema_version: 1,
                        space_type: SPACE.into(),
                        corpus_revision_id: context.revisions.corpus.revision_id.clone(),
                        preprocessing_digest: context
                            .effective_configuration
                            .preprocessing_digest
                            .clone(),
                        chunks: vec![PreparedChunkArtifact {
                            id: "m22-corpus-chunk".into(),
                            title: "Meridian supply".into(),
                            text: variant.corpus_text().into(),
                        }],
                    },
                    None,
                )
            };
            if forge_prepared_output {
                corpus_artifact.chunks[0].text.push_str(" Altered.");
            }
            let corpus_chunk_id = corpus_artifact.chunks[0].id.clone();
            let corpus_semantic_digest = canonical_digest(&corpus_artifact).unwrap();
            let corpus_bytes =
                cognigraph_governance::canonical_json_bytes(&corpus_artifact).unwrap();
            let corpus_manifest = if let Some(documents_bytes) = &documents_bytes {
                staged_m23_corpus_manifest(cas_root, &corpus_bytes, documents_bytes)
            } else {
                staged_manifest(
                    cas_root,
                    ArtifactKind::Corpus,
                    M22_CORPUS_ARTIFACT_FORMAT,
                    CORPUS_ENTRYPOINT,
                    "application/json",
                    false,
                    &corpus_bytes,
                )
            };
            let corpus = store_signed_manifest_attestation(
                state,
                attestor,
                &format!(
                    "{}-{artifact_scope}-corpus-{}",
                    if prepare_from_raw { "m23" } else { "m22" },
                    if forge_prepared_output {
                        "forged"
                    } else {
                        "valid"
                    }
                ),
                corpus_manifest,
                ArtifactSubject::Corpus {
                    revision: context.revisions.corpus.clone(),
                    preprocessing_digest: context
                        .effective_configuration
                        .preprocessing_digest
                        .clone(),
                },
            )
            .await;
            (
                corpus.attestation_id,
                corpus.manifest_digest,
                corpus_semantic_digest,
                corpus_chunk_id,
            )
        };

    let facts = variant.facts(if forge_evidence_chunk {
        "forged-but-score-equivalent-chunk"
    } else {
        &corpus_chunk_id
    });
    let graph_artifact = ReproducibleEvaluationGraphArtifact {
        schema_version: 1,
        space_type: SPACE.into(),
        graph_revision_id: context.revisions.graph.revision_id.clone(),
        corpus_manifest_digest: corpus_manifest_digest.clone(),
        corpus_semantic_digest,
        candidate_digest: candidate_digest.clone(),
        construction_config_digest: construction_config_digest.clone(),
        derivation_plan_digest: context
            .consumption_plan
            .as_ref()
            .unwrap()
            .derivation
            .as_ref()
            .unwrap()
            .plan_digest
            .clone(),
        facts_digest: canonical_digest(&facts).unwrap(),
        facts,
    };
    let graph_bytes = cognigraph_governance::canonical_json_bytes(&graph_artifact).unwrap();
    let graph = store_signed_manifest_attestation(
        state,
        attestor,
        &format!(
            "{}-graph-{artifact_scope}-{forge_evidence_chunk}-{forge_prepared_output}",
            if prepare_from_raw { "m23" } else { "m22" }
        ),
        staged_m22_graph_manifest(cas_root, &candidate_bytes, &graph_bytes),
        ArtifactSubject::Graph {
            revision: context.revisions.graph.clone(),
            candidate_digest,
            construction_config_digest,
            corpus_manifest_digest: corpus_manifest_digest.clone(),
            preprocessing_digest: context.effective_configuration.preprocessing_digest.clone(),
        },
    )
    .await;

    let (oracle_attestation_id, scorer_attestation_id, verifier_attestation_id) =
        if let Some(shared) = shared.filter(|_| !forge_prepared_output) {
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
            let oracle_bytes = serde_json::to_vec(&oracle_artifact).unwrap();
            let oracle = store_signed_manifest_attestation(
                state,
                attestor,
                &format!(
                    "{}-{artifact_scope}-oracle-{}",
                    if prepare_from_raw { "m23" } else { "m22" },
                    if forge_prepared_output {
                        "forged"
                    } else {
                        "valid"
                    }
                ),
                staged_manifest(
                    cas_root,
                    ArtifactKind::Oracle,
                    ORACLE_ARTIFACT_FORMAT,
                    ORACLE_ENTRYPOINT,
                    "application/json",
                    false,
                    &oracle_bytes,
                ),
                ArtifactSubject::Oracle {
                    revision: context.revisions.oracle.clone(),
                    eval_spec_digest: context.effective_configuration.eval_spec_digest.clone(),
                    case_manifest_digest: context.case_manifest.digest.clone(),
                    corpus_manifest_digest: corpus_manifest_digest.clone(),
                },
            )
            .await;
            let (scorer_attestation_id, verifier_attestation_id) = if let Some(shared) = shared {
                (
                    shared.scorer.attestation_id.clone(),
                    shared.verifier.attestation_id.clone(),
                )
            } else {
                let scorer = store_signed_manifest_attestation(
                    state,
                    attestor,
                    &format!(
                        "{}-{artifact_scope}-scorer",
                        if prepare_from_raw { "m23" } else { "m22" }
                    ),
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
                    &format!(
                        "{}-{artifact_scope}-verifier",
                        if prepare_from_raw { "m23" } else { "m22" }
                    ),
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
                (scorer.attestation_id, verifier.attestation_id)
            };
            (
                oracle.attestation_id,
                scorer_attestation_id,
                verifier_attestation_id,
            )
        };

    context.artifact_attestations = Some(
        state
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
            .unwrap(),
    );
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
