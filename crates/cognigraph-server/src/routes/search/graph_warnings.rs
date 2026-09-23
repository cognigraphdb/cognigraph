//! Response warnings for graph-augmented search (CG-89).
//!
//! Accepted `relation_rank_hint` neurons reweight the graph-facts ranking, but
//! neuron-built facts live in the managed `facts` edge collection. When a
//! request traverses any other collection, those hints cannot affect the trace
//! and the caller is told so instead of silently getting an unboosted ranking.

use cognigraph_construct::{Neuron, NeuronKind, NeuronStatus};

/// Warning code: accepted rank hints exist but the traversed collection is
/// not `facts`, so they cannot reweight anything in this response.
pub(super) const INERT_RANK_HINTS: &str = "inert_rank_hints";

/// Edge collection written by governed construction; the only one where
/// neuron-built facts, and therefore rank-hint boosts, can appear.
const FACTS_COLLECTION: &str = "facts";

/// `Some(warning)` when at least one ACCEPTED `relation_rank_hint` neuron
/// exists and `edge_collection` is not exactly `facts`. Collection names are
/// exact identities: no trimming, no case folding.
pub(super) fn inert_rank_hint_warning(
    neurons: &[Neuron],
    edge_collection: &str,
) -> Option<serde_json::Value> {
    if edge_collection == FACTS_COLLECTION {
        return None;
    }
    let accepted = neurons
        .iter()
        .filter(|n| n.kind == NeuronKind::RelationRankHint && n.status == NeuronStatus::Accepted)
        .count();
    if accepted == 0 {
        return None;
    }
    Some(serde_json::json!({
        "code": INERT_RANK_HINTS,
        "message": format!(
            "{accepted} accepted relation_rank_hint neuron(s) cannot reweight this trace: \
             neuron-built facts live in the `{FACTS_COLLECTION}` edge collection and this \
             request traverses `{edge_collection}`. Pass edge_collection: \"{FACTS_COLLECTION}\" \
             to rank construct-built facts."
        ),
        "accepted_rank_hints": accepted,
        "edge_collection": edge_collection,
    }))
}

#[cfg(test)]
#[path = "graph_warnings_tests.rs"]
mod tests;
