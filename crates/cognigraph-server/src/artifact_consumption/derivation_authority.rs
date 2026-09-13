//! Derivation authority.

use super::*;

impl EvidenceDerivationAuthority {
    pub(super) fn from_receipts(
        candidate_original: &CorpusGraphDerivationReceipt,
        candidate_replay: &CorpusGraphDerivationReceipt,
        baseline_original: &CorpusGraphDerivationReceipt,
        baseline_replay: &CorpusGraphDerivationReceipt,
    ) -> Result<Self, CogniGraphError> {
        if candidate_original.derivation_material_digest
            != candidate_replay.derivation_material_digest
            || baseline_original.derivation_material_digest
                != baseline_replay.derivation_material_digest
            || candidate_original.derivation_plan_digest != baseline_original.derivation_plan_digest
            || candidate_original.corpus_manifest_digest != baseline_original.corpus_manifest_digest
        {
            return Err(validation(
                "M22/M23 replay pairs must reproduce identical derivation material and share one corpus and derivation plan",
            ));
        }
        let preparation = match (
            candidate_original.preparation.as_deref(),
            candidate_replay.preparation.as_deref(),
            baseline_original.preparation.as_deref(),
            baseline_replay.preparation.as_deref(),
        ) {
            (
                Some(candidate_original),
                Some(candidate_replay),
                Some(baseline_original),
                Some(baseline_replay),
            ) => Some(Box::new(EvidencePreparationAuthority::from_receipts(
                candidate_original,
                candidate_replay,
                baseline_original,
                baseline_replay,
            )?)),
            (None, None, None, None) => None,
            _ => {
                return Err(validation(
                    "promotion evidence cannot mix prepared and preparation-backed derivation receipts",
                ));
            }
        };
        let mut authority = Self {
            candidate_original_derivation_digest: candidate_original
                .derivation_material_digest
                .clone(),
            candidate_replay_derivation_digest: candidate_replay.derivation_material_digest.clone(),
            candidate_graph_digest: candidate_original.derived_graph_blob_digest.clone(),
            baseline_original_derivation_digest: baseline_original
                .derivation_material_digest
                .clone(),
            baseline_replay_derivation_digest: baseline_replay.derivation_material_digest.clone(),
            baseline_graph_digest: baseline_original.derived_graph_blob_digest.clone(),
            derivation_plan_digest: candidate_original.derivation_plan_digest.clone(),
            preparation,
            authority_digest: String::new(),
        };
        authority.authority_digest = record_digest(&authority, "authority_digest")?;
        authority.validate()?;
        Ok(authority)
    }

    pub fn validate(&self) -> Result<(), CogniGraphError> {
        for (label, digest) in [
            (
                "candidate_original_derivation_digest",
                &self.candidate_original_derivation_digest,
            ),
            (
                "candidate_replay_derivation_digest",
                &self.candidate_replay_derivation_digest,
            ),
            ("candidate_graph_digest", &self.candidate_graph_digest),
            (
                "baseline_original_derivation_digest",
                &self.baseline_original_derivation_digest,
            ),
            (
                "baseline_replay_derivation_digest",
                &self.baseline_replay_derivation_digest,
            ),
            ("baseline_graph_digest", &self.baseline_graph_digest),
            ("derivation_plan_digest", &self.derivation_plan_digest),
            ("authority_digest", &self.authority_digest),
        ] {
            validate_digest(label, digest)?;
        }
        if let Some(preparation) = &self.preparation {
            preparation.validate()?;
        }
        if self.candidate_original_derivation_digest != self.candidate_replay_derivation_digest
            || self.baseline_original_derivation_digest != self.baseline_replay_derivation_digest
            || record_digest(self, "authority_digest")? != self.authority_digest
        {
            return Err(validation("derivation authority digest or replay mismatch"));
        }
        Ok(())
    }
}
