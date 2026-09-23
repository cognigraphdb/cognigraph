//! Deterministic finalization of a drafted space type: merge, drop administrative relations, canonical identity, specific triggers.

use super::*;

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
pub(super) const ADMINISTRATIVE_ENTITY_TYPES: [&str; 27] = [
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
pub(super) fn drop_administrative_relations(
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
pub(super) fn canonicalize_entity_identity(space: &mut SpaceType, skips: &mut Vec<String>) {
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
pub(super) fn drop_non_specific_triggers(
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
pub(super) fn merge_entities(
    acc: &mut Vec<EntityDef>,
    incoming: Vec<EntityDef>,
    skips: &mut Vec<String>,
) {
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
pub(super) fn merge_rules(acc: &mut Vec<RelationRule>, incoming: Vec<RelationRule>) {
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
