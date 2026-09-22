//! Gap-directed neuron proposals: the neural half of the loop. The model
//! proposes; validation and human acceptance dispose. Proposals are ALWAYS
//! emitted with `status: proposed` — nothing here can touch the graph.

use anyhow::Result;
use cognigraph_embeddings::completion::CompletionProvider;
use serde_json::json;

use crate::types::{Chunk, Fact, Neuron, NeuronSet, NeuronStatus, SpaceType};
use crate::validate::validate_neurons;

/// Why a gap produced no proposal — visible, never silent.
#[derive(Debug, Clone)]
pub struct ProposalSkip {
    pub fact: Fact,
    /// Stable snake_case code naming the gate that refused (CG-90).
    pub gate: &'static str,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct ProposalReport {
    pub set: NeuronSet,
    pub skipped: Vec<ProposalSkip>,
}

/// What a proposed blocker actually suppresses, simulated at proposal time
/// by re-grounding the violating chunks under the candidate veto.
#[derive(Debug, Clone)]
pub struct BlockerCoverage {
    pub neuron_id: String,
    /// Violating chunks the veto suppresses.
    pub suppressed: Vec<String>,
    /// Violating chunks that STILL ground the forbidden fact — the human
    /// reviewer sees incomplete coverage instead of discovering it later.
    pub uncovered: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BlockerProposalReport {
    pub set: NeuronSet,
    pub skipped: Vec<ProposalSkip>,
    pub coverage: Vec<BlockerCoverage>,
}

/// The end state of coverage-guided iteration for one violated fact.
#[derive(Debug, Clone)]
pub struct FactCoverage {
    pub fact: Fact,
    /// Chunks grounding the forbidden fact before any proposed veto.
    pub violating_total: usize,
    /// Proposal rounds actually run.
    pub rounds: usize,
    /// True when no violating chunk still grounds the fact under the
    /// accumulated proposed vetoes.
    pub covered: bool,
    pub neuron_ids: Vec<String>,
    /// Chunks still grounding the fact when iteration stopped.
    pub uncovered: Vec<String>,
    /// Why iteration stopped before full coverage (None when covered).
    pub stopped: Option<String>,
}

/// The iterated mirror of [`BlockerProposalReport`]: one-shot proposal
/// left 8/9 hostile violations with disclosed-incomplete coverage; this
/// report carries the per-fact iteration story instead.
#[derive(Debug, Clone)]
pub struct IteratedBlockerReport {
    pub set: NeuronSet,
    pub skipped: Vec<ProposalSkip>,
    /// Per accepted proposal round: what it newly suppressed and what
    /// remained after it.
    pub coverage: Vec<BlockerCoverage>,
    pub facts: Vec<FactCoverage>,
}

fn neuron_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "type": { "type": "string", "enum": ["relation_hint"] },
            "confidence": { "type": "number" },
            "rationale": { "type": "string" },
            "source": { "type": "string" },
            "relation": { "type": "string" },
            "target": { "type": "string" },
            "triggers": { "type": "array", "items": { "type": "string" } },
            "evidence": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["id", "type", "source", "relation", "target", "triggers", "evidence"],
        "additionalProperties": true
    })
}

/// Candidate evidence: chunks mentioning the fact's endpoints (by name or
/// alias). Chunks mentioning BOTH endpoints rank first — a chunk that names
/// only the popular endpoint rarely asserts the relation, and feeding the
/// model those makes it decline facts the document does support.
fn candidate_chunks<'a>(fact: &Fact, space: &SpaceType, chunks: &'a [Chunk]) -> Vec<&'a Chunk> {
    let surfaces_of = |name: &str| -> Vec<String> {
        space
            .entities
            .iter()
            .filter(|e| e.name == name)
            .flat_map(|e| std::iter::once(e.name.clone()).chain(e.aliases.iter().cloned()))
            .map(|s| s.to_lowercase())
            .collect()
    };
    let source_surfaces = surfaces_of(&fact.source);
    let target_surfaces = surfaces_of(&fact.target);
    let mut scored: Vec<(usize, &Chunk)> = chunks
        .iter()
        .filter_map(|chunk| {
            let text = chunk.text.to_lowercase();
            let hits_source = source_surfaces.iter().any(|s| text.contains(s));
            let hits_target = target_surfaces.iter().any(|s| text.contains(s));
            match (hits_source, hits_target) {
                (true, true) => Some((0, chunk)),
                (true, false) | (false, true) => Some((1, chunk)),
                (false, false) => None,
            }
        })
        .collect();
    scored.sort_by_key(|(score, _)| *score); // stable: keeps document order per tier
    scored.into_iter().map(|(_, c)| c).take(6).collect()
}

/// One proposal per coverage gap (the research found aggregate requests
/// unreliable; per-gap calls worked). Every failure is retried and then
/// reported as a skip — never silently dropped. Invalid proposals are
/// recorded, not shipped for review; the loop's controller stays human.
pub async fn propose_neurons(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    missing: &[Fact],
    chunks: &[Chunk],
) -> Result<NeuronSet> {
    Ok(propose_neurons_report(provider, space, missing, chunks)
        .await?
        .set)
}

pub async fn propose_neurons_report(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    missing: &[Fact],
    chunks: &[Chunk],
) -> Result<ProposalReport> {
    let mut neurons: Vec<Neuron> = Vec::new();
    let mut skipped: Vec<ProposalSkip> = Vec::new();
    for (index, fact) in missing.iter().enumerate() {
        let evidence = candidate_chunks(fact, space, chunks);
        match propose_one(provider, space, fact, index, evidence, chunks, &neurons).await {
            Ok(neuron) => neurons.push(neuron),
            Err(reason) => skipped.push(ProposalSkip {
                fact: fact.clone(),
                gate: "proposal_rejected",
                reason,
            }),
        }
    }
    Ok(ProposalReport {
        set: NeuronSet {
            space_type: space.id.clone(),
            neurons,
        },
        skipped,
    })
}

/// B4 (roadmap-2026-h2.md): gap-directed proposing with evidence retrieved
/// through the backend's own search instead of surface matching — the
/// generalization experiment proved retrieval bounds proposal recall.
/// Per gap: BM25 over the ingested `chunks` collection (query = endpoints +
/// relation words), unioned with both-endpoint surface matches (precision
/// anchor), capped at 6. The whole space's chunk set backs the verbatim
/// trigger self-check.
pub async fn propose_neurons_via_backend(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    missing: &[Fact],
    backend: &dyn cognigraph_core::GraphBackend,
    space_id: &str,
) -> Result<ProposalReport> {
    use cognigraph_core::{FieldPredicate, PredicateOp};
    // The space's chunks, fetched once (filtered scan by space_id).
    let predicate = [FieldPredicate {
        path: vec!["space_id".into()],
        op: PredicateOp::Eq,
        value: serde_json::Value::String(space_id.to_string()),
    }];
    let corpus: Vec<Chunk> = backend
        .list_documents_filtered("chunks", &predicate, None, None, None)
        .await?
        .into_iter()
        .filter_map(|doc| {
            Some(Chunk {
                id: doc.get("_key")?.as_str()?.to_string(),
                title: doc
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_string(),
                text: doc.get("text")?.as_str()?.to_string(),
            })
        })
        .collect();

    let mut neurons: Vec<Neuron> = Vec::new();
    let mut skipped: Vec<ProposalSkip> = Vec::new();
    for (index, fact) in missing.iter().enumerate() {
        // Precision anchor: both-endpoint surface matches rank first.
        let mut evidence = candidate_chunks(fact, space, &corpus);
        // Recall extension: BM25 over the chunk text for endpoint +
        // relation wording (RESPONDED_WITH -> "responded with").
        let query = format!(
            "{} {} {}",
            fact.source,
            fact.relation.replace('_', " ").to_lowercase(),
            fact.target
        );
        if let Ok(hits) = backend
            .text_search("chunks", &query, &["text".into()], 8)
            .await
        {
            for hit in hits {
                let Some(key) = hit.document.get("_key").and_then(|k| k.as_str()) else {
                    continue;
                };
                if hit.document.get("space_id").and_then(|v| v.as_str()) != Some(space_id) {
                    continue;
                }
                if let Some(chunk) = corpus.iter().find(|c| c.id == key)
                    && !evidence.iter().any(|c| c.id == chunk.id)
                {
                    evidence.push(chunk);
                }
            }
        }
        evidence.truncate(6);
        match propose_one(provider, space, fact, index, evidence, &corpus, &neurons).await {
            Ok(neuron) => neurons.push(neuron),
            Err(reason) => skipped.push(ProposalSkip {
                fact: fact.clone(),
                gate: "proposal_rejected",
                reason,
            }),
        }
    }
    Ok(ProposalReport {
        set: NeuronSet {
            space_type: space.id.clone(),
            neurons,
        },
        skipped,
    })
}

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

const ATTEMPTS: usize = 2;

async fn propose_one(
    provider: &dyn CompletionProvider,
    space: &SpaceType,
    fact: &Fact,
    index: usize,
    evidence: Vec<&Chunk>,
    corpus: &[Chunk],
    existing: &[Neuron],
) -> std::result::Result<Neuron, String> {
    if evidence.is_empty() {
        return Err("no candidate evidence chunks mention the endpoints".into());
    }
    let evidence_text: Vec<&str> = evidence.iter().map(|c| c.text.as_str()).collect();
    let user = format!(
        "Missing fact: {} --{}--> {}\n\nCandidate evidence chunks:\n{}\n\n\
         Propose exactly ONE relation_hint neuron that recovers this fact.\n\
         Rules:\n\
         - Triggers must be short phrases copied VERBATIM from sentences that \
         explicitly assert this specific relation between this source and target.\n\
         - Do NOT use sentences that only imply the fact by category membership \
         or co-occurrence; if no sentence explicitly supports the fact, return \
         an empty triggers array.\n\
         - id must be descriptive kebab-case (e.g. \"kurtz-leads-crowdstrike\").\n\
         - evidence must quote the licensing sentences.",
        fact.source,
        fact.relation,
        fact.target,
        evidence_text.join("\n---\n")
    );
    let system = "You propose Semantic Neurons: governed edge-construction rules. \
                  You may ONLY reference the given source, relation, and target. \
                  You never invent entities or relations. Precision beats recall: \
                  an unsupported proposal is worse than no proposal.";

    let mut last_error = String::new();
    for _ in 0..ATTEMPTS {
        let value = match provider
            .complete_json(system, &user, &neuron_schema())
            .await
        {
            Ok(value) => value,
            Err(e) => {
                last_error = format!("completion error: {e}");
                continue;
            }
        };
        // Tolerate a {"neurons": [ ... ]} wrapper shape.
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
        // Normalize and force the governance invariants.
        neuron.status = NeuronStatus::Proposed;
        neuron.source = fact.source.clone();
        neuron.relation = fact.relation.clone();
        neuron.target = fact.target.clone();
        neuron.id = kebab(&neuron.id);
        if neuron.id.is_empty() || existing.iter().any(|n| n.id == neuron.id) {
            neuron.id = kebab(&format!(
                "{}-{}-{}-{index}",
                fact.source, fact.relation, fact.target
            ));
        }
        if neuron.triggers.is_empty() {
            // A decline is an answer, but a single sample is noisy — it
            // stands only if the model declines on every attempt.
            last_error = "model declined: no explicit supporting sentence".into();
            continue;
        }
        // Symbolic self-check: a trigger that never occurs verbatim in the
        // document can never ground. Paraphrased triggers are a retry case.
        let before = neuron.triggers.len();
        neuron.triggers.retain(|t| {
            let t = t.to_lowercase();
            corpus.iter().any(|c| c.text.to_lowercase().contains(&t))
        });
        if neuron.triggers.is_empty() {
            last_error = format!(
                "all {before} triggers were paraphrased (none occur verbatim in the document)"
            );
            continue;
        }
        // Per-neuron validation: one bad proposal must not sink the batch.
        // Accepted shape exercises the full check set.
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

fn kebab(raw: &str) -> String {
    raw.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns scripted proposals in order, one per call.
    struct Seq(std::sync::Mutex<Vec<serde_json::Value>>);

    #[async_trait::async_trait]
    impl CompletionProvider for Seq {
        async fn complete_json(
            &self,
            _system: &str,
            _user: &str,
            _schema: &serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            Ok(self.0.lock().unwrap().remove(0))
        }
        fn model_name(&self) -> &str {
            "seq"
        }
    }

    fn space() -> SpaceType {
        serde_json::from_value(json!({
            "id": "s",
            "entities": [
                {"name": "Harbor", "type": "org", "aliases": []},
                {"name": "Harbor Chat", "type": "product", "aliases": []}
            ],
            "relation_rules": [
                {"source": "Harbor", "relation": "CO_DEVELOPED", "target": "Harbor Chat",
                 "when_any": ["co-developed harbor chat"]}
            ]
        }))
        .unwrap()
    }

    fn chunk(id: &str, text: &str) -> Chunk {
        Chunk {
            id: id.into(),
            title: String::new(),
            text: text.into(),
        }
    }

    fn blocker(id: &str, phrase: &str) -> serde_json::Value {
        json!({
            "id": id, "type": "relation_blocker", "confidence": 0.9,
            "rationale": "test", "source": "Harbor", "relation": "CO_DEVELOPED",
            "target": "Harbor Chat", "when_any": [phrase],
            "evidence": ["quoted"]
        })
    }

    #[tokio::test]
    async fn iteration_covers_the_remainder_round_by_round() {
        let space = space();
        let fact = Fact::parse("Harbor --CO_DEVELOPED--> Harbor Chat").unwrap();
        // Two violating chunks with DISJOINT illegitimate-context wording:
        // one veto phrase cannot cover both — the one-shot failure shape.
        let chunks = vec![
            chunk(
                "v1",
                "Rumors allege a rival co-developed Harbor Chat with partners.",
            ),
            chunk(
                "v2",
                "An unverified filing claims a vendor co-developed Harbor Chat.",
            ),
        ];
        // Round 1 covers v1 only; round 2 is asked ONLY about v2.
        let provider = Seq(std::sync::Mutex::new(vec![
            blocker("veto-allegation", "rumors allege"),
            blocker("veto-filing", "unverified filing"),
        ]));

        let report = propose_blockers_covering(&provider, &space, &[fact], &chunks, 5)
            .await
            .unwrap();
        assert_eq!(report.set.neurons.len(), 2);
        assert_eq!(report.facts.len(), 1);
        let outcome = &report.facts[0];
        assert!(outcome.covered, "two rounds must cover both chunks");
        assert_eq!(outcome.rounds, 2);
        assert_eq!(outcome.violating_total, 2);
        assert!(outcome.uncovered.is_empty());
        assert_eq!(outcome.stopped, None);
        // Round records: round 1 suppressed v1 leaving v2, round 2 closed it.
        assert_eq!(report.coverage[0].suppressed, vec!["v1"]);
        assert_eq!(report.coverage[0].uncovered, vec!["v2"]);
        assert_eq!(report.coverage[1].suppressed, vec!["v2"]);
        assert!(report.coverage[1].uncovered.is_empty());
    }

    #[tokio::test]
    async fn iteration_stops_at_the_round_cap_and_discloses_the_remainder() {
        let space = space();
        let fact = Fact::parse("Harbor --CO_DEVELOPED--> Harbor Chat").unwrap();
        let chunks = vec![
            chunk(
                "v1",
                "Rumors allege a rival co-developed Harbor Chat with partners.",
            ),
            chunk(
                "v2",
                "An unverified filing claims a vendor co-developed Harbor Chat.",
            ),
        ];
        let provider = Seq(std::sync::Mutex::new(vec![blocker(
            "veto-allegation",
            "rumors allege",
        )]));
        let report = propose_blockers_covering(&provider, &space, &[fact], &chunks, 1)
            .await
            .unwrap();
        let outcome = &report.facts[0];
        assert!(!outcome.covered);
        assert_eq!(outcome.uncovered, vec!["v2"]);
        assert_eq!(outcome.stopped.as_deref(), Some("round cap 1 reached"));
    }

    #[tokio::test]
    async fn a_failed_round_is_a_visible_skip_never_a_silent_stop() {
        let space = space();
        let fact = Fact::parse("Harbor --CO_DEVELOPED--> Harbor Chat").unwrap();
        let chunks = vec![chunk(
            "v1",
            "Rumors allege a rival co-developed Harbor Chat with partners.",
        )];
        // The proposal's phrase is a paraphrase (not verbatim): the
        // symbolic self-check rejects it on both attempts.
        let provider = Seq(std::sync::Mutex::new(vec![
            blocker("veto-bad", "gossip suggests"),
            blocker("veto-bad", "gossip suggests"),
        ]));
        let report = propose_blockers_covering(&provider, &space, &[fact], &chunks, 3)
            .await
            .unwrap();
        assert!(report.set.neurons.is_empty());
        let outcome = &report.facts[0];
        assert!(!outcome.covered);
        assert_eq!(outcome.rounds, 1);
        assert!(outcome.stopped.as_deref().unwrap().contains("round 1"));
        assert_eq!(report.skipped.len(), 1);
        assert!(report.skipped[0].reason.contains("paraphrased"));
    }
}
