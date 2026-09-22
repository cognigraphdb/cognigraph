//! The measurement half of the control loop: recall (expected facts were
//! constructed, with evidence) and restraint (forbidden facts were NOT).
//!
//! This scores construction coverage — facts present in the graph — which
//! is deliberately narrower than the research repo's retrieval-path metric
//! (their fixed-query-seeder asterisk applies there; here the asterisk is
//! "construction only").

use std::collections::HashSet;

use cognigraph_core::{Direction, GraphBackend, Result};
use serde_json::Value;

use crate::ingest::entity_key;
use crate::types::{EvalSpec, Fact};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalOutcome {
    pub space_id: String,
    pub expected_total: usize,
    pub expected_found: usize,
    pub forbidden_total: usize,
    pub forbidden_triggered: usize,
    pub missing: Vec<Fact>,
    pub violations: Vec<Fact>,
}

impl EvalOutcome {
    pub fn recall_ok(&self) -> bool {
        self.expected_found == self.expected_total
    }
    pub fn restraint_ok(&self) -> bool {
        self.forbidden_triggered == 0
    }
}

pub async fn evaluate(
    backend: &dyn GraphBackend,
    space_id: &str,
    spec: &EvalSpec,
) -> Result<EvalOutcome> {
    let (expected, forbidden) = distinct_eval_facts(spec);

    // Keep the two observed sets separate. The legacy backend evaluator reads
    // a mutable graph one fact at a time, so a fact present in both categories
    // can legitimately have different observations if the graph changes
    // between the recall and restraint passes.
    let mut expected_present = HashSet::new();
    for fact in &expected {
        if fact_exists(backend, space_id, fact).await? {
            expected_present.insert(fact.clone());
        }
    }
    let mut forbidden_present = HashSet::new();
    for fact in &forbidden {
        if fact_exists(backend, space_id, fact).await? {
            forbidden_present.insert(fact.clone());
        }
    }

    Ok(score_distinct_facts(
        space_id,
        expected,
        forbidden,
        &expected_present,
        &forbidden_present,
    ))
}

/// Score an immutable, already-admitted set of facts without consulting a
/// [`GraphBackend`].
///
/// The caller is responsible for scoping the supplied facts to `space_id` and
/// admitting only facts with the required evidence provenance. Expected and
/// forbidden facts are parsed and deduplicated exactly as in [`evaluate`], and
/// result lists retain their first occurrence order from the `EvalSpec`.
pub fn evaluate_facts(space_id: &str, spec: &EvalSpec, facts: &HashSet<Fact>) -> EvalOutcome {
    let (expected, forbidden) = distinct_eval_facts(spec);
    score_distinct_facts(space_id, expected, forbidden, facts, facts)
}

fn distinct_eval_facts(spec: &EvalSpec) -> (Vec<Fact>, Vec<Fact>) {
    let mut expected: Vec<Fact> = Vec::new();
    let mut forbidden: Vec<Fact> = Vec::new();
    for question in &spec.questions {
        expected.extend(
            question
                .expected_facts
                .iter()
                .filter_map(|fact| Fact::parse(fact)),
        );
        forbidden.extend(
            question
                .forbidden_facts
                .iter()
                .filter_map(|fact| Fact::parse(fact)),
        );
    }

    // Distinct facts (D4, decision_grounding_gates.md): a fact listed
    // under several questions is ONE fact for recall/restraint totals.
    // (Vec::dedup only removed adjacent duplicates, silently making the
    // totals per-mention — the hostile kit's "14/18" was 9/13 distinct.)
    let mut seen = HashSet::new();
    expected.retain(|fact| seen.insert(fact.clone()));
    let mut seen = HashSet::new();
    forbidden.retain(|fact| seen.insert(fact.clone()));
    (expected, forbidden)
}

fn score_distinct_facts(
    space_id: &str,
    expected: Vec<Fact>,
    forbidden: Vec<Fact>,
    expected_present: &HashSet<Fact>,
    forbidden_present: &HashSet<Fact>,
) -> EvalOutcome {
    let missing = expected
        .iter()
        .filter(|fact| !expected_present.contains(*fact))
        .cloned()
        .collect::<Vec<_>>();
    let violations = forbidden
        .iter()
        .filter(|fact| forbidden_present.contains(*fact))
        .cloned()
        .collect::<Vec<_>>();
    EvalOutcome {
        space_id: space_id.to_string(),
        expected_total: expected.len(),
        expected_found: expected.len() - missing.len(),
        forbidden_total: forbidden.len(),
        forbidden_triggered: violations.len(),
        missing,
        violations,
    }
}

async fn fact_exists(backend: &dyn GraphBackend, space_id: &str, fact: &Fact) -> Result<bool> {
    let source = format!("entities/{}", entity_key(&fact.source));
    let target = format!("entities/{}", entity_key(&fact.target));
    let edges = backend
        .get_edges("facts", &source, Direction::Outbound)
        .await?;
    Ok(edges.iter().any(|edge| {
        edge.get("_to").and_then(Value::as_str) == Some(target.as_str())
            && edge.get("relation_type").and_then(Value::as_str) == Some(fact.relation.as_str())
            && edge.get("space_id").and_then(Value::as_str) == Some(space_id)
            && edge.get("evidence_chunk_id").is_some()
    }))
}

#[cfg(test)]
#[path = "eval_tests.rs"]
mod tests;
