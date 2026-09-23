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

const ENTITY_SYSTEM: &str = "You draft the ENTITY catalogue of a knowledge-graph ontology from \
document excerpts. Propose the distinct real-world entities the documents are about — \
organizations, people, products, platforms, standards, markets — each with a short lowercase \
type and the alternative surface forms (aliases) the text itself uses. STRICT RULES: every \
name and every alias must occur VERBATIM somewhere in the excerpts (they will be checked and \
dropped otherwise); do not invent canonical names the text never uses; do not include generic \
concepts that could not be an endpoint of a specific factual relation. Respond in the required \
JSON schema.";

const RULE_SYSTEM: &str = "You draft the RELATION RULES of a knowledge-graph ontology. You are \
given a fixed entity catalogue and document excerpts. Propose rules of the form `source \
--RELATION--> target` where source and target are entity NAMES from the catalogue (never \
anything else), RELATION is a short UPPER_SNAKE label of your choice, and `when_any` lists 1-3 \
short trigger phrases copied VERBATIM from sentences that explicitly assert that specific \
relation between those specific entities (they will be checked against the corpus and dropped \
otherwise — paraphrases do not survive). Only propose relations the text plainly asserts; a \
rule constructs a fact wherever a trigger appears affirmed, so a trigger that could appear in \
text about OTHER entities is a bad trigger. Respond in the required JSON schema.";

fn entity_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "entities": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "type": { "type": "string" },
                        "aliases": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["name", "type", "aliases"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["entities"],
        "additionalProperties": false
    })
}

fn rule_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "rules": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string" },
                        "relation": { "type": "string" },
                        "target": { "type": "string" },
                        "when_any": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["source", "relation", "target", "when_any"],
                    "additionalProperties": true
                }
            }
        },
        "required": ["rules"],
        "additionalProperties": false
    })
}

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

/// Merge one document's draft into an accumulating draft: entities unioned by
/// name (first-seen type kept, aliases accumulated, type conflicts surfaced in
/// `skips`), rules unioned by `(source, relation, target)` with their `when_any`
/// triggers accumulated.
///
/// Incremental by construction: merging documents one at a time in any order
/// yields the same accumulator as merging them all at once, which is what lets
/// the durable job checkpoint mid-corpus and resume from the stored draft.
pub fn merge_drafted(space: &mut SpaceType, incoming: SpaceType, skips: &mut Vec<String>) {
    merge_entities(&mut space.entities, incoming.entities, skips);
    merge_rules(&mut space.relation_rules, incoming.relation_rules);
}

/// Close a per-document draft over the FULL corpus: the cross-document trigger
/// specificity check (see [`drop_non_specific_triggers`]). Deliberately separate
/// from [`merge_drafted`] because it is not incremental — it needs every
/// document in hand, so the durable job runs it once on its final pass.
pub fn finalize_draft(space: &mut SpaceType, documents: &[Vec<Chunk>], skips: &mut Vec<String>) {
    canonicalize_entity_identity(space, skips);
    drop_administrative_relations(&mut space.relation_rules, &space.entities, skips);
    drop_non_specific_triggers(&mut space.relation_rules, &space.entities, documents, skips);
}

/// Entity types that denote an ACTOR, a CHANNEL, a CODE, or a DOCUMENT ARTIFACT
/// rather than something the corpus is factually about
/// (decision_pilot_clinical_graph.md, D1).
///
/// Matched EXACTLY, never as substrings: on real labels the drafter emits
/// `anatomical site` and `parasite`, both of which a substring match on "site"
/// would silently destroy. An exact list under-matches on model-invented
/// synonyms, which is the safe direction — an unlisted type leaves an
/// administrative rule standing, it never drops a clinical one.
///
/// Deliberately EXCLUDED even though they look administrative: `person` holds
/// the patient populations (`pregnant woman`, `Elderly patients`, `children`)
/// that `CONTRAINDICATED_IN` needs, and `clinical trial`, `device`,
/// `measurement` and `classification` are all clinical subjects.
const ADMINISTRATIVE_ENTITY_TYPES: [&str; 27] = [
    "organization",
    "company",
    "corporation",
    "manufacturer",
    "distributor",
    "sponsor",
    "agency",
    "regulatory agency",
    "government agency",
    "website",
    "web site",
    "url",
    "platform",
    "database",
    "registry",
    "portal",
    "document",
    "section",
    "label section",
    "document section",
    "publication",
    "standard",
    "identifier",
    "marking",
    "code",
    "phone number",
    "program",
];

/// Drop rules whose subject or object is administrative furniture rather than a
/// clinical subject — the single highest-leverage precision fix measured on the
/// 100-label pilot.
///
/// The drafter proposes relations for regulatory scaffolding as readily as for
/// pharmacology: `drug --REPORT_ADVERSE_REACTIONS_TO--> label manufacturer`,
/// `Amnesteem --APPROVED_BY--> Food and Drug Administration`,
/// `Warnings and Precautions --CONTAINS_INFO_ON--> …`. On the pilot these were
/// 11% of the built graph at **19.4%** precision by relation name — and only
/// **4.5%** when identified by endpoint type, which is why the check keys on the
/// ontology's own types rather than on relation labels. Relation names are an
/// unbounded invented vocabulary (366 types for 1746 facts); entity types are
/// the smaller, structured axis the drafter already commits to.
///
/// Measured on the pilot sample: precision 71.5% → 83.4% while keeping 84% of
/// the facts, and the only correct facts lost were NDC-code edges.
fn drop_administrative_relations(
    rules: &mut Vec<RelationRule>,
    entities: &[EntityDef],
    skips: &mut Vec<String>,
) {
    use std::collections::HashSet;

    let administrative: HashSet<&str> = entities
        .iter()
        .filter(|entity| {
            let entity_type = entity.entity_type.trim().to_lowercase();
            ADMINISTRATIVE_ENTITY_TYPES.contains(&entity_type.as_str())
        })
        .map(|entity| entity.name.as_str())
        .collect();
    if administrative.is_empty() {
        return;
    }
    rules.retain(|rule| {
        let offender = [&rule.source, &rule.target]
            .into_iter()
            .find(|name| administrative.contains(name.as_str()));
        match offender {
            Some(name) => {
                skips.push(format!(
                    "rule `{} --{}--> {}`: `{name}` is administrative (not a clinical subject) — dropped",
                    rule.source, rule.relation, rule.target
                ));
                false
            }
            None => true,
        }
    });
}

/// Collapse entities that are distinct names but the SAME graph identity.
///
/// `merge_drafted` unions entities by their literal name, but the graph keys an
/// entity on [`entity_key`] (case-folded, punctuation-collapsed). Independent
/// per-document drafts routinely capitalize the same entity differently —
/// `Epinephrine` in one label, `epinephrine` in another — and those are one
/// entity to the store. `ingest_chunks` fails closed on the collision rather
/// than silently merging two identities, so an un-collapsed draft cannot be
/// ingested at all: on 100 real FDA labels this produced 63 colliding groups.
///
/// Rules name their endpoints by the entity's literal name and grounding looks
/// them up by exact string, so dropping a variant WITHOUT rewriting the rules
/// that reference it would silently orphan those rules — every merge therefore
/// rewrites the rules too, and rules that collide onto one triple afterwards are
/// folded back together with their triggers unioned. Every collapse is recorded
/// in `skips`, and a type disagreement between variants is reported the same way
/// `merge_entities` reports a same-name one: the human resolves it before
/// acceptance (decision_neurons_real_label_construction.md, D3).
fn canonicalize_entity_identity(space: &mut SpaceType, skips: &mut Vec<String>) {
    use std::collections::HashMap;

    let mut canonical: HashMap<String, usize> = HashMap::new();
    let mut kept: Vec<EntityDef> = Vec::new();
    let mut renamed: HashMap<String, String> = HashMap::new();

    for entity in std::mem::take(&mut space.entities) {
        let key = entity_key(&entity.name);
        if let Some(&index) = canonical.get(&key) {
            let keeper: &mut EntityDef = &mut kept[index];
            if keeper.entity_type == entity.entity_type {
                skips.push(format!(
                    "entity `{}` merged into `{}` — same graph identity `{}`",
                    entity.name, keeper.name, key
                ));
            } else {
                skips.push(format!(
                    "entity `{}` (`{}`) and `{}` (`{}`) share graph identity `{}` — kept `{}` as `{}` (review)",
                    keeper.name,
                    keeper.entity_type,
                    entity.name,
                    entity.entity_type,
                    key,
                    keeper.name,
                    keeper.entity_type
                ));
            }
            let keeper_name = keeper.name.clone();
            for alias in entity.aliases {
                if !keeper.aliases.contains(&alias) {
                    keeper.aliases.push(alias);
                }
            }
            renamed.insert(entity.name, keeper_name);
        } else {
            canonical.insert(key, kept.len());
            kept.push(entity);
        }
    }
    space.entities = kept;

    if renamed.is_empty() {
        return;
    }
    for rule in &mut space.relation_rules {
        if let Some(name) = renamed.get(&rule.source) {
            rule.source = name.clone();
        }
        if let Some(name) = renamed.get(&rule.target) {
            rule.target = name.clone();
        }
    }
    let rewritten = std::mem::take(&mut space.relation_rules);
    merge_rules(&mut space.relation_rules, rewritten);
}

/// Cross-document trigger specificity. The rule prompt already tells the model
/// that "a trigger that could appear in text about OTHER entities is a bad
/// trigger" — per-document drafting is the first place we hold the WHOLE corpus
/// and can enforce it symbolically. A trigger that fires inside a document which
/// never names the rule's source is boilerplate (a regulatory footer, a template
/// sentence), not evidence for this triple: on real FDA labels the MedWatch line
/// "contact fda at 1-800-fda-1088" licensed `MAXALT --CONTACT--> FDA` 29 times,
/// first evidenced from an unrelated atropine label.
///
/// Deliberately fixed at DRAFT time. Grounding stays chunk-local and the advisor
/// keeps its "flag, don't silently pick a side" contract
/// (decision_cross_chunk_grounding.md): a fact whose endpoint is merely absent
/// from the licensing *sentence* may still be legitimate cross-sentence evidence,
/// so the grounder must not drop it — but a trigger that is corpus-wide
/// boilerplate should never have become a rule in the first place.
fn drop_non_specific_triggers(
    rules: &mut Vec<RelationRule>,
    entities: &[EntityDef],
    documents: &[Vec<Chunk>],
    skips: &mut Vec<String>,
) {
    // Document-level scope (not chunk): a label that names its drug in the title
    // section still legitimately licenses triggers in later sections.
    let doc_text_cf: Vec<String> = documents
        .iter()
        .map(|doc| {
            doc.iter()
                .map(|chunk| chunk.text.to_lowercase())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect();

    for rule in rules.iter_mut() {
        let source = rule.source.clone();
        let line = format!("{} --{}--> {}", rule.source, rule.relation, rule.target);
        let surfaces: Vec<String> = entities
            .iter()
            .filter(|entity| entity.name == source)
            .flat_map(|entity| {
                std::iter::once(entity.name.to_lowercase())
                    .chain(entity.aliases.iter().map(|alias| alias.to_lowercase()))
            })
            .filter(|surface| !surface.is_empty())
            .collect();
        let names_source: Vec<bool> = doc_text_cf
            .iter()
            .map(|text| {
                surfaces
                    .iter()
                    .any(|surface| text.contains(surface.as_str()))
            })
            .collect();

        rule.when_any.retain(|trigger| {
            let fires_in_foreign_document = documents.iter().enumerate().any(|(index, doc)| {
                !names_source[index] && doc.iter().any(|chunk| affirms_phrase(&chunk.text, trigger))
            });
            if fires_in_foreign_document {
                skips.push(format!(
                    "rule `{line}`: trigger `{trigger}` also fires in documents that never name \
                     `{source}` — corpus boilerplate, not specific to this triple; dropped"
                ));
            }
            !fires_in_foreign_document
        });
    }

    rules.retain(|rule| {
        if rule.when_any.is_empty() {
            skips.push(format!(
                "rule `{} --{}--> {}`: no trigger survived the cross-document specificity check \
                 — rule dropped",
                rule.source, rule.relation, rule.target
            ));
            false
        } else {
            true
        }
    });
}

/// Union entities by name: same name keeps its first-seen type and accumulates
/// aliases. A type disagreement across documents is surfaced (not silently
/// resolved) — entities are globally name-keyed, so the reviewer decides.
fn merge_entities(acc: &mut Vec<EntityDef>, incoming: Vec<EntityDef>, skips: &mut Vec<String>) {
    for entity in incoming {
        if let Some(existing) = acc.iter_mut().find(|e| e.name == entity.name) {
            if existing.entity_type != entity.entity_type {
                skips.push(format!(
                    "entity `{}`: type `{}` and `{}` proposed across documents — kept `{}` (review)",
                    entity.name, existing.entity_type, entity.entity_type, existing.entity_type
                ));
            }
            for alias in entity.aliases {
                if !existing.aliases.contains(&alias) {
                    existing.aliases.push(alias);
                }
            }
        } else {
            acc.push(entity);
        }
    }
}

/// Union rules by (source, relation, target), accumulating `when_any` triggers.
fn merge_rules(acc: &mut Vec<RelationRule>, incoming: Vec<RelationRule>) {
    for rule in incoming {
        if let Some(existing) = acc.iter_mut().find(|r| {
            r.source == rule.source && r.relation == rule.relation && r.target == rule.target
        }) {
            for trigger in rule.when_any {
                if !existing.when_any.contains(&trigger) {
                    existing.when_any.push(trigger);
                }
            }
        } else {
            acc.push(rule);
        }
    }
}

#[cfg(test)]
#[path = "draft_tests.rs"]
mod tests;
