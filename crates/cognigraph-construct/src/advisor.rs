//! The gate advisor: a deterministic corpus-statistics pass that tells an
//! ontology author WHERE to spend the sentence-gate precision budget
//! (decision_grounding_gates.md: gates are opt-in per rule because global
//! gating was rejected by measurement — it collapsed an honest kit's
//! recall). For every rule the advisor grounds the corpus under each
//! candidate gate — none, source, target, both — using `ground_chunk`
//! itself, so the numbers come from the exact production gate semantics,
//! never a reimplementation. No LLM, no writes; the author decides.
//!
//! Reading the numbers: a chunk that grounds ungated but not under a gate
//! is a grounding whose licensing sentence does not name the endpoint —
//! POTENTIAL wrong-subject leakage (only the author can say for sure,
//! which is why the samples carry the actual sentence). A gate is only
//! suggested when it blocks at least one such chunk AND the fact still
//! grounds somewhere, so following a suggestion never silences a fact on
//! the analyzed corpus.

use serde::Serialize;
use std::collections::HashMap;

use crate::grounding::{
    effective_config, effective_vetoes, ground_chunk, relation_semantics_signals, sentence_bounds,
};
use crate::types::{Chunk, EndpointRef, Neuron, SpaceType};

/// How one candidate gate would behave on the corpus.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct GateOutcome {
    /// Chunks where the fact still grounds with this gate on.
    pub grounds: usize,
    /// Chunks that ground ungated but NOT with this gate — the potential
    /// off-subject groundings the gate would refuse.
    pub blocked: usize,
}

/// A grounding whose licensing sentence does not appear to assert the rule's
/// relation, with the signals that fired and the sentence itself.
#[derive(Debug, Clone, Serialize)]
pub struct SemanticsSample {
    pub chunk_id: String,
    pub sentence: String,
    /// Which detectors fired — the author reads these against the sentence.
    pub signals: Vec<String>,
}

/// A blocked grounding, shown as evidence for the suggestion.
#[derive(Debug, Clone, Serialize)]
pub struct BlockedSample {
    pub chunk_id: String,
    /// The sentence containing the licensing trigger occurrence — the
    /// sentence the gate judged to be off-subject.
    pub sentence: String,
}

/// Per-rule advice: the measured outcomes, a safe suggestion where one
/// exists, and a review flag where only the author can decide.
#[derive(Debug, Clone, Serialize)]
pub struct GateAdvice {
    pub fact: String,
    /// The rule's gate as currently authored.
    pub current_gate: Vec<EndpointRef>,
    /// Chunks grounding this fact with no gate at all.
    pub grounds_ungated: usize,
    pub source_gate: GateOutcome,
    pub target_gate: GateOutcome,
    pub both_gate: GateOutcome,
    /// A SAFE suggestion: this gate blocks off-subject groundings while
    /// the fact keeps grounding elsewhere — applying it costs no recall
    /// on the analyzed corpus. None = nothing safe to apply.
    pub suggestion: Option<Vec<EndpointRef>>,
    /// Endpoints that appear in NO licensing sentence at all while the
    /// rule grounds — the strongest signal in the report, but ambiguous:
    /// either the corpus never attributes this fact to that endpoint
    /// (the hostile cross-company leakage signature — gate it and the
    /// rule fires only on genuinely on-subject evidence) or the evidence
    /// is legitimately cross-sentence (a gate would silence a good
    /// fact). The samples carry the actual sentences; the author
    /// decides. This is exactly how the hostile corpus's authored gates
    /// look to the advisor.
    pub never_in_sentence: Vec<EndpointRef>,
    pub reason: String,
    /// Sentences the flagged/suggested gate would refuse (capped) — the
    /// author's review material.
    pub samples: Vec<BlockedSample>,
    /// Groundings whose licensing sentence does not appear to ASSERT this
    /// relation — a second, independent detector from the gate analysis above.
    /// The gate checks whether the endpoints are PRESENT; this checks whether
    /// the sentence says what the relation claims. Measured on the 100-label
    /// pilot: leaving the flagged groundings out lifts precision 85.0% → 93.9%
    /// while catching 77% of all errors (the endpoint-presence signal alone
    /// separated nothing). Advisory only — nothing is dropped.
    pub semantics_suspect: usize,
    /// Capped review material for `semantics_suspect`.
    pub semantics_samples: Vec<SemanticsSample>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GateAdvisorReport {
    pub rules: Vec<GateAdvice>,
}

const SAMPLE_CAP: usize = 3;

fn fact_line(fact: &crate::types::Fact) -> String {
    format!("{} --{}--> {}", fact.source, fact.relation, fact.target)
}

/// Analyze the EFFECTIVE config (base rules + accepted hints, exactly what
/// grounding runs) against a corpus and advise per-rule sentence gates.
pub fn advise_gates(space: &SpaceType, neurons: &[Neuron], chunks: &[Chunk]) -> GateAdvisorReport {
    let config = effective_config(space, neurons);
    let vetoes = effective_vetoes(neurons);

    // The four candidate configs differ only in every rule's gate.
    let with_gate = |gate: &[EndpointRef]| -> SpaceType {
        let mut variant = config.clone();
        for rule in &mut variant.relation_rules {
            rule.require_in_sentence = gate.to_vec();
        }
        variant
    };
    let ungated = with_gate(&[]);
    let gated_source = with_gate(&[EndpointRef::Source]);
    let gated_target = with_gate(&[EndpointRef::Target]);
    let gated_both = with_gate(&[EndpointRef::Source, EndpointRef::Target]);

    // Per fact: which chunks ground under each variant, plus the licensing
    // sentence from the ungated pass (the sample material).
    #[derive(Default)]
    struct PerFact {
        ungated: Vec<(String, String)>, // (chunk_id, sentence)
        source: usize,
        target: usize,
        both: usize,
        source_blocked_ids: Vec<usize>, // indices into `ungated`
        target_blocked_ids: Vec<usize>,
        both_blocked_ids: Vec<usize>,
    }
    let mut per_fact: HashMap<String, PerFact> = HashMap::new();

    for chunk in chunks {
        let grounded = ground_chunk(&chunk.id, &chunk.text, &ungated, &vetoes);
        let with = |variant: &SpaceType| -> Vec<String> {
            ground_chunk(&chunk.id, &chunk.text, variant, &vetoes)
                .into_iter()
                .map(|g| fact_line(&g.fact))
                .collect()
        };
        let source_facts = with(&gated_source);
        let target_facts = with(&gated_target);
        let both_facts = with(&gated_both);

        for g in grounded {
            let fact = fact_line(&g.fact);
            let (start, end) = sentence_bounds(&chunk.text, g.trigger_span.0);
            let entry = per_fact.entry(fact.clone()).or_default();
            let index = entry.ungated.len();
            entry
                .ungated
                .push((chunk.id.clone(), chunk.text[start..end].trim().to_string()));
            for (facts, grounds, blocked) in [
                (
                    &source_facts,
                    &mut entry.source,
                    &mut entry.source_blocked_ids,
                ),
                (
                    &target_facts,
                    &mut entry.target,
                    &mut entry.target_blocked_ids,
                ),
                (&both_facts, &mut entry.both, &mut entry.both_blocked_ids),
            ] {
                if facts.contains(&fact) {
                    *grounds += 1;
                } else {
                    blocked.push(index);
                }
            }
        }
    }

    let mut rules: Vec<GateAdvice> = config
        .relation_rules
        .iter()
        .map(|rule| {
            let fact = format!("{} --{}--> {}", rule.source, rule.relation, rule.target);
            let stats = per_fact.remove(&fact).unwrap_or_default();
            let total = stats.ungated.len();
            let outcome = |grounds: usize| GateOutcome {
                grounds,
                blocked: total - grounds,
            };
            let source_gate = outcome(stats.source);
            let target_gate = outcome(stats.target);
            let both_gate = outcome(stats.both);

            // Candidates: block something AND keep the fact grounding.
            // Deterministic preference: most blocked; then most survivors;
            // then the cheaper gate (fewer endpoints, source before target).
            let candidates: Vec<(Vec<EndpointRef>, GateOutcome, &[usize])> = [
                (
                    vec![EndpointRef::Source],
                    source_gate,
                    stats.source_blocked_ids.as_slice(),
                ),
                (
                    vec![EndpointRef::Target],
                    target_gate,
                    stats.target_blocked_ids.as_slice(),
                ),
                (
                    vec![EndpointRef::Source, EndpointRef::Target],
                    both_gate,
                    stats.both_blocked_ids.as_slice(),
                ),
            ]
            .into_iter()
            .filter(|(_, o, _)| o.blocked > 0 && o.grounds > 0)
            .collect();
            let best = candidates.into_iter().max_by(|a, b| {
                (a.1.blocked, a.1.grounds, std::cmp::Reverse(a.0.len())).cmp(&(
                    b.1.blocked,
                    b.1.grounds,
                    std::cmp::Reverse(b.0.len()),
                ))
            });

            // The strongest signal: an endpoint in NO licensing sentence
            // while the rule grounds — the hostile fixture's authored
            // gates all have this shape (the gated endpoint's in-sentence
            // count is zero), so it flags for review, never auto-suggests.
            let mut never_in_sentence = Vec::new();
            if total > 0 {
                if source_gate.grounds == 0 {
                    never_in_sentence.push(EndpointRef::Source);
                }
                if target_gate.grounds == 0 {
                    never_in_sentence.push(EndpointRef::Target);
                }
            }

            let take_samples = |blocked_ids: &[usize]| -> Vec<BlockedSample> {
                blocked_ids
                    .iter()
                    .take(SAMPLE_CAP)
                    .map(|&i| BlockedSample {
                        chunk_id: stats.ungated[i].0.clone(),
                        sentence: stats.ungated[i].1.clone(),
                    })
                    .collect()
            };
            let (suggestion, reason, samples) = match best {
                Some((gate, o, blocked_ids)) => {
                    let reason = format!(
                        "sentence gate on {} keeps {} grounding chunk(s) and refuses {} whose \
                         licensing sentence lacks the endpoint",
                        describe(&gate),
                        o.grounds,
                        o.blocked
                    );
                    let samples = take_samples(blocked_ids);
                    (Some(gate), reason, samples)
                }
                None if total == 0 => (
                    None,
                    "no trigger occurrence grounds anywhere — a dead rule is degradation-report \
                     territory, not a gating problem"
                        .to_string(),
                    Vec::new(),
                ),
                None if !never_in_sentence.is_empty() => {
                    let ids = match never_in_sentence[0] {
                        EndpointRef::Source => stats.source_blocked_ids.as_slice(),
                        EndpointRef::Target => stats.target_blocked_ids.as_slice(),
                    };
                    (
                        None,
                        format!(
                            "REVIEW: {} appears in NO licensing sentence across {} grounding \
                             chunk(s) — either the corpus never attributes this fact to it \
                             (cross-company leakage: gate it and the rule fires only on \
                             on-subject evidence) or the evidence is cross-sentence (a gate \
                             would silence the fact; consider {{source}}/{{target}} template \
                             triggers). Read the samples and decide.",
                            describe(&never_in_sentence),
                            total
                        ),
                        take_samples(ids),
                    )
                }
                None if both_gate.blocked == 0 => (
                    None,
                    "every grounding sentence already names both endpoints — a gate would \
                     change nothing"
                        .to_string(),
                    Vec::new(),
                ),
                // Unreachable in practice: with no candidate, no zero
                // endpoint, and per-endpoint blocked == 0, the both-gate
                // cannot block either — kept for match exhaustiveness.
                None => (None, "no gate has any effect".to_string(), Vec::new()),
            };

            // The second, independent detector: does each licensing sentence
            // actually ASSERT this relation? Runs over the same ungated
            // groundings the gate analysis used, so both views describe the
            // exact same evidence.
            let mut semantics_suspect = 0;
            let mut semantics_samples = Vec::new();
            for (chunk_id, sentence) in &stats.ungated {
                let signals = relation_semantics_signals(
                    &rule.source,
                    &rule.relation,
                    &rule.target,
                    &config.entities,
                    sentence,
                );
                if signals.is_empty() {
                    continue;
                }
                semantics_suspect += 1;
                if semantics_samples.len() < SAMPLE_CAP {
                    semantics_samples.push(SemanticsSample {
                        chunk_id: chunk_id.clone(),
                        sentence: sentence.clone(),
                        signals,
                    });
                }
            }

            GateAdvice {
                fact,
                current_gate: rule.require_in_sentence.clone(),
                grounds_ungated: total,
                source_gate,
                target_gate,
                both_gate,
                suggestion,
                never_in_sentence,
                reason,
                samples,
                semantics_suspect,
                semantics_samples,
            }
        })
        .collect();

    // Safe suggestions first (biggest refusal counts on top), then
    // review flags, then the rest — the author reads a priority list,
    // not an inventory.
    rules.sort_by_key(|advice| {
        std::cmp::Reverse((
            advice.suggestion.is_some() as usize,
            (!advice.never_in_sentence.is_empty() && advice.grounds_ungated > 0) as usize,
            advice
                .suggestion
                .as_ref()
                .map(|gate| pick_outcome(advice, gate).blocked)
                .unwrap_or(advice.grounds_ungated),
        ))
    });
    GateAdvisorReport { rules }
}

fn pick_outcome(advice: &GateAdvice, gate: &[EndpointRef]) -> GateOutcome {
    match gate {
        [EndpointRef::Source] => advice.source_gate,
        [EndpointRef::Target] => advice.target_gate,
        _ => advice.both_gate,
    }
}

fn describe(gate: &[EndpointRef]) -> String {
    gate.iter()
        .map(|e| match e {
            EndpointRef::Source => "source",
            EndpointRef::Target => "target",
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
#[path = "advisor_tests.rs"]
mod tests;
