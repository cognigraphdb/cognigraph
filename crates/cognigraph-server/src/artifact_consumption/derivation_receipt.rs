//! Derivation receipt.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusGraphDerivationReceipt {
    pub schema_version: u32,
    pub deriver_id: String,
    pub deriver_version: String,
    pub deriver_semantics_digest: String,
    pub derivation_abi_digest: String,
    pub derivation_plan_digest: String,
    /// Exact signed-manifest material needed to re-establish the M22/M23
    /// derivation content addresses without dereferencing the CAS. Their
    /// canonical digests must match the immutable M20 bindings in the context.
    pub corpus_manifest: ArtifactManifest,
    pub graph_manifest: ArtifactManifest,
    pub oracle_manifest: ArtifactManifest,
    pub corpus_manifest_digest: String,
    pub corpus_blob_digest: String,
    pub corpus_semantic_digest: String,
    /// Historical M22 receipts carry the observed prepared-corpus count. M23
    /// omits it because its compact address-durable receipt cannot re-prove a
    /// copied count without dereferencing the external corpus bytes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub chunk_count: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_non_null_option"
    )]
    pub preparation: Option<Box<RawCorpusPreparationReceipt>>,
    pub candidate_blob_digest: String,
    pub candidate_semantic_digest: String,
    pub resolved_config_digest: String,
    pub claimed_graph_blob_digest: String,
    pub claimed_graph_semantic_digest: String,
    pub derived_graph_blob_digest: String,
    pub derived_graph_semantic_digest: String,
    pub facts_digest: String,
    pub fact_count: u64,
    /// Exact evidence-bearing rows used to rebuild the signed graph package
    /// during offline recovery and snapshot preflight.
    pub facts: Vec<VerifiedGraphFact>,
    /// Exact verified oracle JSON bytes, represented losslessly as UTF-8, used
    /// to re-establish its signed content address and recompute the score
    /// during offline recovery and snapshot preflight.
    pub oracle_json: String,
    pub construction_read_set_digest: String,
    pub derivation_material_digest: String,
}
impl CorpusGraphDerivationReceipt {
    pub(super) fn oracle_artifact(&self) -> Result<PromotionOracleArtifact, CogniGraphError> {
        serde_json::from_str(&self.oracle_json).map_err(|error| {
            validation(format!(
                "invalid durable derivation oracle artifact JSON: {error}"
            ))
        })
    }

    pub(super) fn validate(
        &self,
        context: &PromotionContext,
        plan: &CorpusGraphDerivationPlan,
        preparation_plan: Option<&RawCorpusPreparationPlan>,
        corpus: &ConsumedArtifactReceipt,
        graph: &ConsumedArtifactReceipt,
        oracle: &ConsumedArtifactReceipt,
    ) -> Result<(), CogniGraphError> {
        plan.validate()?;
        let (corpus_entry, expected_schema_version) = if preparation_plan.is_some() {
            let corpus_entries = validate_receipt_manifest(
                &self.corpus_manifest,
                corpus,
                ArtifactKind::Corpus,
                M23_CORPUS_ARTIFACT_FORMAT,
                &[CORPUS_ENTRYPOINT, DOCUMENTS_ENTRYPOINT],
            )?;
            (corpus_entries[0], M23_DERIVATION_RECEIPT_SCHEMA_VERSION)
        } else {
            let corpus_entries = validate_receipt_manifest(
                &self.corpus_manifest,
                corpus,
                ArtifactKind::Corpus,
                M22_CORPUS_ARTIFACT_FORMAT,
                &[CORPUS_ENTRYPOINT],
            )?;
            (corpus_entries[0], M22_DERIVATION_RECEIPT_SCHEMA_VERSION)
        };
        let graph_entries = validate_receipt_manifest(
            &self.graph_manifest,
            graph,
            ArtifactKind::Graph,
            M22_GRAPH_ARTIFACT_FORMAT,
            &[CANDIDATE_ENTRYPOINT, GRAPH_ENTRYPOINT],
        )?;
        let candidate_entry = graph_entries[0];
        let graph_entry = graph_entries[1];
        let oracle_entries = validate_receipt_manifest(
            &self.oracle_manifest,
            oracle,
            ArtifactKind::Oracle,
            ORACLE_ARTIFACT_FORMAT,
            &[ORACLE_ENTRYPOINT],
        )?;
        let oracle_entry = oracle_entries[0];
        let chunk_count_matches = match expected_schema_version {
            M22_DERIVATION_RECEIPT_SCHEMA_VERSION => self
                .chunk_count
                .is_some_and(|count| count > 0 && count <= plan.max_chunks),
            M23_DERIVATION_RECEIPT_SCHEMA_VERSION => self.chunk_count.is_none(),
            _ => false,
        };
        if self.schema_version != expected_schema_version
            || self.deriver_id != plan.deriver_id
            || self.deriver_version != plan.deriver_version
            || self.deriver_semantics_digest != plan.deriver_semantics_digest
            || self.derivation_abi_digest != plan.derivation_abi_digest
            || self.derivation_plan_digest != plan.plan_digest
            || self.corpus_manifest_digest != corpus.manifest_digest
            || self.corpus_manifest_digest != canonical_digest(&self.corpus_manifest)?
            || self.corpus_semantic_digest != corpus.semantic_digest
            || self.corpus_blob_digest != corpus_entry.blob_digest
            || self.corpus_semantic_digest != self.corpus_blob_digest
            || corpus_entry.byte_length > plan.corpus_retention_bytes
            || self.resolved_config_digest
                != context.effective_configuration.construction_config_digest
            || self.candidate_blob_digest != context.candidate.candidate_digest
            || self.candidate_blob_digest != candidate_entry.blob_digest
            || candidate_entry.byte_length != context.candidate.artifact.bytes
            || candidate_entry.byte_length > plan.candidate_retention_bytes
            || self.candidate_semantic_digest != self.candidate_blob_digest
            || self.claimed_graph_blob_digest != graph_entry.blob_digest
            || graph_entry.byte_length > plan.graph_retention_bytes
            || self.claimed_graph_blob_digest != self.derived_graph_blob_digest
            || self.claimed_graph_semantic_digest != self.derived_graph_semantic_digest
            || self.claimed_graph_semantic_digest != self.claimed_graph_blob_digest
            || self.derived_graph_semantic_digest != graph.semantic_digest
            || self.fact_count > plan.max_derived_facts
            || !chunk_count_matches
            || self.fact_count != self.facts.len() as u64
            || self.facts_digest != canonical_digest(&self.facts)?
        {
            return Err(validation(
                "corpus-to-graph derivation receipt does not match its context, plan, consumed artifacts, or signed manifest entries",
            ));
        }
        match (preparation_plan, self.preparation.as_deref()) {
            (None, None) => {}
            (Some(preparation_plan), Some(preparation)) => {
                let documents_entry = self
                    .corpus_manifest
                    .entries
                    .iter()
                    .find(|entry| entry.logical_path == DOCUMENTS_ENTRYPOINT)
                    .expect("validated M23 receipt manifest carries documents.json");
                preparation.validate(
                    context,
                    preparation_plan,
                    &self.corpus_manifest_digest,
                    documents_entry,
                    corpus_entry,
                )?;
            }
            _ => {
                return Err(validation(
                    "corpus-to-graph derivation receipt preparation generation mismatch",
                ));
            }
        }
        for (label, digest) in [
            ("deriver_semantics_digest", &self.deriver_semantics_digest),
            ("derivation_abi_digest", &self.derivation_abi_digest),
            ("derivation_plan_digest", &self.derivation_plan_digest),
            ("corpus_manifest_digest", &self.corpus_manifest_digest),
            ("corpus_blob_digest", &self.corpus_blob_digest),
            ("corpus_semantic_digest", &self.corpus_semantic_digest),
            ("candidate_blob_digest", &self.candidate_blob_digest),
            ("candidate_semantic_digest", &self.candidate_semantic_digest),
            ("resolved_config_digest", &self.resolved_config_digest),
            ("claimed_graph_blob_digest", &self.claimed_graph_blob_digest),
            (
                "claimed_graph_semantic_digest",
                &self.claimed_graph_semantic_digest,
            ),
            ("derived_graph_blob_digest", &self.derived_graph_blob_digest),
            (
                "derived_graph_semantic_digest",
                &self.derived_graph_semantic_digest,
            ),
            ("facts_digest", &self.facts_digest),
            (
                "construction_read_set_digest",
                &self.construction_read_set_digest,
            ),
            (
                "derivation_material_digest",
                &self.derivation_material_digest,
            ),
        ] {
            validate_digest(label, digest)?;
        }
        if self.derivation_material_digest != record_digest(self, "derivation_material_digest")? {
            return Err(validation(
                "corpus-to-graph derivation receipt digest mismatch",
            ));
        }
        let mut expected_read_set = json!({
            "corpus_manifest_digest": &self.corpus_manifest_digest,
            "corpus_blob_digest": &self.corpus_blob_digest,
            "corpus_semantic_digest": &self.corpus_semantic_digest,
            "candidate_blob_digest": &self.candidate_blob_digest,
            "candidate_semantic_digest": &self.candidate_semantic_digest,
            "resolved_config_digest": &self.resolved_config_digest,
            "derivation_plan_digest": &self.derivation_plan_digest,
        });
        if let Some(preparation) = &self.preparation {
            expected_read_set
                .as_object_mut()
                .expect("construction read set is an object")
                .insert(
                    "preparation_material_digest".into(),
                    json!(preparation.preparation_material_digest),
                );
        }
        let expected_read_set_digest = canonical_digest(&expected_read_set)?;
        if self.construction_read_set_digest != expected_read_set_digest {
            return Err(validation(
                "corpus-to-graph derivation construction read-set digest mismatch",
            ));
        }
        validate_graph_facts(&self.facts)?;
        let reconstructed_graph = ReproducibleEvaluationGraphArtifact {
            schema_version: 1,
            space_type: context.target.space_type.clone(),
            graph_revision_id: context.revisions.graph.revision_id.clone(),
            corpus_manifest_digest: self.corpus_manifest_digest.clone(),
            corpus_semantic_digest: self.corpus_semantic_digest.clone(),
            candidate_digest: self.candidate_blob_digest.clone(),
            construction_config_digest: self.resolved_config_digest.clone(),
            derivation_plan_digest: self.derivation_plan_digest.clone(),
            facts_digest: self.facts_digest.clone(),
            facts: self.facts.clone(),
        };
        reconstructed_graph.validate_claim(
            context,
            &self.corpus_manifest_digest,
            &self.corpus_semantic_digest,
            &self.derivation_plan_digest,
        )?;
        let reconstructed_graph_bytes =
            cognigraph_governance::canonical_json_bytes(&reconstructed_graph).map_err(|error| {
                validation(format!(
                    "cannot canonicalize receipt graph projection: {error}"
                ))
            })?;
        if digest_bytes(&reconstructed_graph_bytes) != graph_entry.blob_digest
            || reconstructed_graph_bytes.len() as u64 != graph_entry.byte_length
        {
            return Err(validation(
                "receipt facts do not reconstruct the signed graph manifest entry",
            ));
        }
        let oracle_artifact = self.oracle_artifact()?;
        oracle_artifact.validate(
            context,
            &self.corpus_manifest_digest,
            &oracle_artifact.eval_spec,
        )?;
        if digest_bytes(self.oracle_json.as_bytes()) != oracle_entry.blob_digest
            || self.oracle_json.len() as u64 != oracle_entry.byte_length
            || canonical_digest(&oracle_artifact)? != oracle.semantic_digest
        {
            return Err(validation(
                "receipt oracle does not reconstruct the signed oracle manifest entry",
            ));
        }
        Ok(())
    }
}
