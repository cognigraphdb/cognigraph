//! Consume.

use super::*;

impl PromotionManager {
    /// Verify and consume the exact five M20 manifests selected by an M21
    /// context. Signed locations and logical paths never become filesystem
    /// inputs: the local CAS receives only tenant scope, digest, and length.
    pub(crate) async fn consume_verified_evaluation_inputs(
        &self,
        cas: &LocalArtifactCas,
        pinned_executable_digest: &str,
        job: &ConsumptionJobBinding<'_>,
        resolved_eval_spec: &EvalSpec,
        context: &PromotionContext,
    ) -> Result<ConsumedEvaluationOutcome, CogniGraphError> {
        let plan = context
            .consumption_plan
            .as_ref()
            .ok_or_else(|| validation("M21 evaluation requires an artifact consumption plan"))?;
        plan.validate()?;
        let set = context.artifact_attestations.as_ref().ok_or_else(|| {
            validation("M21 evaluation requires all five artifact attestation bindings")
        })?;
        let started_at_ms = now_millis();
        let active = self
            .active_artifact_attestations(
                job.tenant,
                job.tenant_incarnation,
                context,
                set,
                started_at_ms,
            )
            .await?;
        validate_manifest_contracts(&active, plan)?;

        validate_digest("pinned_executable_digest", pinned_executable_digest)?;
        if context.reproducibility.executable_digest != pinned_executable_digest {
            return Err(validation(
                "startup-pinned server executable-path digest does not match the M21 reproducibility digest",
            ));
        }

        let mut budget = cas.begin_evaluation(job.tenant, job.tenant_incarnation)?;
        let entry_count = [
            active.corpus.manifest.entry_count,
            active.graph.manifest.entry_count,
            active.oracle.manifest.entry_count,
            active.scorer.manifest.entry_count,
            active.verifier.manifest.entry_count,
        ]
        .into_iter()
        .try_fold(0_u64, |total, count| total.checked_add(count))
        .ok_or_else(|| validation("artifact manifest entry accounting overflowed"))?;
        if entry_count > MAX_EVALUATION_MANIFEST_ENTRIES {
            return Err(CogniGraphError::CapacityExceeded(format!(
                "M21 evaluation names {entry_count} manifest entries, exceeding the {MAX_EVALUATION_MANIFEST_ENTRIES} entry limit"
            )));
        }
        let mut verified_blobs = HashMap::new();
        let derivation_plan = plan.derivation.as_deref();
        let preparation_plan = plan.preparation.as_deref();
        let corpus_retain = if let Some(preparation) = preparation_plan {
            vec![
                (
                    CORPUS_ENTRYPOINT,
                    preparation.prepared_corpus_retention_bytes,
                ),
                (DOCUMENTS_ENTRYPOINT, preparation.documents_retention_bytes),
            ]
        } else {
            derivation_plan
                .map(|derivation| vec![(CORPUS_ENTRYPOINT, derivation.corpus_retention_bytes)])
                .unwrap_or_default()
        };
        let graph_retain = if let Some(derivation) = derivation_plan {
            vec![
                (GRAPH_ENTRYPOINT, derivation.graph_retention_bytes),
                (CANDIDATE_ENTRYPOINT, derivation.candidate_retention_bytes),
            ]
        } else {
            vec![(GRAPH_ENTRYPOINT, MAX_GRAPH_ARTIFACT_BYTES)]
        };
        let mut corpus = consume_manifest(
            cas,
            &mut budget,
            &mut verified_blobs,
            &active.corpus,
            &plan.corpus,
            ArtifactConsumptionPurpose::CorpusProvenance,
            &corpus_retain,
        )
        .await?;
        let mut graph = consume_manifest(
            cas,
            &mut budget,
            &mut verified_blobs,
            &active.graph,
            &plan.graph,
            ArtifactConsumptionPurpose::EvaluatedGraph,
            &graph_retain,
        )
        .await?;
        let mut oracle = consume_manifest(
            cas,
            &mut budget,
            &mut verified_blobs,
            &active.oracle,
            &plan.oracle,
            ArtifactConsumptionPurpose::PromotionOracle,
            &[(ORACLE_ENTRYPOINT, MAX_ORACLE_ARTIFACT_BYTES)],
        )
        .await?;
        let mut scorer = consume_manifest(
            cas,
            &mut budget,
            &mut verified_blobs,
            &active.scorer,
            &plan.scorer,
            ArtifactConsumptionPurpose::ScorerExecutable,
            &[],
        )
        .await?;
        let mut verifier = consume_manifest(
            cas,
            &mut budget,
            &mut verified_blobs,
            &active.verifier,
            &plan.verifier,
            ArtifactConsumptionPurpose::VerifierExecutable,
            &[],
        )
        .await?;

        let oracle_artifact: PromotionOracleArtifact =
            serde_json::from_slice(oracle.retained(ORACLE_ENTRYPOINT)?).map_err(|error| {
                validation(format!("invalid verified oracle artifact JSON: {error}"))
            })?;
        oracle_artifact.validate(context, &active.corpus.manifest_digest, resolved_eval_spec)?;
        oracle.receipt.semantic_digest = canonical_digest(&oracle_artifact)?;

        let (facts, derivation) = if let Some(derivation_plan) = derivation_plan {
            let corpus_bytes = corpus.retained(CORPUS_ENTRYPOINT)?;
            let claimed_corpus: PreparedChunkCorpusArtifact = serde_json::from_slice(corpus_bytes)
                .map_err(|error| {
                    validation(format!("invalid prepared corpus artifact JSON: {error}"))
                })?;
            require_canonical_json("prepared corpus artifact", corpus_bytes, &claimed_corpus)?;
            let (corpus_artifact, preparation_receipt) = if let Some(preparation_plan) =
                preparation_plan
            {
                let documents_bytes = corpus.retained(DOCUMENTS_ENTRYPOINT)?;
                let raw_artifact: RawDocumentSetArtifact = serde_json::from_slice(documents_bytes)
                    .map_err(|error| {
                        validation(format!("invalid raw document set JSON: {error}"))
                    })?;
                require_canonical_json(
                    "raw document set artifact",
                    documents_bytes,
                    &raw_artifact,
                )?;
                let documents_entry = require_manifest_entry(&active.corpus, DOCUMENTS_ENTRYPOINT)?;
                let documents_semantic_digest = canonical_digest(&raw_artifact)?;
                if documents_semantic_digest != documents_entry.blob_digest {
                    return Err(validation(
                        "canonical raw document set digest does not match its exact-byte content address",
                    ));
                }
                let decoded = raw_artifact.decode(context, preparation_plan)?;
                let prepared = prepare_documents(
                    &decoded.documents,
                    PreparationOptions {
                        max_document_count: preparation_plan.max_documents as usize,
                        max_document_bytes: preparation_plan.max_raw_document_bytes as usize,
                        max_total_document_bytes: preparation_plan.max_total_raw_document_bytes
                            as usize,
                        max_normalized_document_bytes: preparation_plan
                            .max_normalized_document_bytes
                            as usize,
                        max_total_normalized_bytes: preparation_plan.max_total_normalized_bytes
                            as usize,
                        max_chunk_bytes: preparation_plan.max_chunk_bytes as usize,
                        max_chunk_count: preparation_plan.max_chunks as usize,
                        max_total_prepared_bytes: preparation_plan.max_total_prepared_text_bytes
                            as usize,
                        yield_every_documents: preparation_plan.yield_every_documents as usize,
                    },
                )
                .await
                .map_err(|error| validation(format!("M23 corpus preparation failed: {error}")))?;
                let reproduced = PreparedChunkCorpusArtifact {
                    schema_version: 1,
                    space_type: context.target.space_type.clone(),
                    corpus_revision_id: context.revisions.corpus.revision_id.clone(),
                    preprocessing_digest: preparation_plan.plan_digest.clone(),
                    chunks: prepared
                        .into_iter()
                        .map(|chunk| PreparedChunkArtifact {
                            id: chunk.id,
                            title: chunk.title,
                            text: chunk.text,
                        })
                        .collect(),
                };
                reproduced.validate(context, derivation_plan)?;
                let reproduced_bytes = cognigraph_governance::canonical_json_bytes(&reproduced)
                    .map_err(|error| {
                        validation(format!(
                            "cannot canonicalize reproduced prepared corpus: {error}"
                        ))
                    })?;
                let corpus_entry = require_manifest_entry(&active.corpus, CORPUS_ENTRYPOINT)?;
                let reproduced_digest = digest_bytes(&reproduced_bytes);
                if reproduced != claimed_corpus
                    || reproduced_bytes != corpus_bytes
                    || reproduced_digest != corpus_entry.blob_digest
                    || reproduced_bytes.len() as u64 != corpus_entry.byte_length
                {
                    return Err(validation(
                        "attested prepared corpus does not exactly reproduce from the verified raw document bytes",
                    ));
                }
                // The signed canonical documents.json content address is the
                // durable raw-document-set authority. Do not persist derived
                // counts or inventories that cannot be re-proven without the
                // external CAS bytes during offline snapshot preflight.
                let raw_document_set_digest = documents_semantic_digest.clone();
                let preparation_read_set_digest = canonical_digest(&json!({
                    "corpus_manifest_digest": active.corpus.manifest_digest,
                    "documents_blob_digest": documents_entry.blob_digest,
                    "documents_semantic_digest": documents_semantic_digest,
                    "raw_document_set_digest": raw_document_set_digest,
                    "preparation_plan_digest": preparation_plan.plan_digest,
                }))?;
                let mut receipt = RawCorpusPreparationReceipt {
                    schema_version: M23_PREPARATION_RECEIPT_SCHEMA_VERSION,
                    preparer_id: preparation_plan.preparer_id.clone(),
                    preparer_version: preparation_plan.preparer_version.clone(),
                    preparer_semantics_digest: preparation_plan.preparer_semantics_digest.clone(),
                    preparation_abi_digest: preparation_plan.preparation_abi_digest.clone(),
                    preparation_plan_digest: preparation_plan.plan_digest.clone(),
                    corpus_manifest_digest: active.corpus.manifest_digest.clone(),
                    documents_blob_digest: documents_entry.blob_digest.clone(),
                    documents_semantic_digest,
                    raw_document_set_digest,
                    prepared_corpus_blob_digest: corpus_entry.blob_digest.clone(),
                    prepared_corpus_semantic_digest: reproduced_digest,
                    preparation_read_set_digest,
                    preparation_material_digest: String::new(),
                };
                receipt.preparation_material_digest =
                    record_digest(&receipt, "preparation_material_digest")?;
                receipt.validate(
                    context,
                    preparation_plan,
                    &active.corpus.manifest_digest,
                    documents_entry,
                    corpus_entry,
                )?;
                (reproduced, Some(Box::new(receipt)))
            } else {
                claimed_corpus.validate(context, derivation_plan)?;
                (claimed_corpus, None)
            };
            let corpus_semantic_digest = canonical_digest(&corpus_artifact)?;
            corpus.receipt.semantic_digest = corpus_semantic_digest.clone();

            let candidate_bytes = graph.retained(CANDIDATE_ENTRYPOINT)?;
            let candidate_artifact: ConstructionCandidateArtifact =
                serde_json::from_slice(candidate_bytes).map_err(|error| {
                    validation(format!("invalid construction candidate JSON: {error}"))
                })?;
            let candidate_entry = require_manifest_entry(&active.graph, CANDIDATE_ENTRYPOINT)?;
            if candidate_entry.blob_digest != context.candidate.candidate_digest
                || candidate_entry.byte_length != context.candidate.artifact.bytes
                || context.candidate.artifact.media_type != "application/json"
            {
                return Err(validation(
                    "verified construction candidate bytes do not match the M22 candidate identity",
                ));
            }
            require_canonical_json(
                "construction candidate artifact",
                candidate_bytes,
                &candidate_artifact,
            )?;
            let candidate_semantic_digest = canonical_digest(&candidate_artifact)?;
            if candidate_semantic_digest != candidate_entry.blob_digest {
                return Err(validation(
                    "canonical construction candidate digest does not match its exact-byte content address",
                ));
            }
            let chunks = corpus_artifact.chunks();
            let (effective, vetoes, resolved_config_digest) =
                candidate_artifact.resolve(context, derivation_plan, &chunks)?;

            let graph_bytes = graph.retained(GRAPH_ENTRYPOINT)?;
            let graph_artifact: ReproducibleEvaluationGraphArtifact =
                serde_json::from_slice(graph_bytes).map_err(|error| {
                    validation(format!("invalid reproducible graph artifact JSON: {error}"))
                })?;
            graph_artifact.validate_claim(
                context,
                &active.corpus.manifest_digest,
                &corpus_semantic_digest,
                &derivation_plan.plan_digest,
            )?;
            require_canonical_json(
                "reproducible evaluation graph artifact",
                graph_bytes,
                &graph_artifact,
            )?;

            let rows = cognigraph_construct::derive_fact_rows(
                &chunks,
                &effective,
                &vetoes,
                cognigraph_construct::DerivationOptions {
                    max_fact_count: derivation_plan.max_derived_facts as usize,
                    yield_every_chunks: 1,
                },
            )
            .await
            .map_err(|error| validation(format!("M22 graph derivation failed: {error}")))?;
            let derived_facts = rows
                .into_iter()
                .map(|row| VerifiedGraphFact {
                    source: row.source,
                    relation: row.relation,
                    target: row.target,
                    evidence_chunk_id: row.evidence_chunk_id,
                })
                .collect::<Vec<_>>();
            let facts_digest = canonical_digest(&derived_facts)?;
            let derived_graph = ReproducibleEvaluationGraphArtifact {
                schema_version: 1,
                space_type: context.target.space_type.clone(),
                graph_revision_id: context.revisions.graph.revision_id.clone(),
                corpus_manifest_digest: active.corpus.manifest_digest.clone(),
                corpus_semantic_digest: corpus_semantic_digest.clone(),
                candidate_digest: context.candidate.candidate_digest.clone(),
                construction_config_digest: resolved_config_digest.clone(),
                derivation_plan_digest: derivation_plan.plan_digest.clone(),
                facts_digest: facts_digest.clone(),
                facts: derived_facts,
            };
            derived_graph.validate_claim(
                context,
                &active.corpus.manifest_digest,
                &corpus_semantic_digest,
                &derivation_plan.plan_digest,
            )?;
            let derived_graph_bytes = cognigraph_governance::canonical_json_bytes(&derived_graph)
                .map_err(|error| {
                validation(format!("cannot canonicalize derived graph: {error}"))
            })?;
            let derived_graph_blob_digest = digest_bytes(&derived_graph_bytes);
            let graph_entry = require_manifest_entry(&active.graph, GRAPH_ENTRYPOINT)?;
            if derived_graph != graph_artifact
                || derived_graph_bytes != graph_bytes
                || derived_graph_blob_digest != graph_entry.blob_digest
            {
                return Err(validation(
                    "attested graph does not exactly reproduce from the verified corpus and construction candidate",
                ));
            }
            let graph_semantic_digest = canonical_digest(&derived_graph)?;
            graph.receipt.semantic_digest = graph_semantic_digest.clone();
            let mut construction_read_set = json!({
                "corpus_manifest_digest": active.corpus.manifest_digest,
                "corpus_blob_digest": require_manifest_entry(&active.corpus, CORPUS_ENTRYPOINT)?.blob_digest,
                "corpus_semantic_digest": corpus_semantic_digest,
                "candidate_blob_digest": candidate_entry.blob_digest,
                "candidate_semantic_digest": candidate_semantic_digest,
                "resolved_config_digest": resolved_config_digest,
                "derivation_plan_digest": derivation_plan.plan_digest,
            });
            if let Some(preparation) = &preparation_receipt {
                construction_read_set
                    .as_object_mut()
                    .expect("construction read set is an object")
                    .insert(
                        "preparation_material_digest".into(),
                        json!(preparation.preparation_material_digest),
                    );
            }
            let construction_read_set_digest = canonical_digest(&construction_read_set)?;
            let mut derivation = CorpusGraphDerivationReceipt {
                schema_version: if preparation_receipt.is_some() {
                    M23_DERIVATION_RECEIPT_SCHEMA_VERSION
                } else {
                    M22_DERIVATION_RECEIPT_SCHEMA_VERSION
                },
                deriver_id: derivation_plan.deriver_id.clone(),
                deriver_version: derivation_plan.deriver_version.clone(),
                deriver_semantics_digest: derivation_plan.deriver_semantics_digest.clone(),
                derivation_abi_digest: derivation_plan.derivation_abi_digest.clone(),
                derivation_plan_digest: derivation_plan.plan_digest.clone(),
                corpus_manifest: active.corpus.manifest.clone(),
                graph_manifest: active.graph.manifest.clone(),
                oracle_manifest: active.oracle.manifest.clone(),
                corpus_manifest_digest: active.corpus.manifest_digest.clone(),
                corpus_blob_digest: require_manifest_entry(&active.corpus, CORPUS_ENTRYPOINT)?
                    .blob_digest
                    .clone(),
                corpus_semantic_digest,
                chunk_count: if preparation_receipt.is_some() {
                    None
                } else {
                    Some(chunks.len() as u64)
                },
                preparation: preparation_receipt,
                candidate_blob_digest: candidate_entry.blob_digest.clone(),
                candidate_semantic_digest,
                resolved_config_digest,
                claimed_graph_blob_digest: graph_entry.blob_digest.clone(),
                claimed_graph_semantic_digest: graph_semantic_digest.clone(),
                derived_graph_blob_digest,
                derived_graph_semantic_digest: graph_semantic_digest,
                facts_digest,
                fact_count: derived_graph.facts.len() as u64,
                facts: derived_graph.facts.clone(),
                oracle_json: String::from_utf8(oracle.retained(ORACLE_ENTRYPOINT)?.to_vec())
                    .map_err(|error| {
                        validation(format!("oracle artifact is not UTF-8: {error}"))
                    })?,
                construction_read_set_digest,
                derivation_material_digest: String::new(),
            };
            derivation.derivation_material_digest =
                record_digest(&derivation, "derivation_material_digest")?;
            (derived_graph.scoring_facts(), Some(Box::new(derivation)))
        } else {
            let graph_artifact: EvaluationGraphArtifact =
                serde_json::from_slice(graph.retained(GRAPH_ENTRYPOINT)?).map_err(|error| {
                    validation(format!("invalid verified graph artifact JSON: {error}"))
                })?;
            graph_artifact.validate(context, &active.corpus.manifest_digest)?;
            graph.receipt.semantic_digest = canonical_digest(&graph_artifact)?;
            (graph_artifact.scoring_facts(), None)
        };

        let scorer_entry = active
            .scorer
            .manifest
            .entries
            .first()
            .expect("manifest contract requires one scorer entry");
        let verifier_entry = active
            .verifier
            .manifest
            .entries
            .first()
            .expect("manifest contract requires one verifier entry");
        if scorer_entry.blob_digest != pinned_executable_digest
            || verifier_entry.blob_digest != pinned_executable_digest
        {
            return Err(validation(
                "verified scorer and verifier bytes must match the startup-pinned server executable-path digest",
            ));
        }
        scorer.receipt.semantic_digest = pinned_executable_digest.into();
        verifier.receipt.semantic_digest = pinned_executable_digest.into();

        let outcome = evaluate_facts(
            &oracle_artifact.eval_spec.space_id,
            &oracle_artifact.eval_spec,
            &facts,
        );
        let result = evaluation_result_value(&outcome);
        let evaluation_result_digest = canonical_digest(&result)?;

        // A revocation that lands while bytes are being consumed blocks this
        // execution, including the in-process scoring step. A later revocation
        // preserves the historical receipt but prevents fresh use.
        // Key revocation and every other authority mutation use this same
        // transition lock. Hold it across the final active read and receipt
        // cutoff so a revocation cannot commit in between them.
        let authority_guard = self.transition_lock.lock().await;
        let completed_at_ms = now_millis();
        let final_active = self
            .active_artifact_attestations(
                job.tenant,
                job.tenant_incarnation,
                context,
                set,
                completed_at_ms,
            )
            .await?;
        ensure_same_active_bindings(&active, &final_active)?;

        let mut receipt = ArtifactConsumptionReceipt {
            schema_version: if plan.preparation.is_some() {
                M23_CONSUMPTION_RECEIPT_SCHEMA_VERSION
            } else if derivation.is_some() {
                M22_CONSUMPTION_RECEIPT_SCHEMA_VERSION
            } else {
                M21_CONSUMPTION_RECEIPT_SCHEMA_VERSION
            },
            digest_algorithm: DIGEST_ALGORITHM.into(),
            resolver: LOCAL_CAS_RESOLVER.into(),
            tenant: job.tenant.into(),
            tenant_incarnation: job.tenant_incarnation.into(),
            job_id: job.job_id.into(),
            attempt: job.attempt,
            recoveries: job.recoveries,
            input_digest: job.input_digest.into(),
            execution_payload_digest: job.execution_payload_digest.into(),
            eval_spec_digest: job.eval_spec_digest.into(),
            evaluation_result_digest,
            plan_digest: plan.plan_digest.clone(),
            context_digest: context.digest()?,
            artifact_set_digest: set.set_digest.clone(),
            corpus: corpus.receipt,
            graph: graph.receipt,
            oracle: oracle.receipt,
            scorer: scorer.receipt,
            verifier: verifier.receipt,
            derivation,
            started_at_ms,
            completed_at_ms,
            material_digest: String::new(),
            receipt_digest: String::new(),
        };
        receipt.material_digest = receipt.expected_material_digest()?;
        receipt.receipt_digest = record_digest(&receipt, "receipt_digest")?;
        receipt.validate(context)?;
        receipt.validate_job_binding(job)?;
        receipt.validate_evaluation_result(&result)?;
        drop(authority_guard);

        Ok(ConsumedEvaluationOutcome { result, receipt })
    }
}
