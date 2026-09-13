//! Reproducible graph.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReproducibleEvaluationGraphArtifact {
    pub schema_version: u32,
    pub space_type: String,
    pub graph_revision_id: String,
    pub corpus_manifest_digest: String,
    pub corpus_semantic_digest: String,
    pub candidate_digest: String,
    pub construction_config_digest: String,
    pub derivation_plan_digest: String,
    pub facts_digest: String,
    pub facts: Vec<VerifiedGraphFact>,
}
impl ReproducibleEvaluationGraphArtifact {
    pub(super) fn validate_claim(
        &self,
        context: &PromotionContext,
        corpus_manifest_digest: &str,
        corpus_semantic_digest: &str,
        derivation_plan_digest: &str,
    ) -> Result<(), CogniGraphError> {
        if self.schema_version != 1
            || self.space_type != context.target.space_type
            || self.graph_revision_id != context.revisions.graph.revision_id
            || self.corpus_manifest_digest != corpus_manifest_digest
            || self.corpus_semantic_digest != corpus_semantic_digest
            || self.candidate_digest != context.candidate.candidate_digest
            || self.construction_config_digest
                != context.effective_configuration.construction_config_digest
            || self.derivation_plan_digest != derivation_plan_digest
            || self.facts.len() > MAX_GRAPH_FACTS
            || self.facts_digest != canonical_digest(&self.facts)?
        {
            return Err(validation(
                "reproducible evaluation graph does not match its M22 context, corpus, plan, or fact digest",
            ));
        }
        for (label, digest) in [
            ("graph.corpus_manifest_digest", &self.corpus_manifest_digest),
            ("graph.corpus_semantic_digest", &self.corpus_semantic_digest),
            ("graph.candidate_digest", &self.candidate_digest),
            (
                "graph.construction_config_digest",
                &self.construction_config_digest,
            ),
            ("graph.derivation_plan_digest", &self.derivation_plan_digest),
            ("graph.facts_digest", &self.facts_digest),
        ] {
            validate_digest(label, digest)?;
        }
        validate_graph_facts(&self.facts)
    }

    pub(super) fn scoring_facts(&self) -> HashSet<Fact> {
        self.facts
            .iter()
            .map(|fact| Fact {
                source: fact.source.clone(),
                relation: fact.relation.clone(),
                target: fact.target.clone(),
            })
            .collect()
    }
}
