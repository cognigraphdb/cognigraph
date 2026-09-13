//! Impact.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedSemanticRepairImpact {
    pub schema_version: u32,
    pub candidate_facts_digest: String,
    pub baseline_facts_digest: String,
    pub added_count: u64,
    pub removed_count: u64,
    pub unchanged_count: u64,
    pub added_facts: Vec<VerifiedGraphFact>,
    pub removed_facts: Vec<VerifiedGraphFact>,
    pub added_facts_digest: String,
    pub removed_facts_digest: String,
    pub candidate_occurrence_projection_digest: String,
    pub impact_digest: String,
}
impl VerifiedSemanticRepairImpact {
    pub(super) fn new(
        candidate: &[VerifiedGraphFact],
        baseline: &[VerifiedGraphFact],
        projection_digest: &str,
    ) -> Result<Self, CogniGraphError> {
        require_impact_fact_bounds(candidate, baseline)?;
        let candidate_map = fact_map(candidate)?;
        let baseline_map = fact_map(baseline)?;
        let added_facts = candidate_map
            .iter()
            .filter(|(key, _)| !baseline_map.contains_key(*key))
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>();
        let removed_facts = baseline_map
            .iter()
            .filter(|(key, _)| !candidate_map.contains_key(*key))
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>();
        let unchanged_count = candidate_map
            .keys()
            .filter(|key| baseline_map.contains_key(*key))
            .count() as u64;
        let mut impact = Self {
            schema_version: 1,
            candidate_facts_digest: canonical_digest(&candidate.to_vec())?,
            baseline_facts_digest: canonical_digest(&baseline.to_vec())?,
            added_count: added_facts.len() as u64,
            removed_count: removed_facts.len() as u64,
            unchanged_count,
            added_facts_digest: canonical_digest(&added_facts)?,
            removed_facts_digest: canonical_digest(&removed_facts)?,
            candidate_occurrence_projection_digest: projection_digest.into(),
            added_facts,
            removed_facts,
            impact_digest: String::new(),
        };
        impact.impact_digest = record_digest(&impact, "impact_digest")?;
        impact.validate(candidate, baseline, projection_digest)?;
        Ok(impact)
    }

    pub(super) fn validate(
        &self,
        candidate: &[VerifiedGraphFact],
        baseline: &[VerifiedGraphFact],
        projection_digest: &str,
    ) -> Result<(), CogniGraphError> {
        let expected = Self::new_unchecked(candidate, baseline, projection_digest)?;
        if self != &expected {
            return Err(conflict("M26 verified repair impact is malformed"));
        }
        Ok(())
    }

    pub(super) fn new_unchecked(
        candidate: &[VerifiedGraphFact],
        baseline: &[VerifiedGraphFact],
        projection_digest: &str,
    ) -> Result<Self, CogniGraphError> {
        require_impact_fact_bounds(candidate, baseline)?;
        let candidate_map = fact_map(candidate)?;
        let baseline_map = fact_map(baseline)?;
        let added_facts = candidate_map
            .iter()
            .filter(|(key, _)| !baseline_map.contains_key(*key))
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>();
        let removed_facts = baseline_map
            .iter()
            .filter(|(key, _)| !candidate_map.contains_key(*key))
            .map(|(_, value)| value.clone())
            .collect::<Vec<_>>();
        let unchanged_count = candidate_map
            .keys()
            .filter(|key| baseline_map.contains_key(*key))
            .count() as u64;
        let mut impact = Self {
            schema_version: 1,
            candidate_facts_digest: canonical_digest(&candidate.to_vec())?,
            baseline_facts_digest: canonical_digest(&baseline.to_vec())?,
            added_count: added_facts.len() as u64,
            removed_count: removed_facts.len() as u64,
            unchanged_count,
            added_facts_digest: canonical_digest(&added_facts)?,
            removed_facts_digest: canonical_digest(&removed_facts)?,
            candidate_occurrence_projection_digest: projection_digest.into(),
            added_facts,
            removed_facts,
            impact_digest: String::new(),
        };
        impact.impact_digest = record_digest(&impact, "impact_digest")?;
        Ok(impact)
    }
}
pub(super) fn require_impact_fact_bounds(
    candidate: &[VerifiedGraphFact],
    baseline: &[VerifiedGraphFact],
) -> Result<(), CogniGraphError> {
    for (role, facts) in [("candidate", candidate), ("baseline", baseline)] {
        if facts.len() > MAX_M26_SEMANTIC_FACTS {
            return Err(capacity(format!(
                "M26 {role} impact input has {} semantic facts, exceeding {MAX_M26_SEMANTIC_FACTS}",
                facts.len()
            )));
        }
    }
    Ok(())
}
