//! Preparation receipt.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawCorpusPreparationReceipt {
    pub schema_version: u32,
    pub preparer_id: String,
    pub preparer_version: String,
    pub preparer_semantics_digest: String,
    pub preparation_abi_digest: String,
    pub preparation_plan_digest: String,
    pub corpus_manifest_digest: String,
    pub documents_blob_digest: String,
    pub documents_semantic_digest: String,
    pub raw_document_set_digest: String,
    pub prepared_corpus_blob_digest: String,
    pub prepared_corpus_semantic_digest: String,
    pub preparation_read_set_digest: String,
    pub preparation_material_digest: String,
}
impl RawCorpusPreparationReceipt {
    pub(super) fn validate(
        &self,
        context: &PromotionContext,
        plan: &RawCorpusPreparationPlan,
        corpus_manifest_digest: &str,
        documents_entry: &crate::artifact_attestations::ArtifactManifestEntry,
        corpus_entry: &crate::artifact_attestations::ArtifactManifestEntry,
    ) -> Result<(), CogniGraphError> {
        plan.validate()?;
        if self.schema_version != M23_PREPARATION_RECEIPT_SCHEMA_VERSION
            || self.preparer_id != plan.preparer_id
            || self.preparer_version != plan.preparer_version
            || self.preparer_semantics_digest != plan.preparer_semantics_digest
            || self.preparation_abi_digest != plan.preparation_abi_digest
            || self.preparation_plan_digest != plan.plan_digest
            || context.effective_configuration.preprocessing_digest != plan.plan_digest
            || self.corpus_manifest_digest != corpus_manifest_digest
            || self.documents_blob_digest != documents_entry.blob_digest
            || self.documents_semantic_digest != self.documents_blob_digest
            || self.raw_document_set_digest != self.documents_semantic_digest
            || documents_entry.byte_length > plan.documents_retention_bytes
            || self.prepared_corpus_blob_digest != corpus_entry.blob_digest
            || self.prepared_corpus_semantic_digest != self.prepared_corpus_blob_digest
            || corpus_entry.byte_length > plan.prepared_corpus_retention_bytes
        {
            return Err(validation(
                "raw-document preparation receipt does not match its M23 context, plan, signed corpus package, or limits",
            ));
        }
        for (label, digest) in [
            ("preparer_semantics_digest", &self.preparer_semantics_digest),
            ("preparation_abi_digest", &self.preparation_abi_digest),
            ("preparation_plan_digest", &self.preparation_plan_digest),
            ("corpus_manifest_digest", &self.corpus_manifest_digest),
            ("documents_blob_digest", &self.documents_blob_digest),
            ("documents_semantic_digest", &self.documents_semantic_digest),
            ("raw_document_set_digest", &self.raw_document_set_digest),
            (
                "prepared_corpus_blob_digest",
                &self.prepared_corpus_blob_digest,
            ),
            (
                "prepared_corpus_semantic_digest",
                &self.prepared_corpus_semantic_digest,
            ),
            (
                "preparation_read_set_digest",
                &self.preparation_read_set_digest,
            ),
            (
                "preparation_material_digest",
                &self.preparation_material_digest,
            ),
        ] {
            validate_digest(label, digest)?;
        }
        let expected_read_set_digest = canonical_digest(&json!({
            "corpus_manifest_digest": &self.corpus_manifest_digest,
            "documents_blob_digest": &self.documents_blob_digest,
            "documents_semantic_digest": &self.documents_semantic_digest,
            "raw_document_set_digest": &self.raw_document_set_digest,
            "preparation_plan_digest": &self.preparation_plan_digest,
        }))?;
        if self.preparation_read_set_digest != expected_read_set_digest
            || self.preparation_material_digest
                != record_digest(self, "preparation_material_digest")?
        {
            return Err(validation(
                "raw-document preparation receipt digest mismatch",
            ));
        }
        Ok(())
    }
}
