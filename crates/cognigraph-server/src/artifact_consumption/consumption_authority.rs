//! Consumption authority.

use super::*;

impl EvidenceConsumptionAuthority {
    pub fn from_receipts(
        candidate_original: &ArtifactConsumptionReceipt,
        candidate_replay: &ArtifactConsumptionReceipt,
        baseline_original: &ArtifactConsumptionReceipt,
        baseline_replay: &ArtifactConsumptionReceipt,
    ) -> Result<Self, CogniGraphError> {
        if candidate_original.material_digest != candidate_replay.material_digest
            || baseline_original.material_digest != baseline_replay.material_digest
        {
            return Err(validation(
                "each M21 original/replay pair must consume identical verified material",
            ));
        }
        for (candidate, baseline) in [
            (&candidate_original.corpus, &baseline_original.corpus),
            (&candidate_original.oracle, &baseline_original.oracle),
            (&candidate_original.scorer, &baseline_original.scorer),
            (&candidate_original.verifier, &baseline_original.verifier),
        ] {
            if candidate != baseline {
                return Err(validation(
                    "M21 candidate and baseline must consume identical corpus, oracle, scorer, and verifier material",
                ));
            }
        }
        let derivation = match (
            candidate_original.derivation.as_deref(),
            candidate_replay.derivation.as_deref(),
            baseline_original.derivation.as_deref(),
            baseline_replay.derivation.as_deref(),
        ) {
            (
                Some(candidate_original),
                Some(candidate_replay),
                Some(baseline_original),
                Some(baseline_replay),
            ) => Some(Box::new(EvidenceDerivationAuthority::from_receipts(
                candidate_original,
                candidate_replay,
                baseline_original,
                baseline_replay,
            )?)),
            (None, None, None, None) => None,
            _ => {
                return Err(validation(
                    "promotion evidence cannot mix derived and non-derived consumption receipts",
                ));
            }
        };
        let mut authority = Self {
            candidate_original_receipt_digest: candidate_original.receipt_digest.clone(),
            candidate_replay_receipt_digest: candidate_replay.receipt_digest.clone(),
            candidate_material_digest: candidate_original.material_digest.clone(),
            baseline_original_receipt_digest: baseline_original.receipt_digest.clone(),
            baseline_replay_receipt_digest: baseline_replay.receipt_digest.clone(),
            baseline_material_digest: baseline_original.material_digest.clone(),
            derivation,
            authority_digest: String::new(),
        };
        authority.authority_digest = record_digest(&authority, "authority_digest")?;
        authority.validate()?;
        Ok(authority)
    }

    pub fn validate(&self) -> Result<(), CogniGraphError> {
        for (label, digest) in [
            (
                "candidate_original_receipt_digest",
                &self.candidate_original_receipt_digest,
            ),
            (
                "candidate_replay_receipt_digest",
                &self.candidate_replay_receipt_digest,
            ),
            ("candidate_material_digest", &self.candidate_material_digest),
            (
                "baseline_original_receipt_digest",
                &self.baseline_original_receipt_digest,
            ),
            (
                "baseline_replay_receipt_digest",
                &self.baseline_replay_receipt_digest,
            ),
            ("baseline_material_digest", &self.baseline_material_digest),
            ("authority_digest", &self.authority_digest),
        ] {
            validate_digest(label, digest)?;
        }
        if let Some(derivation) = &self.derivation {
            derivation.validate()?;
        }
        if record_digest(self, "authority_digest")? != self.authority_digest {
            return Err(validation("consumption authority digest mismatch"));
        }
        Ok(())
    }
}
