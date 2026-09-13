//! Per-neuron ablation: what do accepted neurons add over the base
//! ontology? Redundant neurons are graduation candidates (success, not
//! waste — the research lifecycle's key insight).

use crate::grounding::{VetoRule, effective_config, effective_vetoes, ground_chunk};
use crate::types::{Chunk, Fact, Neuron, NeuronKind, NeuronStatus, SpaceType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AblationStatus {
    /// Grounded by the base ontology alone — any neuron targeting it is redundant.
    Base,
    /// Grounded only with the neuron set active.
    Recovered,
    /// Not grounded either way.
    Missing,
}

#[derive(Debug, Clone)]
pub struct AblationEntry {
    pub fact: Fact,
    pub status: AblationStatus,
    /// Neuron ids whose triples target this fact.
    pub attributed_to: Vec<String>,
}

/// Ground expected facts with base config vs base+accepted neurons
/// (including accepted blockers' vetoes on the neuron side).
pub fn ablation_report(
    space: &SpaceType,
    neurons: &[Neuron],
    chunks: &[Chunk],
    expected: &[Fact],
) -> Vec<AblationEntry> {
    let with_neurons = effective_config(space, neurons);
    let vetoes = effective_vetoes(neurons);
    let base_facts = ground_all(space, chunks, &[]);
    let full_facts = ground_all(&with_neurons, chunks, &vetoes);
    expected
        .iter()
        .map(|fact| {
            let status = if base_facts.contains(fact) {
                AblationStatus::Base
            } else if full_facts.contains(fact) {
                AblationStatus::Recovered
            } else {
                AblationStatus::Missing
            };
            let attributed_to = neurons
                .iter()
                .filter(|n| {
                    n.source == fact.source
                        && n.relation == fact.relation
                        && n.target == fact.target
                })
                .map(|n| n.id.clone())
                .collect();
            AblationEntry {
                fact: fact.clone(),
                status,
                attributed_to,
            }
        })
        .collect()
}

fn ground_all(config: &SpaceType, chunks: &[Chunk], vetoes: &[VetoRule]) -> Vec<Fact> {
    chunks
        .iter()
        .flat_map(|chunk| ground_chunk(&chunk.id, &chunk.text, config, vetoes))
        .map(|g| g.fact)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockerStatus {
    /// The forbidden fact never grounded even without blockers.
    NeverFired,
    /// Grounded without blockers, suppressed with them — the blocker's delta.
    Suppressed,
    /// Still grounds with blockers active — the veto wording does not cover
    /// every evidencing chunk.
    StillViolated,
}

#[derive(Debug, Clone)]
pub struct BlockerEntry {
    pub fact: Fact,
    pub status: BlockerStatus,
    /// Blocker neuron ids targeting this triple.
    pub attributed_to: Vec<String>,
}

/// The restraint counterpart of `ablation_report`: for each forbidden fact,
/// does it ground without blockers vs with them? `Suppressed` entries are
/// the "prove the delta" artifact — pair with `ablation_report` over the
/// expected facts to show no recall was lost.
pub fn blocker_report(
    space: &SpaceType,
    neurons: &[Neuron],
    chunks: &[Chunk],
    forbidden: &[Fact],
) -> Vec<BlockerEntry> {
    let config = effective_config(space, neurons);
    let vetoes = effective_vetoes(neurons);
    let unblocked = ground_all(&config, chunks, &[]);
    let blocked = ground_all(&config, chunks, &vetoes);
    forbidden
        .iter()
        .map(|fact| {
            let status = match (unblocked.contains(fact), blocked.contains(fact)) {
                (false, _) => BlockerStatus::NeverFired,
                (true, false) => BlockerStatus::Suppressed,
                (true, true) => BlockerStatus::StillViolated,
            };
            let attributed_to = neurons
                .iter()
                .filter(|n| {
                    n.kind == NeuronKind::RelationBlocker
                        && n.source == fact.source
                        && n.relation == fact.relation
                        && n.target == fact.target
                })
                .map(|n| n.id.clone())
                .collect();
            BlockerEntry {
                fact: fact.clone(),
                status,
                attributed_to,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraduationReason {
    /// The base ontology alone grounds the fact — the research's original
    /// redundancy finding (graduation is success, not waste).
    CoveredByBase,
    /// Other accepted neurons ground the fact without this one.
    CoveredByOthers,
    /// The blocker's forbidden fact no longer grounds even without the
    /// veto — the document set or ontology moved on.
    InertBlocker,
}

#[derive(Debug, Clone)]
pub struct GraduationCandidate {
    pub neuron_id: String,
    pub fact: Fact,
    pub reason: GraduationReason,
}

/// B6: leave-one-out graduation analysis over the ACCEPTED set. A
/// relation_hint is a candidate when its target fact still grounds with
/// the neuron removed; a relation_blocker when its forbidden fact fails
/// to ground even with the veto removed. Alias and rank-hint neurons are
/// out of scope (no single groundable fact to attribute). The report only
/// FLAGS — retirement remains a human transition.
pub fn graduation_report(
    space: &SpaceType,
    neurons: &[Neuron],
    chunks: &[Chunk],
) -> Vec<GraduationCandidate> {
    let accepted: Vec<&Neuron> = neurons
        .iter()
        .filter(|n| n.status == NeuronStatus::Accepted)
        .collect();
    let base_facts = ground_all(space, chunks, &[]);

    let mut candidates = Vec::new();
    for neuron in &accepted {
        let fact = Fact {
            source: neuron.source.clone(),
            relation: neuron.relation.clone(),
            target: neuron.target.clone(),
        };
        let others: Vec<Neuron> = accepted
            .iter()
            .filter(|n| n.id != neuron.id)
            .map(|n| (*n).clone())
            .collect();
        let config = effective_config(space, &others);
        let vetoes = effective_vetoes(&others);
        let facts_without = ground_all(&config, chunks, &vetoes);
        match neuron.kind {
            NeuronKind::RelationHint => {
                if facts_without.contains(&fact) {
                    let reason = if base_facts.contains(&fact) {
                        GraduationReason::CoveredByBase
                    } else {
                        GraduationReason::CoveredByOthers
                    };
                    candidates.push(GraduationCandidate {
                        neuron_id: neuron.id.clone(),
                        fact,
                        reason,
                    });
                }
            }
            NeuronKind::RelationBlocker => {
                if !facts_without.contains(&fact) {
                    candidates.push(GraduationCandidate {
                        neuron_id: neuron.id.clone(),
                        fact,
                        reason: GraduationReason::InertBlocker,
                    });
                }
            }
            NeuronKind::Alias | NeuronKind::RelationRankHint => {}
        }
    }
    candidates
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathwayKind {
    BaseRule,
    Neuron,
}

/// A grounding pathway whose triggers no longer fire anywhere in the
/// corpus — the "failed neural path" of the self-healing loop (B7).
/// Distinct from graduation: a graduated neuron's fact is covered
/// elsewhere; a degraded pathway's capability is LOST unless rewired.
#[derive(Debug, Clone)]
pub struct DegradedPathway {
    /// Neuron id, or `rule:<index>` for base ontology rules.
    pub id: String,
    pub kind: PathwayKind,
    pub fact: Fact,
    /// Every trigger of the pathway (all are dead by definition).
    pub dead_triggers: Vec<String>,
}

/// B7 detection: pathways whose triggers no longer affirm in ANY chunk.
/// Checked per source — a base rule and a neuron on the same triple
/// degrade independently (each is its own path to the fact).
pub fn degradation_report(
    space: &SpaceType,
    neurons: &[Neuron],
    chunks: &[Chunk],
) -> Vec<DegradedPathway> {
    let fires = |phrase: &str| {
        chunks
            .iter()
            .any(|chunk| crate::grounding::affirms_phrase(&chunk.text, phrase))
    };
    let mut degraded = Vec::new();
    for (index, rule) in space.relation_rules.iter().enumerate() {
        if !rule.when_any.is_empty() && !rule.when_any.iter().any(|p| fires(p)) {
            degraded.push(DegradedPathway {
                id: format!("rule:{index}"),
                kind: PathwayKind::BaseRule,
                fact: Fact {
                    source: rule.source.clone(),
                    relation: rule.relation.clone(),
                    target: rule.target.clone(),
                },
                dead_triggers: rule.when_any.clone(),
            });
        }
    }
    for neuron in neurons {
        if neuron.status == NeuronStatus::Accepted
            && neuron.kind == NeuronKind::RelationHint
            && !neuron.triggers.is_empty()
            && !neuron.triggers.iter().any(|p| fires(p))
        {
            degraded.push(DegradedPathway {
                id: neuron.id.clone(),
                kind: PathwayKind::Neuron,
                fact: Fact {
                    source: neuron.source.clone(),
                    relation: neuron.relation.clone(),
                    target: neuron.target.clone(),
                },
                dead_triggers: neuron.triggers.clone(),
            });
        }
    }
    degraded
}
