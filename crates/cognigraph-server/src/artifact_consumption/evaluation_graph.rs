//! Evaluation graph.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationGraphArtifact {
    pub schema_version: u32,
    pub space_type: String,
    pub corpus_manifest_digest: String,
    pub candidate_digest: String,
    pub construction_config_digest: String,
    pub facts: Vec<VerifiedGraphFact>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedGraphFact {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub evidence_chunk_id: String,
}
impl EvaluationGraphArtifact {
    pub(crate) fn validate(
        &self,
        context: &PromotionContext,
        corpus_manifest_digest: &str,
    ) -> Result<(), CogniGraphError> {
        if self.schema_version != 1
            || self.space_type != context.target.space_type
            || self.corpus_manifest_digest != corpus_manifest_digest
            || self.candidate_digest != context.candidate.candidate_digest
            || self.construction_config_digest
                != context.effective_configuration.construction_config_digest
            || self.facts.len() > MAX_GRAPH_FACTS
        {
            return Err(validation(
                "evaluation graph artifact does not match its M21 context",
            ));
        }
        validate_digest("graph.corpus_manifest_digest", &self.corpus_manifest_digest)?;
        validate_digest("graph.candidate_digest", &self.candidate_digest)?;
        validate_digest(
            "graph.construction_config_digest",
            &self.construction_config_digest,
        )?;
        let mut previous: Option<(&str, &str, &str, &str)> = None;
        let mut unique = HashSet::new();
        for fact in &self.facts {
            for (label, value) in [
                ("source", &fact.source),
                ("relation", &fact.relation),
                ("target", &fact.target),
                ("evidence_chunk_id", &fact.evidence_chunk_id),
            ] {
                validate_text(&format!("graph.fact.{label}"), value)?;
            }
            let key = (
                fact.source.as_str(),
                fact.relation.as_str(),
                fact.target.as_str(),
                fact.evidence_chunk_id.as_str(),
            );
            if previous.is_some_and(|previous| previous >= key) || !unique.insert(key) {
                return Err(validation(
                    "evaluation graph facts must be sorted and unique",
                ));
            }
            previous = Some(key);
        }
        Ok(())
    }

    pub(crate) fn scoring_facts(&self) -> HashSet<Fact> {
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
