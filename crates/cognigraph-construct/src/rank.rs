//! Accepted Semantic Neuron hints for the shared retrieval ranker.

use crate::types::{Neuron, NeuronKind, NeuronStatus};
pub use cognigraph_core::graph_ranking::*;
use std::collections::HashMap;
#[cfg(test)]
use std::collections::HashSet;

/// Per-relation score boosts from ACCEPTED `relation_rank_hint` neurons.
/// Additive and order-independent: multiple hints for one relation sum.
/// Proposed/rejected/retired hints contribute nothing — same governance
/// boundary as construction neurons.
pub fn rank_boosts(neurons: &[Neuron]) -> HashMap<String, f64> {
    let mut boosts: HashMap<String, f64> = HashMap::new();
    for neuron in neurons {
        if neuron.kind == NeuronKind::RelationRankHint && neuron.status == NeuronStatus::Accepted {
            *boosts.entry(neuron.relation.to_uppercase()).or_default() += neuron.boost;
        }
    }
    boosts
}

#[cfg(test)]
#[path = "rank_tests.rs"]
mod tests;
