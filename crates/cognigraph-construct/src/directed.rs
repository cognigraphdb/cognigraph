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

/// Find `needle` in `haystack`, case-insensitive (ASCII) and tolerant of
/// whitespace differences: any whitespace RUN on either side matches any
/// whitespace run on the other. Returns byte offsets into the ORIGINAL
/// haystack, so evidence spans point at the real text even when the model
/// collapsed a line break into a space.
pub(crate) fn find_loose(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let needle = needle.trim();
    if needle.is_empty() {
        return None;
    }
    let hay = haystack.as_bytes();
    let need = needle.as_bytes();
    let mut start = 0usize;
    while start < hay.len() {
        // Candidate must begin on a non-space matching needle's first byte.
        if hay[start].is_ascii_whitespace()
            || !need
                .first()
                .is_some_and(|&b| b.eq_ignore_ascii_case(&hay[start]))
        {
            start += 1;
            continue;
        }
        let (mut i, mut j) = (start, 0usize);
        loop {
            if j >= need.len() {
                return Some((start, i));
            }
            if i >= hay.len() {
                break;
            }
            let (hb, nb) = (hay[i], need[j]);
            if nb.is_ascii_whitespace() {
                if !hb.is_ascii_whitespace() {
                    break;
                }
                while i < hay.len() && hay[i].is_ascii_whitespace() {
                    i += 1;
                }
                while j < need.len() && need[j].is_ascii_whitespace() {
                    j += 1;
                }
                continue;
            }
            if !hb.eq_ignore_ascii_case(&nb) {
                break;
            }
            i += 1;
            j += 1;
        }
        start += 1;
    }
    None
}

/// Endpoint mentions use the quote matcher's ASCII case/whitespace tolerance,
/// but neither adjacent character may be a Unicode word character (UTS #18:
/// Alphabetic, Mark, Decimal_Number, Connector_Punctuation or Join_Control).
/// Punctuation such as apostrophes and hyphens separates tokens. This prevents
/// partial words, not partial multi-word names or incorrect entity resolution;
/// unsegmented scripts conservatively require a separator around the name.
/// Keep this separate from quote lookup so evidence byte spans do not change.
fn find_endpoint(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    while let Some((start, end)) = find_loose(&haystack[offset..], needle) {
        let (start, end) = (offset + start, offset + end);
        let touches_word = haystack[..start]
            .chars()
            .next_back()
            .is_some_and(regex_syntax::is_word_character)
            || haystack[end..]
                .chars()
                .next()
                .is_some_and(regex_syntax::is_word_character);
        if !touches_word {
            return Some((start, end));
        }
        // An invalid first substring must not hide a later complete mention.
        offset = start + haystack[start..].chars().next()?.len_utf8();
    }
    None
}

/// Run every proposal through the gates. Pure — the LLM and the backend sit
/// on either side of this function, which is what makes the restraint
/// behaviour testable in isolation.
/// Canonically equivalent quotes/endpoints are compared in NFC; returned spans
/// index NFC chunk text, the same representation used by the ingestion writer.
pub fn gate_directed_proposals(
    chunks: &[Chunk],
    taxonomy: &[DirectedRelation],
    proposals: &[DirectedProposal],
    extracted_by: &str,
) -> (Vec<(String, GroundedFact)>, Vec<EntityDef>, Vec<Refusal>) {
    let chunks = canonical_chunks(chunks);
    let by_id: BTreeMap<&str, &Chunk> = chunks.iter().map(|c| (c.id.as_str(), c)).collect();
    let by_relation: BTreeMap<&str, &DirectedRelation> =
        taxonomy.iter().map(|r| (r.relation.as_str(), r)).collect();

    let mut grounded: Vec<(String, GroundedFact)> = Vec::new();
    let mut entities: BTreeMap<String, EntityDef> = BTreeMap::new();
    let mut skips: Vec<Refusal> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for proposal in proposals {
        let p = DirectedProposal {
            source: canonical_text(&proposal.source).into_owned(),
            target: canonical_text(&proposal.target).into_owned(),
            evidence: canonical_text(&proposal.evidence).into_owned(),
            ..proposal.clone()
        };
        let label = format!("{} --{}--> {}", p.source, p.relation, p.target);
        let refusal = |gate: DirectedGate, reason: String| Refusal {
            gate,
            source: p.source.clone(),
            relation: p.relation.clone(),
            target: p.target.clone(),
            chunk_id: p.chunk_id.clone(),
            evidence: p.evidence.clone(),
            reason,
        };
        let Some(rule) = by_relation.get(p.relation.as_str()) else {
            skips.push(refusal(
                DirectedGate::RelationNotInTaxonomy,
                format!("`{label}`: relation not in the taxonomy — dropped"),
            ));
            continue;
        };
        let Some(chunk) = by_id.get(p.chunk_id.as_str()) else {
            skips.push(refusal(
                DirectedGate::ChunkNotInRequest,
                format!(
                    "`{label}`: cites chunk `{}` which is not in this request — dropped",
                    p.chunk_id
                ),
            ));
            continue;
        };
        if p.source.trim().is_empty() || p.target.trim().is_empty() {
            skips.push(refusal(
                DirectedGate::EmptyEndpoint,
                format!("`{label}`: empty endpoint — dropped"),
            ));
            continue;
        }
        // An endpoint must sanitize to a usable identity: "[***]" (a
        // redaction) passes every textual gate but has no key to live
        // under, and letting it through would fail the whole atomic write.
        if entity_key(&p.source).is_empty() || entity_key(&p.target).is_empty() {
            skips.push(refusal(
                DirectedGate::UnusableEndpointIdentity,
                format!("`{label}`: an endpoint has no usable identity (symbols only) — dropped"),
            ));
            continue;
        }
        let Some((ev_start, ev_end)) = find_loose(&chunk.text, &p.evidence) else {
            skips.push(refusal(
                DirectedGate::EvidenceNotVerbatim,
                format!(
                    "`{label}`: evidence is not a verbatim quote of chunk `{}` — dropped",
                    p.chunk_id
                ),
            ));
            continue;
        };
        // The gate window: the sentence the quote starts in, extended to the
        // end of the quote when it crosses a boundary.
        let (s_start, s_end) = sentence_bounds(&chunk.text, ev_start);
        let window = &chunk.text[s_start..s_end.max(ev_end)];
        if find_endpoint(window, &p.source).is_none() {
            skips.push(refusal(
                DirectedGate::SourceNotInSentence,
                format!("`{label}`: source does not occur in the evidence sentence — dropped"),
            ));
            continue;
        }
        if find_endpoint(window, &p.target).is_none() {
            skips.push(refusal(
                DirectedGate::TargetNotInSentence,
                format!("`{label}`: target does not occur in the evidence sentence — dropped"),
            ));
            continue;
        }
        let affirmed = rule.require_in_sentence.iter().any(|phrase| {
            !phrase.trim().is_empty() && affirms_phrase(window, &canonical_text(phrase))
        });
        if !affirmed {
            skips.push(refusal(
                DirectedGate::VocabularyNotAffirmed,
                format!(
                    "`{label}`: no affirmed occurrence of the relation's vocabulary in the \
                     evidence sentence — dropped"
                ),
            ));
            continue;
        }
        if !seen.insert((
            entity_key(&p.source),
            rule.relation.clone(),
            entity_key(&p.target),
            chunk.id.clone(),
            ev_start,
        )) {
            continue;
        }
        for (name, kind) in [(&p.source, &p.source_type), (&p.target, &p.target_type)] {
            let key = entity_key(name);
            entities.entry(key).or_insert_with(|| EntityDef {
                name: name.trim().to_string(),
                entity_type: if kind.trim().is_empty() {
                    "entity".into()
                } else {
                    kind.trim().to_string()
                },
                aliases: Vec::new(),
            });
        }
        grounded.push((
            chunk.id.clone(),
            GroundedFact {
                fact: Fact {
                    source: p.source.trim().to_string(),
                    relation: rule.relation.clone(),
                    target: p.target.trim().to_string(),
                },
                trigger: chunk.text[ev_start..ev_end].to_string(),
                chunk_id: chunk.id.clone(),
                licensed_by_neuron: None,
                reviewed_by: Some(extracted_by.to_string()),
                trigger_span: (ev_start, ev_end),
            },
        ));
    }
    (grounded, entities.into_values().collect(), skips)
}

fn validate_taxonomy(taxonomy: &[DirectedRelation]) -> Result<()> {
    if taxonomy.is_empty() {
        return Err(CogniGraphError::ValidationError(
            "directed construction needs a non-empty taxonomy".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for rule in taxonomy {
        if rule.relation.trim().is_empty() {
            return Err(CogniGraphError::ValidationError(
                "taxonomy relation names must be non-empty".into(),
            ));
        }
        if !seen.insert(rule.relation.trim().to_string()) {
            return Err(CogniGraphError::ValidationError(format!(
                "duplicate taxonomy relation `{}`",
                rule.relation
            )));
        }
        if rule
            .require_in_sentence
            .iter()
            .all(|phrase| phrase.trim().is_empty())
        {
            return Err(CogniGraphError::ValidationError(format!(
                "taxonomy relation `{}` has no restraint vocabulary — every directed \
                 relation must name at least one require_in_sentence phrase",
                rule.relation
            )));
        }
    }
    Ok(())
}

const DIRECTED_SYSTEM: &str = "You extract facts from document excerpts, under restraint.\n\
Rules:\n\
- Assert ONLY relations from the taxonomy the user provides; never invent one.\n\
- `source` and `target` must be short entity names copied VERBATIM from the evidence sentence.\n\
- `evidence` must be an EXACT quote (one sentence, or a contiguous span) copied from one excerpt, \
  and `chunk_id` must be that excerpt's id.\n\
- Only assert what the quoted sentence itself states. If the document does not clearly state a \
  relation, do not report it. An empty facts list is a good answer for an excerpt with nothing \
  to say.\n\
Return JSON only.";

fn proposal_schema(taxonomy: &[DirectedRelation], chunks: &[Chunk]) -> Value {
    // IDs are opaque: bind the exact values used by the local gates, with no
    // trimming, normalization or repair. Sets keep enum order deterministic.
    let relations: BTreeSet<&str> = taxonomy.iter().map(|r| r.relation.as_str()).collect();
    let chunk_ids: BTreeSet<&str> = chunks.iter().map(|c| c.id.as_str()).collect();
    json!({
        "type": "object",
        "properties": {
            "facts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "source": {"type": "string"},
                        "source_type": {"type": "string"},
                        "target": {"type": "string"},
                        "target_type": {"type": "string"},
                        "relation": {"type": "string", "enum": relations},
                        "evidence": {"type": "string"},
                        "chunk_id": {"type": "string", "enum": chunk_ids}
                    },
                    "required": ["source", "source_type", "target", "target_type",
                                  "relation", "evidence", "chunk_id"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["facts"],
        "additionalProperties": false
    })
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

#[cfg(test)]
#[path = "directed_tests.rs"]
mod tests;
