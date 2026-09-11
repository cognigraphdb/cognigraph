//! Corpus.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedChunkCorpusArtifact {
    pub schema_version: u32,
    pub space_type: String,
    pub corpus_revision_id: String,
    pub preprocessing_digest: String,
    pub chunks: Vec<PreparedChunkArtifact>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedChunkArtifact {
    pub id: String,
    pub title: String,
    pub text: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDocumentSetArtifact {
    pub schema_version: u32,
    pub space_type: String,
    pub corpus_revision_id: String,
    pub preparation_plan_digest: String,
    pub documents: Vec<RawDocumentArtifact>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDocumentArtifact {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub byte_length: u64,
    pub blob_digest: String,
    pub content_base64url: String,
}
impl RawDocumentSetArtifact {
    pub(super) fn decode(
        &self,
        context: &PromotionContext,
        plan: &RawCorpusPreparationPlan,
    ) -> Result<DecodedRawDocuments, CogniGraphError> {
        plan.validate()?;
        if self.schema_version != 1
            || self.space_type != context.target.space_type
            || self.corpus_revision_id != context.revisions.corpus.revision_id
            || self.preparation_plan_digest != plan.plan_digest
            || context.effective_configuration.preprocessing_digest != plan.plan_digest
            || self.documents.is_empty()
            || self.documents.len() as u64 > plan.max_documents
        {
            return Err(validation(
                "raw document set does not match its M23 context, preparation plan, or limits",
            ));
        }
        validate_text("raw documents space_type", &self.space_type)?;
        validate_text("raw documents corpus_revision_id", &self.corpus_revision_id)?;
        validate_digest(
            "raw documents preparation_plan_digest",
            &self.preparation_plan_digest,
        )?;

        let mut documents = Vec::with_capacity(self.documents.len());
        let mut previous = None;
        let mut total_raw_bytes = 0_u64;
        for document in &self.documents {
            validate_text("raw document id", &document.id)?;
            validate_optional_text("raw document title", &document.title, MAX_TEXT_BYTES)?;
            validate_digest("raw document blob_digest", &document.blob_digest)?;
            if document.media_type != "text/plain; charset=utf-8"
                || document.byte_length == 0
                || document.byte_length > plan.max_raw_document_bytes
                || previous.is_some_and(|previous| previous >= document.id.as_str())
            {
                return Err(validation(
                    "raw documents must be sorted by unique id and use bounded non-empty text/plain; charset=utf-8 payloads",
                ));
            }
            let bytes = decode_raw_document_bytes(document)?;
            total_raw_bytes = total_raw_bytes
                .checked_add(document.byte_length)
                .ok_or_else(|| validation("raw document byte accounting overflowed"))?;
            if total_raw_bytes > plan.max_total_raw_document_bytes {
                return Err(CogniGraphError::CapacityExceeded(format!(
                    "M23 raw document bytes exceed the pinned {} byte limit",
                    plan.max_total_raw_document_bytes
                )));
            }
            documents.push(RawDocumentBytes {
                id: document.id.clone(),
                title: document.title.clone(),
                bytes,
            });
            previous = Some(document.id.as_str());
        }
        Ok(DecodedRawDocuments { documents })
    }
}
pub(super) fn decode_raw_document_bytes(
    document: &RawDocumentArtifact,
) -> Result<Vec<u8>, CogniGraphError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(&document.content_base64url)
        .map_err(|_| validation("raw document content_base64url is not canonical base64url"))?;
    if URL_SAFE_NO_PAD.encode(&bytes) != document.content_base64url
        || bytes.len() as u64 != document.byte_length
        || digest_bytes(&bytes) != document.blob_digest
    {
        return Err(validation(
            "raw document base64url bytes do not match their declared exact length and SHA-256 digest",
        ));
    }
    Ok(bytes)
}
pub(super) struct DecodedRawDocuments {
    pub(super) documents: Vec<RawDocumentBytes>,
}
impl PreparedChunkCorpusArtifact {
    pub(super) fn validate(
        &self,
        context: &PromotionContext,
        plan: &CorpusGraphDerivationPlan,
    ) -> Result<(), CogniGraphError> {
        if self.schema_version != 1
            || self.space_type != context.target.space_type
            || self.corpus_revision_id != context.revisions.corpus.revision_id
            || self.preprocessing_digest != context.effective_configuration.preprocessing_digest
            || self.chunks.is_empty()
            || self.chunks.len() as u64 > plan.max_chunks
        {
            return Err(validation(
                "prepared chunk corpus does not match its M22 context or limits",
            ));
        }
        validate_text("corpus.space_type", &self.space_type)?;
        validate_text("corpus.corpus_revision_id", &self.corpus_revision_id)?;
        validate_digest("corpus.preprocessing_digest", &self.preprocessing_digest)?;
        let mut previous: Option<&str> = None;
        let mut chunk_keys = HashSet::new();
        for chunk in &self.chunks {
            validate_text("corpus.chunk.id", &chunk.id)?;
            validate_optional_text("corpus.chunk.title", &chunk.title, MAX_TEXT_BYTES)?;
            validate_corpus_text("corpus.chunk.text", &chunk.text, plan.max_chunk_text_bytes)?;
            if previous.is_some_and(|previous| previous >= chunk.id.as_str()) {
                return Err(validation(
                    "prepared corpus chunks must be sorted by unique raw id",
                ));
            }
            let key = cognigraph_construct::ingest::chunk_key(&self.space_type, &chunk.id);
            if !chunk_keys.insert(key) {
                return Err(validation(
                    "prepared corpus chunk ids collide under the construction storage key",
                ));
            }
            previous = Some(&chunk.id);
        }
        Ok(())
    }

    pub(super) fn chunks(&self) -> Vec<Chunk> {
        self.chunks
            .iter()
            .map(|chunk| Chunk {
                id: chunk.id.clone(),
                title: chunk.title.clone(),
                text: chunk.text.clone(),
            })
            .collect()
    }

    /// Validate and expose the exact prepared chunks selected by an M22/M23
    /// promotion context for M26 materialization. The caller must still prove
    /// that the serialized bytes are canonical and match the verified CAS
    /// content address before invoking the grounder.
    pub(crate) fn materialization_chunks(
        &self,
        context: &PromotionContext,
    ) -> Result<Vec<Chunk>, CogniGraphError> {
        let plan = context
            .consumption_plan
            .as_deref()
            .and_then(|plan| plan.derivation.as_deref())
            .ok_or_else(|| {
                validation(
                    "verified materialization requires an M22/M23 derivation-capable context",
                )
            })?;
        self.validate(context, plan)?;
        Ok(self.chunks())
    }
}
