//! Directed construction (D12): taxonomy in, evidence-bound facts out,
//! through the same gates.
//!
//! The classic pipeline extracts recurring corpus knowledge: rules are
//! instance-anchored to entities a draft saw, which is the right shape for
//! "this drug treats this condition" and the wrong shape for "does THIS
//! contract contain an exclusivity grant". Directed construction covers the
//! second kind of question: the caller supplies a fixed relation taxonomy,
//! an LLM proposes per-document facts constrained to it, and every proposal
//! must then survive deterministic gates before anything is written —
//! the model nominates, the gates decide.
//!
//! Gates, in order, each rejection recorded as a skip:
//! 1. the relation is in the taxonomy (a hallucinated relation never lands);
//! 2. the cited chunk is in the submitted corpus;
//! 3. the evidence is a verbatim quote of that chunk (whitespace-tolerant,
//!    case-insensitive — offsets are recovered into canonical NFC chunk text);
//! 4. source and target occur with Unicode word boundaries in the evidence window (the
//!    sentence containing the quote, extended to the quote's end);
//! 5. at least one of the relation's `require_in_sentence` phrases is
//!    present AND affirmed in that window — negated vocabulary does not
//!    license a fact.
//!
//! Survivors become ordinary `GroundedFact`s and flow through the SAME
//! writer as rule grounding (`ingest_chunks_grounded_by`): same
//! occurrence-v1 rows, same delete-and-rebuild reconciliation, same
//! semantics sidecar. A directed fact is distinguishable by its
//! `reviewed_by` attribution (`directed:<model>@directed-policy-v2`), not
//! by a parallel schema.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use cognigraph_core::{CogniGraphError, GraphBackend, Result};
use cognigraph_embeddings::completion::CompletionProvider;

use crate::evidence::{canonical_chunks, canonical_text};
use crate::grounding::{GroundedFact, affirms_phrase, sentence_bounds};
use crate::ingest::{entity_key, ingest_chunks_grounded_by};
use crate::refusals::{DirectedGate, Refusal};
use crate::types::{Chunk, EntityDef, Fact, SpaceType};

/// The extraction policy revision, stamped into `reviewed_by` attribution.
pub const DIRECTED_POLICY: &str = "directed-policy-v2";

/// One relation the caller wants facts for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectedRelation {
    /// Relation name facts are written under (e.g. `GRANTS_EXCLUSIVITY`).
    pub relation: String,
    /// Shown to the model verbatim — what the relation means, when to
    /// assert it, when not to.
    pub description: String,
    /// Restraint vocabulary: the evidence window must contain at least one
    /// of these phrases, affirmed. Required non-empty — a relation with no
    /// vocabulary gate would let the model's say-so stand alone.
    pub require_in_sentence: Vec<String>,
}

/// What one directed run did — counts plus every rejection, verbatim.
#[derive(Debug, Serialize)]
pub struct DirectedOutcome {
    pub proposed: usize,
    pub facts_grounded: usize,
    /// Human-readable rejections, one per refusal (compatibility surface).
    pub skips: Vec<String>,
    /// The same rejections as structured records (CG-90).
    pub refusals: Vec<Refusal>,
    pub extracted_by: String,
}

/// One model proposal, pre-gates.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectedProposal {
    pub source: String,
    pub source_type: String,
    pub target: String,
    pub target_type: String,
    pub relation: String,
    pub evidence: String,
    pub chunk_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectedResponse {
    facts: Vec<DirectedProposal>,
}

/// Extract directed facts from `chunks` and write the survivors.
///
/// One completion call over the given chunks — callers submit bounded
/// slices (the server route caps a request at one call's worth) and rely on
/// the per-chunk delete-and-rebuild reconciliation to make re-runs and
/// resubmissions idempotent.
pub async fn directed_ingest(
    backend: &dyn GraphBackend,
    space_id: &str,
    taxonomy: &[DirectedRelation],
    chunks: &[Chunk],
    provider: &dyn CompletionProvider,
) -> Result<DirectedOutcome> {
    validate_taxonomy(taxonomy)?;
    if chunks.is_empty() {
        return Err(CogniGraphError::ValidationError("chunks is empty".into()));
    }
    let canonical = canonical_chunks(chunks);
    let chunks = canonical.as_ref();
    let taxonomy_text = taxonomy
        .iter()
        .map(|r| format!("- {}: {}", r.relation, r.description))
        .collect::<Vec<_>>()
        .join("\n");
    let excerpts = chunks
        .iter()
        .map(|c| format!("[chunk {}] {}", c.id, c.text))
        .collect::<Vec<_>>()
        .join("\n---\n");
    let response = provider
        .complete_json(
            DIRECTED_SYSTEM,
            &format!(
                "Taxonomy (assert only these relations):\n{taxonomy_text}\n\n\
                 Document excerpts:\n{excerpts}\n\n\
                 Extract the facts."
            ),
            &proposal_schema(taxonomy, chunks),
        )
        .await
        .map_err(|e| CogniGraphError::BackendError(format!("directed completion failed: {e}")))?;
    // Validate the entire envelope before any backend access. A malformed
    // response must never become an empty delete-and-rebuild replacement.
    let proposals = serde_json::from_value::<DirectedResponse>(response)
        .map_err(|e| {
            CogniGraphError::BackendError(format!(
                "directed completion returned invalid facts: {e}"
            ))
        })?
        .facts;

    let extracted_by = format!("directed:{}@{}", provider.model_name(), DIRECTED_POLICY);
    let (grounded, mut entities, refusals) =
        gate_directed_proposals(chunks, taxonomy, &proposals, &extracted_by);

    // Entity identity is first-writer-wins across the store: when a key
    // already exists, adopt the stored name and type so a later document's
    // wording cannot fork (or fail) an identity the graph already holds.
    for entity in &mut entities {
        if let Some(stored) = backend
            .get_document("entities", &entity_key(&entity.name))
            .await?
            && let (Some(name), Some(kind)) = (
                stored.get("name").and_then(Value::as_str),
                stored.get("entity_type").and_then(Value::as_str),
            )
        {
            entity.name = name.to_string();
            entity.entity_type = kind.to_string();
        }
    }

    let by_chunk: BTreeMap<String, Vec<GroundedFact>> =
        grounded
            .into_iter()
            .fold(BTreeMap::new(), |mut acc, (chunk_id, fact)| {
                acc.entry(chunk_id).or_default().push(fact);
                acc
            });
    let config = SpaceType {
        id: space_id.to_string(),
        name: space_id.to_string(),
        version: 1,
        description: "directed extraction (D12)".into(),
        entities,
        relation_rules: Vec::new(),
    };
    let facts_grounded = ingest_chunks_grounded_by(backend, space_id, &config, chunks, &|chunk| {
        by_chunk.get(&chunk.id).cloned().unwrap_or_default()
    })
    .await?;

    Ok(DirectedOutcome {
        proposed: proposals.len(),
        facts_grounded,
        skips: refusals.iter().map(|r| r.reason.clone()).collect(),
        refusals,
        extracted_by,
    })
}

mod gates;
mod prompt;
pub use gates::gate_directed_proposals;
#[cfg(test)]
use gates::{find_endpoint, find_loose};
use prompt::{DIRECTED_SYSTEM, proposal_schema, validate_taxonomy};

#[cfg(test)]
#[path = "directed_tests.rs"]
mod tests;
