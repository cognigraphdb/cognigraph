//! Derivation fixtures.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn stored_reproducible_context(
    state: &AppState,
    cas_root: &Path,
    attestor: &TestGovernancePrincipal,
    candidate: &str,
    binding: &PolicyGovernanceBinding,
    shared: Option<&ArtifactAttestationSet>,
    forge_evidence_chunk: bool,
    prepare_from_raw: bool,
    forge_prepared_output: bool,
) -> PromotionContext {
    stored_reproducible_context_variant(
        state,
        cas_root,
        attestor,
        candidate,
        binding,
        shared,
        forge_evidence_chunk,
        prepare_from_raw,
        forge_prepared_output,
        ReproducibleContextVariant::Supply,
    )
    .await
}
#[derive(Clone, Copy)]
pub(super) enum ReproducibleContextVariant {
    Empty,
    EmptyWithAll,
    Supply,
    SupplyAndDistribute,
    SupplyAndManufacture,
}
impl ReproducibleContextVariant {
    pub(super) fn relation_rules(self) -> Vec<ConstructionRelationRule> {
        let mut rules = Vec::new();
        if matches!(self, Self::SupplyAndDistribute) {
            rules.push(ConstructionRelationRule {
                source: "Meridian".into(),
                relation: "DISTRIBUTES".into(),
                target: "Compound X".into(),
                when_any: vec!["distributes Compound X".into()],
                require_in_sentence: Vec::new(),
            });
        }
        if matches!(
            self,
            Self::Supply | Self::SupplyAndDistribute | Self::SupplyAndManufacture
        ) {
            rules.push(ConstructionRelationRule {
                source: "Meridian".into(),
                relation: "SUPPLIES".into(),
                target: "Compound X".into(),
                when_any: vec!["supplies Compound X".into()],
                require_in_sentence: Vec::new(),
            });
        }
        if matches!(self, Self::SupplyAndManufacture) {
            rules.push(ConstructionRelationRule {
                source: "Meridian".into(),
                relation: "MANUFACTURES".into(),
                target: "Compound X".into(),
                when_any: vec!["manufactures Compound X".into()],
                require_in_sentence: Vec::new(),
            });
            rules.sort_by(|left, right| left.relation.cmp(&right.relation));
        }
        rules
    }

    pub(super) fn facts(self, evidence_chunk_id: &str) -> Vec<VerifiedGraphFact> {
        self.relation_rules()
            .into_iter()
            .map(|rule| VerifiedGraphFact {
                source: rule.source,
                relation: rule.relation,
                target: rule.target,
                evidence_chunk_id: evidence_chunk_id.into(),
            })
            .collect()
    }

    pub(super) fn corpus_text(self) -> &'static str {
        match self {
            Self::Empty => "Meridian supplies Compound X and distributes Compound X.",
            Self::EmptyWithAll => {
                "Meridian supplies Compound X, distributes Compound X, and manufactures Compound X."
            }
            Self::Supply => "Meridian supplies Compound X.",
            Self::SupplyAndDistribute => {
                "Meridian supplies Compound X, distributes Compound X, and manufactures Compound X."
            }
            Self::SupplyAndManufacture => {
                "Meridian supplies Compound X, distributes Compound X, and manufactures Compound X."
            }
        }
    }
}
