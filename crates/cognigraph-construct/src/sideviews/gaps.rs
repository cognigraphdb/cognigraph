//! Side views as a gap detector for the proposal queue (CG-88).
//!
//! A side view is a generated question/answer pair about one document. When
//! it names two ontology entities that the fact graph does not connect, it
//! points at knowledge the graph may lack. This module turns an explicit
//! selection of side views into *candidate gaps* (`A --REL--> B`) for the
//! existing proposal path; it never writes anything.
//!
//! The detector is deliberately conservative:
//!
//! - entities are found with the same surface matcher grounding uses
//!   ([`crate::grounding::mentions`]), over the question and answer together;
//! - a pair becomes a candidate only when exactly ONE relation rule of the
//!   space type joins the two entities (rules name entity instances), in
//!   exactly one direction. No relation is guessed: several fitting rules or
//!   directions are reported as `ambiguous_relation`, none as
//!   `no_matching_rule`;
//! - a pair already connected by any fact edge in the same `space_id`, in
//!   either direction and by any relation, is not a gap (`endpoints_connected`);
//! - a candidate must be supported by at least `min_support` distinct side
//!   views (`below_min_support` otherwise).
//!
//! See `docs/decisions/decision_sideview_gap_detector.md`.

use crate::ingest::entity_key;
use crate::types::{Fact, SpaceType};
use anyhow::Result;
use cognigraph_core::{Direction, GraphBackend};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The text of one stored side view, as the detector reads it.
#[derive(Debug, Clone)]
pub struct SideViewText {
    pub id: String,
    pub question: String,
    pub answer: String,
}

/// A candidate gap and the side views that support it (sorted ids).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GapCandidate {
    pub fact: Fact,
    pub side_views: Vec<String>,
}

/// A co-mentioned entity pair that did not become a candidate.
///
/// `reason` is one of `no_matching_rule`, `ambiguous_relation`,
/// `endpoints_connected` or `below_min_support`. For the first two, `source`
/// and `target` follow the space type's entity order; for the others they are
/// the candidate's direction. `detail` lists the fitting facts of an
/// ambiguous pair.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GapSkip {
    pub reason: &'static str,
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub side_views: Vec<String>,
}

/// Detector output: candidates ordered by support (descending), then by
/// source, relation and target; skips in detection order.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct GapReport {
    pub scanned: usize,
    pub candidates: Vec<GapCandidate>,
    pub skipped: Vec<GapSkip>,
}

fn arrow(fact: &Fact) -> String {
    format!("{} --{}--> {}", fact.source, fact.relation, fact.target)
}

/// Every fact a relation rule licenses between entities `a` and `b`, in
/// either direction, deduplicated and sorted.
fn fitting_facts(space: &SpaceType, a: usize, b: usize) -> Vec<Fact> {
    let (ea, eb) = (&space.entities[a], &space.entities[b]);
    let mut fits = BTreeSet::new();
    for rule in &space.relation_rules {
        for (source, target) in [(ea, eb), (eb, ea)] {
            if rule.source == source.name && rule.target == target.name {
                fits.insert((
                    source.name.clone(),
                    rule.relation.clone(),
                    target.name.clone(),
                ));
            }
        }
    }
    fits.into_iter()
        .map(|(source, relation, target)| Fact {
            source,
            relation,
            target,
        })
        .collect()
}

/// The pure half of the detector: entity pairs, rule matching and support.
/// No graph lookup and no support threshold.
pub(crate) fn pair_candidates(space: &SpaceType, views: &[SideViewText]) -> GapReport {
    // Unordered pairs keyed by entity index (a < b) so the output does not
    // depend on the order side views mention entities.
    let mut pairs: BTreeMap<(usize, usize), BTreeSet<String>> = BTreeMap::new();
    for view in views {
        let text = format!("{}\n{}", view.question, view.answer);
        let mut seen_names = BTreeSet::new();
        let found: Vec<usize> = crate::grounding::mentions(&text, &space.entities)
            .into_iter()
            .filter(|entity| seen_names.insert(entity.name.clone()))
            .filter_map(|entity| {
                space
                    .entities
                    .iter()
                    .position(|candidate| std::ptr::eq(candidate, entity))
            })
            .collect();
        for (i, &a) in found.iter().enumerate() {
            for &b in &found[i + 1..] {
                pairs
                    .entry((a.min(b), a.max(b)))
                    .or_default()
                    .insert(view.id.clone());
            }
        }
    }

    let mut report = GapReport {
        scanned: views.len(),
        ..GapReport::default()
    };
    for ((a, b), ids) in pairs {
        let side_views: Vec<String> = ids.into_iter().collect();
        let mut fits = fitting_facts(space, a, b);
        let skip = |reason, detail| GapSkip {
            reason,
            source: space.entities[a].name.clone(),
            target: space.entities[b].name.clone(),
            detail,
            side_views: side_views.clone(),
        };
        match fits.len() {
            0 => report.skipped.push(skip("no_matching_rule", None)),
            1 => report.candidates.push(GapCandidate {
                fact: fits.remove(0),
                side_views,
            }),
            _ => {
                let options = fits.iter().map(arrow).collect::<Vec<_>>().join("; ");
                report
                    .skipped
                    .push(skip("ambiguous_relation", Some(options)));
            }
        }
    }
    report.candidates.sort_by(|x, y| {
        y.side_views.len().cmp(&x.side_views.len()).then_with(|| {
            (&x.fact.source, &x.fact.relation, &x.fact.target).cmp(&(
                &y.fact.source,
                &y.fact.relation,
                &y.fact.target,
            ))
        })
    });
    report
}

/// Whether any fact edge of `space_id` joins the two entities, either way.
async fn endpoints_connected(
    backend: &dyn GraphBackend,
    space_id: &str,
    fact: &Fact,
) -> Result<bool> {
    let source = format!("entities/{}", entity_key(&fact.source));
    let target = format!("entities/{}", entity_key(&fact.target));
    for (from, to) in [(&source, &target), (&target, &source)] {
        let edges = backend
            .get_edges("facts", from, Direction::Outbound)
            .await?;
        if edges.iter().any(|edge| {
            edge.get("_to").and_then(Value::as_str) == Some(to.as_str())
                && edge.get("space_id").and_then(Value::as_str) == Some(space_id)
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Detect candidate gaps in `views` for `space` against the fact graph of
/// `space_id`. Read-only: it only lists collections and reads fact edges.
/// A `min_support` of 0 behaves as 1.
pub async fn detect_sideview_gaps(
    backend: &dyn GraphBackend,
    space: &SpaceType,
    space_id: &str,
    views: &[SideViewText],
    min_support: usize,
) -> Result<GapReport> {
    let mut report = pair_candidates(space, views);
    let pending = std::mem::take(&mut report.candidates);
    if pending.is_empty() {
        return Ok(report);
    }
    let has_facts = backend
        .list_collections()
        .await?
        .iter()
        .any(|info| info.name == "facts");
    for candidate in pending {
        let skip = |reason| GapSkip {
            reason,
            source: candidate.fact.source.clone(),
            target: candidate.fact.target.clone(),
            detail: Some(arrow(&candidate.fact)),
            side_views: candidate.side_views.clone(),
        };
        if candidate.side_views.len() < min_support.max(1) {
            report.skipped.push(skip("below_min_support"));
        } else if has_facts && endpoints_connected(backend, space_id, &candidate.fact).await? {
            report.skipped.push(skip("endpoints_connected"));
        } else {
            report.candidates.push(candidate);
        }
    }
    Ok(report)
}

#[cfg(test)]
#[path = "gaps_tests.rs"]
mod tests;
