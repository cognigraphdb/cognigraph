//! Evidence grounding: negation-aware trigger matching ported faithfully
//! from the research (`_affirms_phrase` in entity_intelligence.py — the
//! Case Bravo lesson: a trigger inside a negated clause must not ground).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::types::{
    EndpointRef, EntityDef, Fact, Neuron, NeuronKind, NeuronStatus, RelationRule, SpaceType,
    TriggerProvenance,
};

/// True if `phrase` appears in `text` at least once without a preceding
/// negation cue in the same clause. Casefold substring match; the lookback
/// is bounded to the occurrence's own clause, so an earlier negated clause
/// does not suppress a later affirmative one.
pub fn affirms_phrase(text: &str, phrase: &str) -> bool {
    affirms_phrase_cf(&text.to_lowercase(), &phrase.to_lowercase())
}

pub(crate) fn sentence_bounds(text: &str, offset: usize) -> (usize, usize) {
    let at = offset.min(text.len());
    let start = text[..at]
        .char_indices()
        .rev()
        .find(|(i, c)| is_sentence_boundary(text, *i, *c))
        .map_or(0, |(i, _)| i + 1);
    let end = text[at..]
        .char_indices()
        .map(|(i, c)| (at + i, c))
        .find(|(i, c)| is_sentence_boundary(text, *i, *c))
        .map_or(text.len(), |(i, _)| i + 1);
    (start, end)
}

/// Apply accepted neurons to a space type: alias neurons append aliases,
/// relation-hint neurons append trigger phrases to matching rules (or add
/// the rule when the ontology has none for that triple). Additive and
/// order-independent, exactly like the research implementation.
pub fn effective_config(space: &SpaceType, neurons: &[Neuron]) -> SpaceType {
    let mut config = space.clone();
    let mut entity_by_name = HashMap::new();
    for (index, entity) in config.entities.iter().enumerate() {
        entity_by_name.entry(entity.name.clone()).or_insert(index);
    }
    let mut entity_aliases = config
        .entities
        .iter()
        .map(|entity| entity.aliases.iter().cloned().collect::<HashSet<_>>())
        .collect::<Vec<_>>();
    let mut rule_by_triple = HashMap::new();
    for (index, rule) in config.relation_rules.iter().enumerate() {
        rule_by_triple
            .entry((
                rule.source.clone(),
                rule.relation.clone(),
                rule.target.clone(),
            ))
            .or_insert(index);
    }
    let mut rule_triggers = config
        .relation_rules
        .iter()
        .map(|rule| rule.when_any.iter().cloned().collect::<HashSet<_>>())
        .collect::<Vec<_>>();
    for neuron in neurons {
        if neuron.status != NeuronStatus::Accepted {
            continue;
        }
        match neuron.kind {
            NeuronKind::Alias => {
                if let Some(&index) = entity_by_name.get(&neuron.entity) {
                    let entity = &mut config.entities[index];
                    for alias in &neuron.aliases {
                        if entity_aliases[index].insert(alias.clone()) {
                            entity.aliases.push(alias.clone());
                        }
                    }
                }
            }
            NeuronKind::RelationHint => {
                let triple = (
                    neuron.source.clone(),
                    neuron.relation.clone(),
                    neuron.target.clone(),
                );
                // Attribution is stamped PER TRIGGER, because a hint may extend
                // an authored rule: after this, one rule's `when_any` can mix
                // author-written triggers with neuron-added ones, and only the
                // trigger that actually matches licenses the fact.
                let provenance = TriggerProvenance {
                    neuron_id: neuron.id.clone(),
                    reviewed_by: neuron.reviewed_by.clone(),
                };
                match rule_by_triple.get(&triple).copied() {
                    Some(index) => {
                        let rule = &mut config.relation_rules[index];
                        for trigger in &neuron.triggers {
                            if rule_triggers[index].insert(trigger.clone()) {
                                rule.when_any.push(trigger.clone());
                            }
                            // Stamp even if the trigger already existed: an
                            // authored trigger a neuron also proposed is still
                            // authored, so do not overwrite an existing entry.
                            rule.trigger_provenance
                                .entry(trigger.clone())
                                .or_insert_with(|| provenance.clone());
                        }
                    }
                    None => {
                        let index = config.relation_rules.len();
                        config.relation_rules.push(RelationRule {
                            source: neuron.source.clone(),
                            relation: neuron.relation.clone(),
                            target: neuron.target.clone(),
                            when_any: neuron.triggers.clone(),
                            // A hint that CREATES a rule carries no gate; gates
                            // are authored ontology config (a hint extending an
                            // existing rule inherits that rule's gate).
                            require_in_sentence: Vec::new(),
                            trigger_provenance: neuron
                                .triggers
                                .iter()
                                .map(|trigger| (trigger.clone(), provenance.clone()))
                                .collect(),
                        });
                        rule_triggers.push(neuron.triggers.iter().cloned().collect());
                        rule_by_triple.insert(triple, index);
                    }
                }
            }
            // Rank hints reweight retrieval-trace ranking only (see
            // `rank::rank_boosts`); by design they cannot touch what gets
            // constructed. Blockers veto at grounding time (see
            // `effective_vetoes`); neither may alter the space type.
            NeuronKind::RelationRankHint | NeuronKind::RelationBlocker => {}
        }
    }
    config
}

/// An extraction-time veto for one triple: grounding is suppressed for any
/// chunk where a veto phrase is present.
#[derive(Debug, Clone, PartialEq)]
pub struct VetoRule {
    pub source: String,
    pub relation: String,
    pub target: String,
    pub when_any: Vec<String>,
}

/// Vetoes from ACCEPTED `relation_blocker` neurons — the same governance
/// boundary as every other neuron kind: proposed blockers are inert.
pub fn effective_vetoes(neurons: &[Neuron]) -> Vec<VetoRule> {
    neurons
        .iter()
        .filter(|n| n.kind == NeuronKind::RelationBlocker && n.status == NeuronStatus::Accepted)
        .map(|n| VetoRule {
            source: n.source.clone(),
            relation: n.relation.clone(),
            target: n.target.clone(),
            when_any: n.triggers.clone(),
        })
        .collect()
}

/// Entities (canonical names) mentioned in a chunk, via name or alias.
pub fn mentions<'a>(text: &str, entities: &'a [EntityDef]) -> Vec<&'a EntityDef> {
    let text_cf = text.to_lowercase();
    entities
        .iter()
        .filter(|entity| {
            std::iter::once(&entity.name)
                .chain(entity.aliases.iter())
                .any(|surface| text_cf.contains(&surface.to_lowercase()))
        })
        .collect()
}

/// A grounded fact with its licensing evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundedFact {
    pub fact: Fact,
    pub trigger: String,
    pub chunk_id: String,
    /// The neuron whose accepted trigger licensed this fact, and the actor who
    /// accepted it. BOTH are `None` for a fact licensed by an authored base-
    /// ontology trigger — that is the common case, and it means "the ontology
    /// author wrote this rule", not "nobody approved it".
    pub licensed_by_neuron: Option<String>,
    pub reviewed_by: Option<String>,
    /// Byte range in the ORIGINAL chunk text where the affirmed trigger
    /// occurrence sits — "look, right here", not "trust me". When the
    /// first occurrence is negated and a later one grounds, the span
    /// points at the later, licensing occurrence.
    pub trigger_span: (usize, usize),
}

/// Ground every configured rule whose trigger is affirmed in the chunk,
/// unless an accepted veto phrase for the same triple is present:
/// `(any trigger affirmed) AND NOT (any veto phrase present)`. Both sides
/// are commutative, so the config stays order-independent — a veto beats
/// a hint beats a base rule, unconditionally. Veto matching is plain
/// casefold presence, deliberately NOT negation-aware: an over-eager veto
/// costs recall on one chunk (the fact can ground elsewhere); an over-timid
/// one costs restraint, the asymmetrically expensive failure.
pub fn ground_chunk(
    chunk_id: &str,
    text: &str,
    config: &SpaceType,
    vetoes: &[VetoRule],
) -> Vec<GroundedFact> {
    let text_cf = text.to_lowercase();
    let negated_at = clause_negation_index(&text_cf);
    let entity_surfaces = indexed_entity_surfaces(config);
    let mut vetoes_by_triple = HashMap::<(&str, &str, &str), Vec<&VetoRule>>::new();
    for veto in vetoes {
        vetoes_by_triple
            .entry((
                veto.source.as_str(),
                veto.relation.as_str(),
                veto.target.as_str(),
            ))
            .or_default()
            .push(veto);
    }
    let sentence_ranges = if config
        .relation_rules
        .iter()
        .any(|rule| !rule.require_in_sentence.is_empty())
    {
        sentence_ranges_cf(&text_cf)
    } else {
        Vec::new()
    };
    config
        .relation_rules
        .iter()
        .filter_map(|rule| {
            let vetoed = vetoes_by_triple
                .get(&(
                    rule.source.as_str(),
                    rule.relation.as_str(),
                    rule.target.as_str(),
                ))
                .into_iter()
                .flatten()
                .any(|veto| {
                    veto.when_any.iter().any(|phrase| {
                        !phrase.trim().is_empty() && text_cf.contains(&phrase.to_lowercase())
                    })
                });
            if vetoed {
                return None;
            }
            // D1: sentence-scoped endpoint gate, checked per occurrence.
            let required: Vec<&[String]> = rule
                .require_in_sentence
                .iter()
                .map(|endpoint| match endpoint {
                    EndpointRef::Source => entity_surfaces
                        .get(rule.source.as_str())
                        .map_or(&[][..], |surfaces| surfaces.casefolded.as_slice()),
                    EndpointRef::Target => entity_surfaces
                        .get(rule.target.as_str())
                        .map_or(&[][..], |surfaces| surfaces.casefolded.as_slice()),
                })
                .collect();
            let sentence_gate_cache = RefCell::new(HashMap::new());
            let passes_gate = |cf_start: usize| {
                sentence_gate_accepts(
                    &text_cf,
                    &required,
                    &sentence_ranges,
                    &sentence_gate_cache,
                    cf_start,
                )
            };
            // D2: triggers may be templates; every expansion is a
            // candidate, in authored order. Plain triggers skip the
            // expansion machinery entirely (bench-guarded hot path).
            // `authored` is the phrase AS WRITTEN in the rule; `trigger` is what
            // actually matched (an expansion, for a template). Attribution is
            // keyed by the authored phrase — the expanded string is not in the
            // rule and would never be found in the provenance map.
            let (trigger, cf_span, authored) = rule.when_any.iter().find_map(|phrase| {
                if phrase.contains("{source}") || phrase.contains("{target}") {
                    find_expanded_trigger(
                        phrase,
                        rule,
                        &entity_surfaces,
                        &text_cf,
                        &negated_at,
                        &passes_gate,
                    )
                    .map(|(trigger, span)| (trigger, span, phrase.clone()))
                } else {
                    let phrase_cf = phrase.to_lowercase();
                    affirming_offset_where_indexed(&text_cf, &phrase_cf, &negated_at, passes_gate)
                        .map(|offset| {
                            (
                                phrase.clone(),
                                (offset, offset + phrase_cf.len()),
                                phrase.clone(),
                            )
                        })
                }
            })?;
            let attribution = rule.trigger_provenance.get(&authored);
            let trigger_span = (
                map_cf_offset(text, cf_span.0),
                map_cf_offset(text, cf_span.1),
            );
            Some(GroundedFact {
                fact: Fact {
                    source: rule.source.clone(),
                    relation: rule.relation.clone(),
                    target: rule.target.clone(),
                },
                trigger,
                chunk_id: chunk_id.to_string(),
                trigger_span,
                licensed_by_neuron: attribution.map(|a| a.neuron_id.clone()),
                reviewed_by: attribution.and_then(|a| a.reviewed_by.clone()),
            })
        })
        .collect()
}

mod negation;
mod semantics;
mod sentences;
mod triggers;
use negation::*;
pub use semantics::{SEMANTICS_REV, relation_semantics_signals};
use sentences::*;
use triggers::*;

#[cfg(test)]
#[path = "grounding_tests.rs"]
mod tests;
