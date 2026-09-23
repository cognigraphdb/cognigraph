//! Blocker proposal: restraint violations become candidate relation blockers.

use super::neurons::{ATTEMPTS, kebab};
use super::*;

/// B3 (roadmap-2026-h2.md): the symmetric loop — a restraint violation
/// proposes a `relation_blocker`, exactly as a coverage gap proposes a
/// `relation_hint`. Per violated forbidden fact: find the chunks that
/// ground it (with the trigger that fired), ask for veto phrases copied
/// verbatim from those chunks, self-check the phrases symbolically, and
/// simulate coverage. Proposals arrive `proposed` — review stays human.
pub async fn propose_blockers_report(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    violations: &[Fact],
    chunks: &[Chunk],
) -> Result<BlockerProposalReport> {
    let mut neurons: Vec<Neuron> = Vec::new();
    let mut skipped: Vec<ProposalSkip> = Vec::new();
    let mut coverage: Vec<BlockerCoverage> = Vec::new();
    for (index, fact) in violations.iter().enumerate() {
        // The violating evidence: chunks grounding this forbidden fact.
        let violating: Vec<(&Chunk, String)> = chunks
            .iter()
            .filter_map(|chunk| {
                crate::grounding::ground_chunk(&chunk.id, &chunk.text, space, &[])
                    .into_iter()
                    .find(|g| g.fact == *fact)
                    .map(|g| (chunk, g.trigger))
            })
            .collect();
        if violating.is_empty() {
            skipped.push(ProposalSkip {
                fact: fact.clone(),
                gate: "forbidden_fact_not_grounded",
                reason: "forbidden fact does not ground from any chunk".into(),
            });
            continue;
        }
        match propose_one_blocker(provider, space, fact, index, &violating, &neurons, &[]).await {
            Ok(neuron) => {
                // Simulate: which violating chunks does this veto suppress?
                let vetoes = [crate::grounding::VetoRule {
                    source: neuron.source.clone(),
                    relation: neuron.relation.clone(),
                    target: neuron.target.clone(),
                    when_any: neuron.triggers.clone(),
                }];
                let mut suppressed = Vec::new();
                let mut uncovered = Vec::new();
                for (chunk, _) in &violating {
                    let still =
                        crate::grounding::ground_chunk(&chunk.id, &chunk.text, space, &vetoes)
                            .iter()
                            .any(|g| g.fact == *fact);
                    if still {
                        uncovered.push(chunk.id.clone());
                    } else {
                        suppressed.push(chunk.id.clone());
                    }
                }
                coverage.push(BlockerCoverage {
                    neuron_id: neuron.id.clone(),
                    suppressed,
                    uncovered,
                });
                neurons.push(neuron);
            }
            Err(reason) => skipped.push(ProposalSkip {
                fact: fact.clone(),
                gate: "blocker_rejected",
                reason,
            }),
        }
    }
    Ok(BlockerProposalReport {
        set: NeuronSet {
            space_type: space.id.clone(),
            neurons,
        },
        skipped,
        coverage,
    })
}

/// Coverage-guided iteration (the blocker-repair experiment's fix): per
/// violated fact, propose a veto, simulate what it suppresses, and
/// re-propose against ONLY the still-uncovered chunks until the fact is
/// fully covered, a round suppresses nothing new (dry), a proposal
/// fails, or `max_rounds` is hit. A round that suppresses nothing is
/// DROPPED, not kept — a useless veto is pure risk. Everything is still
/// emitted `status: proposed`; iteration changes how thoroughly the
/// proposer works, never who authorizes.
pub async fn propose_blockers_covering(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    violations: &[Fact],
    chunks: &[Chunk],
    max_rounds: usize,
) -> Result<IteratedBlockerReport> {
    let mut neurons: Vec<Neuron> = Vec::new();
    let mut skipped: Vec<ProposalSkip> = Vec::new();
    let mut coverage: Vec<BlockerCoverage> = Vec::new();
    let mut facts: Vec<FactCoverage> = Vec::new();
    let mut counter = 0usize;

    for fact in violations {
        let violating_all: Vec<(&Chunk, String)> = chunks
            .iter()
            .filter_map(|chunk| {
                crate::grounding::ground_chunk(&chunk.id, &chunk.text, space, &[])
                    .into_iter()
                    .find(|g| g.fact == *fact)
                    .map(|g| (chunk, g.trigger))
            })
            .collect();
        if violating_all.is_empty() {
            skipped.push(ProposalSkip {
                fact: fact.clone(),
                gate: "forbidden_fact_not_grounded",
                reason: "forbidden fact does not ground from any chunk".into(),
            });
            continue;
        }

        let mut vetoes: Vec<crate::grounding::VetoRule> = Vec::new();
        let mut neuron_ids: Vec<String> = Vec::new();
        let mut uncovered: Vec<(&Chunk, String)> = violating_all.clone();
        let mut rounds = 0usize;
        let mut stopped: Option<String> = None;

        while !uncovered.is_empty() && rounds < max_rounds {
            rounds += 1;
            let prior: Vec<String> = vetoes.iter().flat_map(|v| v.when_any.clone()).collect();
            let proposed =
                propose_one_blocker(provider, space, fact, counter, &uncovered, &neurons, &prior)
                    .await;
            counter += 1;
            let neuron = match proposed {
                Ok(neuron) => neuron,
                Err(reason) => {
                    let reason = format!("round {rounds}: {reason}");
                    skipped.push(ProposalSkip {
                        fact: fact.clone(),
                        gate: "blocker_round_failed",
                        reason: reason.clone(),
                    });
                    stopped = Some(reason);
                    break;
                }
            };
            let mut candidate = vetoes.clone();
            candidate.push(crate::grounding::VetoRule {
                source: neuron.source.clone(),
                relation: neuron.relation.clone(),
                target: neuron.target.clone(),
                when_any: neuron.triggers.clone(),
            });
            let (suppressed, remaining): (Vec<_>, Vec<_>) =
                uncovered.into_iter().partition(|(chunk, _)| {
                    !crate::grounding::ground_chunk(&chunk.id, &chunk.text, space, &candidate)
                        .iter()
                        .any(|g| g.fact == *fact)
                });
            if suppressed.is_empty() {
                // Dry round: the proposal covers nothing new — drop it.
                let reason = format!(
                    "round {rounds}: proposal `{}` suppressed nothing new; stopping (dry)",
                    neuron.id
                );
                skipped.push(ProposalSkip {
                    fact: fact.clone(),
                    gate: "blocker_round_dry",
                    reason: reason.clone(),
                });
                stopped = Some(reason);
                uncovered = remaining;
                break;
            }
            coverage.push(BlockerCoverage {
                neuron_id: neuron.id.clone(),
                suppressed: suppressed.iter().map(|(c, _)| c.id.clone()).collect(),
                uncovered: remaining.iter().map(|(c, _)| c.id.clone()).collect(),
            });
            neuron_ids.push(neuron.id.clone());
            vetoes = candidate;
            neurons.push(neuron);
            uncovered = remaining;
        }

        if !uncovered.is_empty() && stopped.is_none() {
            stopped = Some(format!("round cap {max_rounds} reached"));
        }
        facts.push(FactCoverage {
            fact: fact.clone(),
            violating_total: violating_all.len(),
            rounds,
            covered: uncovered.is_empty(),
            neuron_ids,
            uncovered: uncovered.iter().map(|(c, _)| c.id.clone()).collect(),
            stopped,
        });
    }

    Ok(IteratedBlockerReport {
        set: NeuronSet {
            space_type: space.id.clone(),
            neurons,
        },
        skipped,
        coverage,
        facts,
    })
}

async fn propose_one_blocker(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    fact: &Fact,
    index: usize,
    violating: &[(&Chunk, String)],
    existing: &[Neuron],
    prior_phrases: &[String],
) -> std::result::Result<Neuron, String> {
    let passages: Vec<String> = violating
        .iter()
        .map(|(chunk, trigger)| {
            format!(
                "[chunk {}] (fired trigger: `{trigger}`)\n{}",
                chunk.id, chunk.text
            )
        })
        .collect();
    let prior_note = if prior_phrases.is_empty() {
        String::new()
    } else {
        format!(
            "\n         Veto phrases already in place for this fact: {prior_phrases:?}. \
             They do NOT appear in the passages below — do not repeat them; your \
             phrases must occur verbatim in THESE passages."
        )
    };
    let user = format!(
        "FORBIDDEN fact wrongly constructed: {} --{}--> {}

         Violating passages:
{}
{prior_note}
         Propose exactly ONE relation_blocker neuron that suppresses this          fact in these passages.
         Rules:
         - Veto phrases (when_any) must be short phrases copied VERBATIM          from the violating passages, choosing wording that marks the          ILLEGITIMATE context (allegation, investigation, negation,          speculation) — not wording that would appear where the relation          is genuinely asserted.
         - id must be descriptive kebab-case.
         - evidence must quote the violating sentences.",
        fact.source,
        fact.relation,
        fact.target,
        passages.join("
---
")
    );
    let system = "You propose Semantic Neuron relation_blockers: governed                   extraction-time vetoes. You may ONLY reference the given                   source, relation, and target. A veto that also fires on                   legitimate assertions is worse than no veto.";

    let mut last_error = String::new();
    for _ in 0..ATTEMPTS {
        let value = match provider
            .complete_json(system, &user, &blocker_schema())
            .await
        {
            Ok(value) => value,
            Err(e) => {
                last_error = format!("completion error: {e}");
                continue;
            }
        };
        let value = value
            .get("neurons")
            .and_then(|n| n.as_array())
            .and_then(|a| a.first())
            .cloned()
            .unwrap_or(value);
        let mut neuron: Neuron = match serde_json::from_value(value) {
            Ok(neuron) => neuron,
            Err(e) => {
                last_error = format!("unparseable proposal: {e}");
                continue;
            }
        };
        // Force the governance invariants.
        neuron.kind = crate::types::NeuronKind::RelationBlocker;
        neuron.status = NeuronStatus::Proposed;
        neuron.source = fact.source.clone();
        neuron.relation = fact.relation.clone();
        neuron.target = fact.target.clone();
        neuron.id = kebab(&neuron.id);
        if neuron.id.is_empty() || existing.iter().any(|n| n.id == neuron.id) {
            neuron.id = kebab(&format!(
                "block-{}-{}-{}-{index}",
                fact.source, fact.relation, fact.target
            ));
        }
        // Symbolic self-check: a veto that never occurs verbatim in a
        // violating chunk can never suppress anything.
        let before = neuron.triggers.len();
        neuron.triggers.retain(|phrase| {
            let phrase = phrase.to_lowercase();
            violating
                .iter()
                .any(|(chunk, _)| chunk.text.to_lowercase().contains(&phrase))
        });
        if neuron.triggers.is_empty() {
            last_error = format!(
                "all {before} veto phrases were paraphrased (none occur verbatim in the violating chunks)"
            );
            continue;
        }
        // Per-neuron validation against the ontology (accepted shape).
        let mut check = neuron.clone();
        check.status = NeuronStatus::Accepted;
        let single = NeuronSet {
            space_type: space.id.clone(),
            neurons: vec![check],
        };
        if let Err(e) = validate_neurons(&single, space) {
            last_error = format!("invalid proposal: {e}");
            continue;
        }
        return Ok(neuron);
    }
    Err(last_error)
}

fn blocker_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "type": { "type": "string", "enum": ["relation_blocker"] },
            "confidence": { "type": "number" },
            "rationale": { "type": "string" },
            "source": { "type": "string" },
            "relation": { "type": "string" },
            "target": { "type": "string" },
            "when_any": { "type": "array", "items": { "type": "string" } },
            "evidence": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["id", "type", "source", "relation", "target", "when_any", "evidence"],
        "additionalProperties": true
    })
}
