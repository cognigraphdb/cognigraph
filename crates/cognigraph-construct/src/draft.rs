//! The ontology drafter (decision_ontology_drafter.md): an
//! LLM-per-purpose pipeline that proposes a DRAFT space type from
//! sample chunks. The drafter holds no authority — a human reads the
//! whole artifact and accepts it explicitly (D1: drafts live in a
//! separate collection and are structurally unable to ground). Two
//! purpose-bound stages (D2): entity discovery, then rule discovery
//! against the CHECKED entity list, each behind symbolic self-checks —
//! every surface verbatim in the corpus, every trigger verbatim AND
//! affirmed (negation-aware), every rule endpoint closed over the
//! drafted entities. Precision config is never LLM-authored (D4):
//! drafts are naive, and the deterministic gate advisor annotates where
//! the precision budget is likely needed.

use anyhow::Result;
use serde_json::{Value, json};

use crate::advisor::{GateAdvisorReport, advise_gates};
use crate::grounding::affirms_phrase;
use crate::ingest::entity_key;
use crate::types::{Chunk, EntityDef, RelationRule, SpaceType};
use cognigraph_embeddings::completion::CompletionProvider;

/// Version of the drafter prompts, recorded in `drafted_by`
/// attribution. No regression-harness rule attaches (D6): the drafter
/// holds no authority — a human reads every word before acceptance.
pub const DRAFT_REV: &str = "draft-policy-v1";

/// A drafted space type plus everything a reviewing human needs: what
/// the self-checks dropped (visible, never silent) and where the gate
/// advisor thinks the precision budget is needed (D4).
#[derive(Debug, Clone)]
pub struct DraftReport {
    pub space: SpaceType,
    /// Self-check drops and strips, human-readable.
    pub skips: Vec<String>,
    /// Deterministic gate-advisor annotations against the corpus.
    pub advisor: GateAdvisorReport,
    /// Chunks actually shown to the model (the self-checks and the
    /// advisor always run against the FULL corpus).
    pub sampled_chunks: usize,
}

/// Evenly spaced deterministic sample of at most `cap` chunks — the
/// model's context window is bounded; the symbolic checks are not.
fn sample(chunks: &[Chunk], cap: usize) -> Vec<&Chunk> {
    if chunks.len() <= cap {
        return chunks.iter().collect();
    }
    (0..cap).map(|i| &chunks[i * chunks.len() / cap]).collect()
}

fn excerpts(sampled: &[&Chunk]) -> String {
    sampled
        .iter()
        .map(|c| format!("[chunk {}] {}", c.id, c.text))
        .collect::<Vec<_>>()
        .join("\n---\n")
}

/// Draft a space type from a corpus. `space_id` becomes the draft's id;
/// existence checks against `space_types` are the caller's job (the
/// server route refuses existing ids — D3).
pub async fn draft_space_type(
    provider: &dyn CompletionProvider,
    space_id: &str,
    chunks: &[Chunk],
    sample_cap: usize,
) -> Result<DraftReport> {
    let sampled = sample(chunks, sample_cap.max(1));
    let corpus_cf: Vec<String> = chunks.iter().map(|c| c.text.to_lowercase()).collect();
    let occurs = |phrase: &str| -> bool {
        let phrase = phrase.to_lowercase();
        !phrase.trim().is_empty() && corpus_cf.iter().any(|text| text.contains(&phrase))
    };
    let mut skips: Vec<String> = Vec::new();

    // Stage 1: entities, every surface checked verbatim against the
    // FULL corpus.
    let stage1 = provider
        .complete_json(
            ENTITY_SYSTEM,
            &format!(
                "Document excerpts:\n{}\n\nDraft the entity catalogue.",
                excerpts(&sampled)
            ),
            &entity_schema(),
        )
        .await?;
    let mut entities: Vec<EntityDef> = Vec::new();
    for item in stage1["entities"].as_array().cloned().unwrap_or_default() {
        let name = item["name"].as_str().unwrap_or_default().trim().to_string();
        if name.is_empty() {
            continue;
        }
        if !occurs(&name) {
            skips.push(format!(
                "entity `{name}`: name does not occur verbatim in the corpus — dropped"
            ));
            continue;
        }
        if entities.iter().any(|e| e.name == name) {
            continue;
        }
        let mut aliases = Vec::new();
        for alias in item["aliases"].as_array().cloned().unwrap_or_default() {
            let alias = alias.as_str().unwrap_or_default().trim().to_string();
            if alias.is_empty() || alias == name || aliases.contains(&alias) {
                continue;
            }
            if occurs(&alias) {
                aliases.push(alias);
            } else {
                skips.push(format!(
                    "entity `{name}`: alias `{alias}` does not occur verbatim — dropped"
                ));
            }
        }
        entities.push(EntityDef {
            name,
            entity_type: item["type"].as_str().unwrap_or("entity").to_string(),
            aliases,
        });
    }

    // Stage 2: rules against the CHECKED catalogue — endpoint closure
    // plus verbatim-AND-affirmed triggers (negation-aware, full corpus).
    let catalogue: Vec<String> = entities
        .iter()
        .map(|e| {
            if e.aliases.is_empty() {
                e.name.clone()
            } else {
                format!("{} (aliases: {})", e.name, e.aliases.join(", "))
            }
        })
        .collect();
    let stage2 = provider
        .complete_json(
            RULE_SYSTEM,
            &format!(
                "Entity catalogue (the ONLY allowed endpoints):\n{}\n\n\
                 Document excerpts:\n{}\n\nDraft the relation rules.",
                catalogue.join("\n"),
                excerpts(&sampled)
            ),
            &rule_schema(),
        )
        .await?;
    let mut rules: Vec<RelationRule> = Vec::new();
    for item in stage2["rules"].as_array().cloned().unwrap_or_default() {
        let source = item["source"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        let relation = item["relation"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        let target = item["target"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string();
        let line = format!("{source} --{relation}--> {target}");
        if source.is_empty() || relation.is_empty() || target.is_empty() {
            continue;
        }
        let known = |name: &str| entities.iter().any(|e| e.name == name);
        if !known(&source) || !known(&target) {
            skips.push(format!(
                "rule `{line}`: endpoint not in the drafted entity catalogue — dropped (closure)"
            ));
            continue;
        }
        if source == target {
            skips.push(format!("rule `{line}`: self-loop — dropped"));
            continue;
        }
        if rules
            .iter()
            .any(|r| r.source == source && r.relation == relation && r.target == target)
        {
            continue;
        }
        let mut triggers = Vec::new();
        for phrase in item["when_any"].as_array().cloned().unwrap_or_default() {
            let phrase = phrase.as_str().unwrap_or_default().trim().to_lowercase();
            if phrase.is_empty() || triggers.contains(&phrase) {
                continue;
            }
            // D4: precision config is never LLM-authored.
            if phrase.contains("{source}") || phrase.contains("{target}") {
                skips.push(format!(
                    "rule `{line}`: template trigger `{phrase}` stripped — precision config \
                     is author-only (D4); see the advisor annotations"
                ));
                continue;
            }
            if chunks.iter().any(|c| affirms_phrase(&c.text, &phrase)) {
                triggers.push(phrase);
            } else {
                skips.push(format!(
                    "rule `{line}`: trigger `{phrase}` never occurs affirmed in the corpus \
                     — dropped"
                ));
            }
        }
        if triggers.is_empty() {
            skips.push(format!(
                "rule `{line}`: no trigger survived the verbatim-affirmed check — rule dropped"
            ));
            continue;
        }
        rules.push(RelationRule {
            source,
            relation,
            target,
            when_any: triggers,
            // D4: always naive; gates are applied by the reviewing
            // human, guided by the advisor report below.
            require_in_sentence: Vec::new(),
            trigger_provenance: Default::default(),
        });
    }

    let space = SpaceType {
        id: space_id.to_string(),
        name: space_id.to_string(),
        version: 1,
        description: format!("drafted by {DRAFT_REV} — review before acceptance"),
        entities,
        relation_rules: rules,
    };
    let advisor = advise_gates(&space, &[], chunks);
    Ok(DraftReport {
        space,
        skips,
        advisor,
        sampled_chunks: sampled.len(),
    })
}

/// Draft a space type by drafting each source document SEPARATELY and merging
/// the results. A single broad draft over a mixed corpus samples ~evenly across
/// every document, so an entity only prominent in one document (a drug's
/// indicated condition) is rarely shown to the model and never enters the
/// catalogue — starving the drug→condition rules that closure then drops. The
/// 2026-07-20 A/B measured this: broad interleaved drafting extracted 0 of 15
/// condition entities and 0 drug→condition rules; per-document drafting recovered
/// 15/15 (see decision_neurons_real_label_construction.md). Each document is
/// drafted densely against its OWN chunks (so every entity/trigger stays verbatim
/// self-checked within its source), then the catalogues and rules are unioned.
pub async fn draft_space_type_per_document(
    provider: &dyn CompletionProvider,
    space_id: &str,
    documents: &[Vec<Chunk>],
    sample_cap: usize,
) -> Result<DraftReport> {
    let mut space = empty_per_document_draft(space_id);
    let mut skips: Vec<String> = Vec::new();
    let mut sampled_chunks = 0usize;
    let mut all_chunks: Vec<Chunk> = Vec::new();

    for (index, doc) in documents.iter().enumerate() {
        if doc.is_empty() {
            continue;
        }
        let report = draft_space_type(provider, space_id, doc, sample_cap).await?;
        sampled_chunks += report.sampled_chunks;
        merge_drafted(&mut space, report.space, &mut skips);
        for skip in report.skips {
            skips.push(format!("[doc {index}] {skip}"));
        }
        all_chunks.extend(doc.iter().cloned());
    }

    // Only reachable here: each per-document draft self-checked against its own
    // chunks, so corpus-wide boilerplate is invisible until the documents are
    // held together.
    finalize_draft(&mut space, documents, &mut skips);

    // The advisor and all self-checks already ran per document against each
    // document's own chunks; re-run the advisor once over the union so its
    // corpus-wide annotations reflect the merged space.
    let advisor = advise_gates(&space, &[], &all_chunks);
    Ok(DraftReport {
        space,
        skips,
        advisor,
        sampled_chunks,
    })
}

/// The empty shell a per-document draft accumulates into. Exposed because the
/// durable `construct.draft` job drafts a BATCH of documents per pass and keeps
/// its accumulator in the stored draft between passes — it must start from
/// exactly the same shell (id, version, description) the in-process drafter
/// produces, or a resumed job would emit a differently-attributed artifact.
pub fn empty_per_document_draft(space_id: &str) -> SpaceType {
    SpaceType {
        id: space_id.to_string(),
        name: space_id.to_string(),
        version: 1,
        description: format!("drafted per-document by {DRAFT_REV} — review before acceptance"),
        entities: Vec::new(),
        relation_rules: Vec::new(),
    }
}

mod finalize;
mod prompts;
pub use finalize::{finalize_draft, merge_drafted};
use prompts::*;

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
