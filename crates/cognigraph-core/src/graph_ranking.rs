//! Generic retrieval-trace ranking; optional relation boosts are supplied by callers.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use serde_json::Value;

/// Relations that carry structure but little meaning; capped during
/// selection so they cannot crowd out semantic facts.
pub const LOW_SIGNAL_RELATIONS: &[&str] = &["CO_OCCURS_WITH", "HAS_CHUNK", "MENTIONS"];

pub fn is_low_signal(relation: &str) -> bool {
    LOW_SIGNAL_RELATIONS.contains(&relation.to_uppercase().as_str())
}

/// A directed graph fact with its evidence chunk, as ranked for a
/// retrieval trace.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GraphEdge {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub evidence_chunk_id: String,
}

impl GraphEdge {
    /// Build from a backend edge document (`_from`/`_to`/`relation_type`,
    /// as written by `ingest_chunks`). Returns None for malformed edges.
    pub fn from_value(edge: &Value) -> Option<Self> {
        Some(Self {
            source: edge.get("_from")?.as_str()?.to_string(),
            relation: edge.get("relation_type")?.as_str()?.to_string(),
            target: edge.get("_to")?.as_str()?.to_string(),
            evidence_chunk_id: edge
                .get("evidence_chunk_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        })
    }
}

/// The research's static score — configured relations beat MENTIONS beat
/// other low-signal — plus any accepted rank-hint boost for the relation.
pub fn relation_score(relation: &str, boosts: &HashMap<String, f64>) -> f64 {
    let upper = relation.to_uppercase();
    let base = if !is_low_signal(&upper) {
        100.0
    } else if upper == "MENTIONS" {
        20.0
    } else {
        10.0
    };
    base + boosts.get(&upper).copied().unwrap_or(0.0)
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeSelectOpts {
    pub limit: usize,
    pub low_signal_limit: usize,
    pub low_signal_per_entity: usize,
}

impl Default for EdgeSelectOpts {
    fn default() -> Self {
        Self {
            limit: 12,
            low_signal_limit: 4,
            low_signal_per_entity: 2,
        }
    }
}

/// Return a compact, deterministic edge set for a retrieval trace:
/// rank by (relation score, seed adjacency, evidence presence, then
/// lexicographic), dedupe, and cap low-signal relations globally and
/// per entity. Faithful to `select_graph_edges` in the research repo,
/// with `boosts` as the `relation_rank_hint` parameterization.
pub fn select_graph_edges(
    edges: &[GraphEdge],
    seeds: &HashSet<String>,
    boosts: &HashMap<String, f64>,
    opts: &EdgeSelectOpts,
) -> Vec<GraphEdge> {
    // Dedupe exact duplicates, preserving input order (stable sort below
    // keeps that order for fully tied rank keys).
    let mut seen: HashSet<&GraphEdge> = HashSet::new();
    let mut ranked: Vec<&GraphEdge> = edges.iter().filter(|e| seen.insert(e)).collect();
    ranked.sort_by(|a, b| rank_cmp(a, b, seeds, boosts));

    let mut selected: Vec<GraphEdge> = Vec::new();
    let mut selected_keys: HashSet<(String, String, String, Option<String>)> = HashSet::new();
    let mut low_signal_count = 0usize;
    let mut low_signal_entity_counts: HashMap<&str, usize> = HashMap::new();

    for edge in ranked {
        if selected.len() >= opts.limit {
            break;
        }
        // Low-signal edges dedupe per evidence chunk; semantic facts dedupe
        // by triple regardless of which chunk evidenced them.
        let key = (
            edge.source.clone(),
            edge.relation.clone(),
            edge.target.clone(),
            is_low_signal(&edge.relation).then(|| edge.evidence_chunk_id.clone()),
        );
        if selected_keys.contains(&key) {
            continue;
        }
        if is_low_signal(&edge.relation) {
            if low_signal_count >= opts.low_signal_limit {
                continue;
            }
            let over_entity_cap = [&edge.source, &edge.target].iter().any(|entity| {
                low_signal_entity_counts
                    .get(entity.as_str())
                    .copied()
                    .unwrap_or(0)
                    >= opts.low_signal_per_entity
            });
            if over_entity_cap {
                continue;
            }
            low_signal_count += 1;
            for entity in [edge.source.as_str(), edge.target.as_str()] {
                *low_signal_entity_counts.entry(entity).or_default() += 1;
            }
        }
        selected.push(edge.clone());
        selected_keys.insert(key);
    }
    selected
}

fn rank_cmp(
    a: &GraphEdge,
    b: &GraphEdge,
    seeds: &HashSet<String>,
    boosts: &HashMap<String, f64>,
) -> Ordering {
    let score_a = relation_score(&a.relation, boosts);
    let score_b = relation_score(&b.relation, boosts);
    score_b
        .partial_cmp(&score_a)
        .unwrap_or(Ordering::Equal)
        .then_with(|| seed_score(b, seeds).cmp(&seed_score(a, seeds)))
        .then_with(|| evidence_score(b).cmp(&evidence_score(a)))
        .then_with(|| a.source.to_lowercase().cmp(&b.source.to_lowercase()))
        .then_with(|| a.relation.to_lowercase().cmp(&b.relation.to_lowercase()))
        .then_with(|| a.target.to_lowercase().cmp(&b.target.to_lowercase()))
}

fn seed_score(edge: &GraphEdge, seeds: &HashSet<String>) -> usize {
    usize::from(seeds.contains(&edge.source)) + usize::from(seeds.contains(&edge.target))
}

fn evidence_score(edge: &GraphEdge) -> usize {
    usize::from(!edge.evidence_chunk_id.is_empty())
}
