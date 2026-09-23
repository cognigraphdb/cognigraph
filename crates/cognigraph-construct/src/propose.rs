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

mod blockers;
mod neurons;
pub use blockers::{propose_blockers_covering, propose_blockers_report};
use neurons::{candidate_chunks, propose_one};

#[cfg(test)]
#[path = "propose_tests.rs"]
mod tests;
