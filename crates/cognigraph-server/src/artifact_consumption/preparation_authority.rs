//! Preparation authority.

use super::*;

impl EvidencePreparationAuthority {
    pub(super) fn from_receipts(
        candidate_original: &RawCorpusPreparationReceipt,
        candidate_replay: &RawCorpusPreparationReceipt,
        baseline_original: &RawCorpusPreparationReceipt,
        baseline_replay: &RawCorpusPreparationReceipt,
    ) -> Result<Self, CogniGraphError> {
        if candidate_original.preparation_material_digest
            != candidate_replay.preparation_material_digest
            || candidate_original.preparation_material_digest
                != baseline_original.preparation_material_digest
            || candidate_original.preparation_material_digest
                != baseline_replay.preparation_material_digest
            || candidate_original.raw_document_set_digest
                != baseline_original.raw_document_set_digest
            || candidate_original.prepared_corpus_blob_digest
                != baseline_original.prepared_corpus_blob_digest
            || candidate_original.preparation_plan_digest
                != baseline_original.preparation_plan_digest
        {
            return Err(validation(
                "M23 candidate and baseline replay pairs must reproduce one identical raw-document preparation",
            ));
        }
        let mut authority = Self {
            candidate_original_preparation_digest: candidate_original
                .preparation_material_digest
                .clone(),
            candidate_replay_preparation_digest: candidate_replay
                .preparation_material_digest
                .clone(),
            baseline_original_preparation_digest: baseline_original
                .preparation_material_digest
                .clone(),
            baseline_replay_preparation_digest: baseline_replay.preparation_material_digest.clone(),
            raw_document_set_digest: candidate_original.raw_document_set_digest.clone(),
            prepared_corpus_digest: candidate_original.prepared_corpus_blob_digest.clone(),
            preparation_plan_digest: candidate_original.preparation_plan_digest.clone(),
            authority_digest: String::new(),
        };
        authority.authority_digest = record_digest(&authority, "authority_digest")?;
        authority.validate()?;
        Ok(authority)
    }

    pub fn validate(&self) -> Result<(), CogniGraphError> {
        for (label, digest) in [
            (
                "candidate_original_preparation_digest",
                &self.candidate_original_preparation_digest,
            ),
            (
                "candidate_replay_preparation_digest",
                &self.candidate_replay_preparation_digest,
            ),
            (
                "baseline_original_preparation_digest",
                &self.baseline_original_preparation_digest,
            ),
            (
                "baseline_replay_preparation_digest",
                &self.baseline_replay_preparation_digest,
            ),
            ("raw_document_set_digest", &self.raw_document_set_digest),
            ("prepared_corpus_digest", &self.prepared_corpus_digest),
            ("preparation_plan_digest", &self.preparation_plan_digest),
            ("authority_digest", &self.authority_digest),
        ] {
            validate_digest(label, digest)?;
        }
        if self.candidate_original_preparation_digest != self.candidate_replay_preparation_digest
            || self.candidate_original_preparation_digest
                != self.baseline_original_preparation_digest
            || self.candidate_original_preparation_digest != self.baseline_replay_preparation_digest
            || record_digest(self, "authority_digest")? != self.authority_digest
        {
            return Err(validation(
                "preparation authority digest or four-run replay mismatch",
            ));
        }
        Ok(())
    }
}
