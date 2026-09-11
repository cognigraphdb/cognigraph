//! Evidence grounding: negation-aware trigger matching ported faithfully
//! from the research (`_affirms_phrase` in entity_intelligence.py — the
//! Case Bravo lesson: a trigger inside a negated clause must not ground).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use crate::types::{
    EndpointRef, EntityDef, Fact, Neuron, NeuronKind, NeuronStatus, RelationRule, SpaceType,
    TriggerProvenance,
};

const NEGATION_CUES: &[&str] = &[
    "not",
    "no",
    "never",
    "none",
    "nor",
    "false",
    "untrue",
    "without",
    "denied",
    "denies",
    "deny",
    "retracted",
    "withdrawn",
    "cannot",
];

const CLAUSE_BOUNDARIES: &[char] = &['.', ';', '!', '?', ',', ':'];

/// True if `phrase` appears in `text` at least once without a preceding
/// negation cue in the same clause. Casefold substring match; the lookback
/// is bounded to the occurrence's own clause, so an earlier negated clause
/// does not suppress a later affirmative one.
pub fn affirms_phrase(text: &str, phrase: &str) -> bool {
    affirms_phrase_cf(&text.to_lowercase(), &phrase.to_lowercase())
}

/// Casefolded fast path: `ground_chunk` folds the chunk text once and
/// reuses it across every rule trigger and veto phrase (measured 2.5x on
/// the grounding benchmark — see docs/benchmarks.md).
fn affirms_phrase_cf(text_cf: &str, phrase_cf: &str) -> bool {
    affirming_offset_cf(text_cf, phrase_cf).is_some()
}

/// Byte offset (in the CASEFOLDED text) of the first occurrence of
/// `phrase_cf` that is not negated within its clause — the occurrence
/// that licenses grounding. `map_cf_offset` translates it back to the
/// original text for provenance.
fn affirming_offset_cf(text_cf: &str, phrase_cf: &str) -> Option<usize> {
    affirming_offset_where(text_cf, phrase_cf, |_| true)
}

/// Like `affirming_offset_cf`, but the occurrence must also satisfy
/// `accept` (the sentence-scoped endpoint gate) — a later occurrence in a
/// satisfying sentence still grounds when an earlier one fails the gate,
/// mirroring how negation lookback is per-occurrence.
fn affirming_offset_where(
    text_cf: &str,
    phrase_cf: &str,
    accept: impl Fn(usize) -> bool,
) -> Option<usize> {
    let negated_at = clause_negation_index(text_cf);
    affirming_offset_where_indexed(text_cf, phrase_cf, &negated_at, accept)
}

fn affirming_offset_where_indexed(
    text_cf: &str,
    phrase_cf: &str,
    negated_at: &[bool],
    accept: impl Fn(usize) -> bool,
) -> Option<usize> {
    if phrase_cf.is_empty() {
        return None;
    }
    let mut start = 0;
    while let Some(offset) = text_cf[start..].find(phrase_cf) {
        let idx = start + offset;
        if !negated_at.get(idx).copied().unwrap_or(true) && accept(idx) {
            return Some(idx);
        }
        // Advance one character (respecting UTF-8 boundaries).
        start = idx + text_cf[idx..].chars().next().map_or(1, char::len_utf8);
        if start >= text_cf.len() {
            break;
        }
    }
    None
}

/// Map a byte offset in `text.to_lowercase()` back to a byte offset in
/// `text`. Casefolding can change byte lengths (ß → ss, İ → i̇), so the
/// two coordinate systems diverge on non-ASCII text; this walks both in
/// lockstep. An offset falling INSIDE one character's folded expansion
/// resolves to that character's start. Offsets past the end clamp to
/// `text.len()`.
fn map_cf_offset(text: &str, cf_offset: usize) -> usize {
    let mut folded = 0usize;
    for (original, c) in text.char_indices() {
        if folded >= cf_offset {
            return original;
        }
        folded += c.to_lowercase().map(char::len_utf8).sum::<usize>();
        if folded > cf_offset {
            return original; // inside this char's folded expansion
        }
    }
    text.len()
}

/// Abbreviations whose trailing dot is NOT a sentence end.
///
/// Deliberately short and conservative, because the two mistakes are not
/// symmetric: failing to split costs restraint (a `require_in_sentence` gate
/// then judges a longer span and admits more groundings), while splitting too
/// eagerly only truncates a sentence. So this lists only tokens that are
/// essentially never sentence-final in label prose, and pointedly EXCLUDES ones
/// that often are — `etc.`, and the corporate suffixes `Inc.`/`Ltd.`/`Corp.`,
/// which routinely end a sentence in an FDA label's manufacturer line.
///
/// Every entry is drawn from a measured false flag on the 100-label pilot, not
/// from a general-purpose abbreviation list (decision_pilot_clinical_graph.md).
const NON_TERMINAL_ABBREVIATIONS: [&str; 12] = [
    "st", // "St. John's Wort" — the case that motivated this
    "no", // "D C Yellow No. 10 Aluminum Lake"
    "dr", "mr", "mrs", "ms", "prof", // titles
    "vs", "cf", "al", // "et al."
    "fig", "approx",
];

/// The alphabetic token immediately preceding a dot, if any.
fn word_before_dot(text: &str, dot: usize) -> &str {
    let head = &text[..dot];
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_alphabetic())
        .map_or(0, |(i, c)| i + c.len_utf8());
    &head[start..]
}

/// Sentence bounds (byte range in the original text) around an offset.
/// Sentence boundaries are deliberately COARSER than the negation clause
/// boundaries (no ','/':') — the gate asks "is this sentence about the
/// endpoint", not "is this clause negated". A '.' ends a sentence only
/// when followed by whitespace or end-of-text: a dot flanked by other
/// characters is part of a token — a decimal ("$3.09B", found by the
/// gate advisor) or a dotted name ("Chorus.ai", "OpenProtein.AI",
/// found by the chunk-sensitivity simulation: mid-name splits were the
/// ENTIRE measured cost of adversarial re-chunking, and they truncate
/// the sentence a gate judges).
///
/// A dot closing a known abbreviation is likewise not a boundary. Real FDA
/// labels name entities that CONTAIN one — `St. John's Wort`, `D C Yellow No. 10
/// Aluminum Lake` — and splitting there truncates the sentence mid-entity-name,
/// which made correct facts look target-absent to the relation-semantics
/// detector (6 false flags per 200 facts, all of them correct facts).
fn is_sentence_boundary(text: &str, i: usize, c: char) -> bool {
    const BOUNDS: &[char] = &['.', '!', '?', '\n'];
    if !BOUNDS.contains(&c) {
        return false;
    }
    if c == '.' {
        let next = text[i + c.len_utf8()..].chars().next();
        if next.is_some_and(|n| !n.is_whitespace()) {
            return false;
        }
        let word = word_before_dot(text, i);
        if !word.is_empty()
            && NON_TERMINAL_ABBREVIATIONS
                .iter()
                .any(|abbrev| word.eq_ignore_ascii_case(abbrev))
        {
            return false;
        }
    }
    true
}

/// Revision of the relation-semantics signal set, stamped on every stored
/// verdict so a consumer can tell which detector produced it (and re-run when
/// it moves). The detector is advisory and expected to evolve; the governed
/// fact record deliberately does not carry it
/// (decision_pilot_clinical_graph.md, D2).
pub const SEMANTICS_REV: &str = "semantics-v1";

/// Words carrying no relation meaning, so their absence says nothing.
const RELATION_STOPWORDS: [&str; 21] = [
    "of", "to", "in", "with", "for", "by", "on", "at", "as", "the", "a", "an", "is", "are", "has",
    "have", "be", "and", "or", "from", "into",
];

/// Crude prefix stem, enough to survive inflection (`treats`/`treatment`,
/// `causes`/`caused`, `indicated`/`indications`) without a stemmer dependency.
fn stem(word: &str) -> &str {
    let keep = word.len().saturating_sub(3).max(4).min(word.len());
    let mut end = keep;
    while end > 0 && !word.is_char_boundary(end) {
        end -= 1;
    }
    &word[..end]
}

fn entity_surfaces<'a>(name: &'a str, entities: &'a [EntityDef]) -> Vec<String> {
    let mut out = vec![name.to_lowercase()];
    for entity in entities.iter().filter(|e| e.name == name) {
        out.extend(entity.aliases.iter().map(|a| a.to_lowercase()));
    }
    out.retain(|s| !s.trim().is_empty());
    out
}

fn names_entity(name: &str, entities: &[EntityDef], sentence_cf: &str) -> bool {
    entity_surfaces(name, entities)
        .iter()
        .any(|surface| sentence_cf.contains(surface.as_str()))
}

/// Source and target sit adjacent in a coordinated list with nothing but a
/// separator between them: the sentence ENUMERATES the two, it does not relate
/// them. `myopathy and rhabdomyolysis` licensed `myopathy --CAUSES-->
/// rhabdomyolysis`; `on moles, birthmarks, warts` licensed `common warts
/// --LOCATED_ON--> moles`.
fn merely_co_listed(source: &str, target: &str, entities: &[EntityDef], sentence_cf: &str) -> bool {
    for a in entity_surfaces(source, entities) {
        for b in entity_surfaces(target, entities) {
            for (first, second) in [(&a, &b), (&b, &a)] {
                let Some(i) = sentence_cf.find(first.as_str()) else {
                    continue;
                };
                let after = i + first.len();
                let Some(offset) = sentence_cf[after..].find(second.as_str()) else {
                    continue;
                };
                let between = sentence_cf[after..after + offset]
                    .trim_matches(|c: char| c.is_whitespace() || c == ',' || c == ';');
                if between.is_empty() || between == "and" || between == "or" || between == "and/or"
                {
                    return true;
                }
            }
        }
    }
    false
}

/// No content word of the relation's own name appears in the licensing
/// sentence — the sentence never uses the vocabulary the relation claims.
/// `atorvastatin --TREATS--> MI` off "indicated to reduce the risk of MI":
/// the sentence is about PREVENTION and never says treat.
fn relation_vocabulary_absent(relation: &str, sentence_cf: &str) -> bool {
    let mut any = false;
    for token in relation
        .split(|c: char| c == '_' || c.is_whitespace())
        .map(str::to_lowercase)
        .filter(|t| !t.is_empty() && !RELATION_STOPWORDS.contains(&t.as_str()))
    {
        any = true;
        if sentence_cf.contains(stem(&token)) {
            return false;
        }
    }
    any
}

/// Signals that the licensing sentence does not ASSERT this triple, even though
/// the trigger fired verbatim and affirmed inside it.
///
/// A pure function of the fact triple, the governed entity catalogue, and the
/// sentence — it never needs the rule, so grounding, ingest and the advisor all
/// share this one implementation.
///
/// Every signal was chosen by measurement before shipping
/// (decision_pilot_clinical_graph.md, D2): designed against 200 judged facts
/// from one ontology and confirmed on 200 from a second, independent one.
/// Candidates that looked good on the design set and did NOT survive the
/// holdout were dropped — notably "source absent from the sentence", which is
/// ANTI-correlated with error (a label section names its drug once and refers
/// to it implicitly thereafter, so source absence is ordinary prose), and
/// "section-index shaped". That asymmetry is why the advisor's endpoint-presence
/// checks, which treat source and target alike, do not separate good facts from
/// bad on a real corpus.
pub fn relation_semantics_signals(
    source: &str,
    relation: &str,
    target: &str,
    entities: &[EntityDef],
    sentence: &str,
) -> Vec<String> {
    let sentence_cf = sentence.to_lowercase();
    let mut signals = Vec::new();
    if !names_entity(target, entities, &sentence_cf) {
        signals.push("target absent from the licensing sentence".to_string());
    }
    if merely_co_listed(source, target, entities, &sentence_cf) {
        signals.push("endpoints merely co-listed, not related".to_string());
    }
    if relation_vocabulary_absent(relation, &sentence_cf) {
        signals.push("sentence never uses the relation's vocabulary".to_string());
    }
    signals
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

/// Sentence ranges in casefolded coordinates, computed once per chunk. Sentence
/// punctuation is stable under lowercasing, so these ranges match
/// `sentence_bounds` while avoiding a full before/after scan per occurrence.
fn sentence_ranges_cf(text_cf: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for (i, c) in text_cf.char_indices() {
        if is_sentence_boundary(text_cf, i, c) {
            let end = i + c.len_utf8();
            ranges.push((start, end));
            start = end;
        }
    }
    if start < text_cf.len() {
        ranges.push((start, text_cf.len()));
    }
    ranges
}

struct EntitySurfaces<'a> {
    raw: Vec<&'a str>,
    casefolded: Vec<String>,
}

fn indexed_entity_surfaces(config: &SpaceType) -> HashMap<&str, EntitySurfaces<'_>> {
    let mut indexed = HashMap::new();
    for entity in &config.entities {
        let surfaces = indexed
            .entry(entity.name.as_str())
            .or_insert_with(|| EntitySurfaces {
                raw: Vec::new(),
                casefolded: Vec::new(),
            });
        for surface in
            std::iter::once(entity.name.as_str()).chain(entity.aliases.iter().map(String::as_str))
        {
            surfaces.raw.push(surface);
            surfaces.casefolded.push(surface.to_lowercase());
        }
    }
    indexed
}

/// Expand a trigger's `{source}`/`{target}` placeholders against the
/// endpoints' surfaces (as authored, one expansion per combination), in
/// deterministic order. Plain triggers pass through unchanged. The
/// expansion is what actually matches — and what gets recorded as the
/// grounded trigger, so provenance stays verbatim.
fn find_expanded_trigger(
    phrase: &str,
    rule: &RelationRule,
    surfaces: &HashMap<&str, EntitySurfaces<'_>>,
    text_cf: &str,
    negated_at: &[bool],
    accept: &impl Fn(usize) -> bool,
) -> Option<(String, (usize, usize))> {
    let needs_source = phrase.contains("{source}");
    let needs_target = phrase.contains("{target}");
    debug_assert!(needs_source || needs_target);
    let source_options: Vec<Option<&str>> = if needs_source {
        surfaces
            .get(rule.source.as_str())
            .into_iter()
            .flat_map(|surfaces| surfaces.raw.iter().copied())
            .map(Some)
            .collect()
    } else {
        vec![None]
    };
    let target_options: Vec<Option<&str>> = if needs_target {
        surfaces
            .get(rule.target.as_str())
            .into_iter()
            .flat_map(|surfaces| surfaces.raw.iter().copied())
            .map(Some)
            .collect()
    } else {
        vec![None]
    };
    for source in &source_options {
        for target in &target_options {
            let expanded = substitute_authored_placeholders(phrase, *source, *target);
            let expanded_cf = expanded.to_lowercase();
            if let Some(offset) =
                affirming_offset_where_indexed(text_cf, &expanded_cf, negated_at, accept)
            {
                return Some((expanded, (offset, offset + expanded_cf.len())));
            }
        }
    }
    None
}

/// Substitute placeholders found in the authored trigger exactly once.
///
/// Surface text is always literal: a `{target}` token inside a source alias is
/// not interpreted during the target substitution pass. Besides preventing
/// surprising matches, this keeps expansion work proportional to the authored
/// template and selected surfaces.
fn substitute_authored_placeholders(
    phrase: &str,
    source: Option<&str>,
    target: Option<&str>,
) -> String {
    let mut expanded = String::with_capacity(phrase.len());
    let mut cursor = 0;
    while cursor < phrase.len() {
        let remaining = &phrase[cursor..];
        if remaining.starts_with("{source}") {
            expanded.push_str(source.unwrap_or("{source}"));
            cursor += "{source}".len();
        } else if remaining.starts_with("{target}") {
            expanded.push_str(target.unwrap_or("{target}"));
            cursor += "{target}".len();
        } else {
            let character = remaining
                .chars()
                .next()
                .expect("cursor is before the end of the trigger");
            expanded.push(character);
            cursor += character.len_utf8();
        }
    }
    expanded
}

fn sentence_gate_accepts(
    text_cf: &str,
    required: &[&[String]],
    sentence_ranges: &[(usize, usize)],
    cache: &RefCell<HashMap<usize, bool>>,
    cf_start: usize,
) -> bool {
    if required.is_empty() {
        return true;
    }
    let sentence_index = sentence_ranges.partition_point(|(_, end)| *end <= cf_start);
    let Some(&(start, end)) = sentence_ranges.get(sentence_index) else {
        return false;
    };
    if cf_start < start {
        return false;
    }
    if let Some(accepted) = cache.borrow().get(&sentence_index).copied() {
        return accepted;
    }
    let accepted = required.iter().all(|surfaces| {
        surfaces
            .iter()
            .any(|surface| text_cf[start..end].contains(surface))
    });
    cache.borrow_mut().insert(sentence_index, accepted);
    accepted
}

fn token_is_negation(token: &str) -> bool {
    !token.is_empty() && (NEGATION_CUES.contains(&token) || token.ends_with("n't"))
}

/// Whether each UTF-8 byte boundary has a negation cue earlier in its current
/// clause. This exactly preserves the prefix semantics of `negated_before`
/// while replacing repeated reverse clause scans with one linear chunk pass.
fn clause_negation_index(text_cf: &str) -> Vec<bool> {
    let mut negated_at = vec![false; text_cf.len() + 1];
    let mut completed_negation = false;
    let mut token_start = None;
    for (index, character) in text_cf.char_indices() {
        let partial_negation = token_start
            .map(|start| token_is_negation(&text_cf[start..index]))
            .unwrap_or(false);
        negated_at[index] = completed_negation || partial_negation;

        if CLAUSE_BOUNDARIES.contains(&character) {
            completed_negation = false;
            token_start = None;
        } else if character.is_ascii_alphanumeric() || character == '\'' {
            token_start.get_or_insert(index);
        } else if let Some(start) = token_start.take()
            && token_is_negation(&text_cf[start..index])
        {
            completed_negation = true;
        }
    }
    let partial_negation = token_start
        .map(|start| token_is_negation(&text_cf[start..]))
        .unwrap_or(false);
    negated_at[text_cf.len()] = completed_negation || partial_negation;
    negated_at
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentence_gate_blocks_wrong_subject_chunks() {
        let space: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [
                {"name": "Harbor", "type": "org", "aliases": []},
                {"name": "Northstar", "type": "org", "aliases": ["NS"]},
                {"name": "Stack+", "type": "platform", "aliases": []}
            ],
            "relation_rules": [
                {"source": "Harbor", "relation": "SELECTED", "target": "Stack+",
                 "when_any": ["selected stack+"],
                 "require_in_sentence": ["source"]}
            ]
        }))
        .unwrap();

        // The leakage case: trigger affirmed in a sentence about ANOTHER
        // company — the gate refuses even though the chunk mentions Harbor.
        let leak = "Harbor is discussed elsewhere in this report. \
                    Northstar selected Stack+ for its commercial stack.";
        assert!(ground_chunk("c1", leak, &space, &[]).is_empty());

        // The legitimate case grounds, and the span sits in the right
        // sentence.
        let legit = "Northstar went another way. Harbor selected Stack+ last spring.";
        let grounded = ground_chunk("c2", legit, &space, &[]);
        assert_eq!(grounded.len(), 1);
        let (start, end) = grounded[0].trigger_span;
        assert_eq!(&legit[start..end], "selected Stack+");
        assert!(start > 28, "must be the second sentence's occurrence");

        // Per-occurrence: a first occurrence in a wrong-subject sentence
        // does not suppress a later one in a satisfying sentence.
        let both = "Northstar selected Stack+ in 2024. Later, Harbor selected Stack+ too.";
        let grounded = ground_chunk("c3", both, &space, &[]);
        assert_eq!(grounded.len(), 1);
        assert!(grounded[0].trigger_span.0 > 34);

        // Aliases count as surfaces for the gate.
        let space_gated_alias: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [
                {"name": "Northstar", "type": "org", "aliases": ["NS"]},
                {"name": "Stack+", "type": "platform", "aliases": []}
            ],
            "relation_rules": [
                {"source": "Northstar", "relation": "SELECTED", "target": "Stack+",
                 "when_any": ["selected stack+"],
                 "require_in_sentence": ["source"]}
            ]
        }))
        .unwrap();
        let via_alias = "NS selected Stack+ after a long evaluation.";
        assert_eq!(
            ground_chunk("c4", via_alias, &space_gated_alias, &[]).len(),
            1
        );

        // A typo'd gate value fails loudly at deserialization.
        let bad = serde_json::from_value::<SpaceType>(serde_json::json!({
            "id": "s", "entities": [],
            "relation_rules": [{"source": "A", "relation": "R", "target": "B",
                                "when_any": [], "require_in_sentence": ["sorce"]}]
        }));
        assert!(bad.is_err());
    }

    #[test]
    fn template_triggers_are_direction_faithful() {
        let space: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [
                {"name": "CedarWorks", "type": "org", "aliases": []},
                {"name": "MapleSystems", "type": "org", "aliases": ["MapleSystems Inc"]}
            ],
            "relation_rules": [
                {"source": "CedarWorks", "relation": "ACQUIRED", "target": "MapleSystems",
                 "when_any": ["{source} acquired {target}",
                              "{target} was acquired by {source}"]}
            ]
        }))
        .unwrap();

        // Correct direction grounds — via either phrasing, alias included.
        let forward = "In May, CedarWorks acquired MapleSystems for an undisclosed sum.";
        let grounded = ground_chunk("c1", forward, &space, &[]);
        assert_eq!(grounded.len(), 1);
        assert_eq!(grounded[0].trigger, "CedarWorks acquired MapleSystems");
        let (start, end) = grounded[0].trigger_span;
        assert_eq!(&forward[start..end], "CedarWorks acquired MapleSystems");

        let passive = "MapleSystems Inc was acquired by CedarWorks.";
        let grounded = ground_chunk("c2", passive, &space, &[]);
        assert_eq!(grounded.len(), 1);
        assert_eq!(
            grounded[0].trigger,
            "MapleSystems Inc was acquired by CedarWorks"
        );

        // The REVERSED phrasing does not match any expansion: the rule
        // whose direction contradicts the text stays silent.
        let reversed = "MapleSystems acquired CedarWorks, sources claimed.";
        assert!(ground_chunk("c3", reversed, &space, &[]).is_empty());

        // Negation awareness applies to expansions like any trigger.
        let negated = "It is not true that CedarWorks acquired MapleSystems.";
        assert!(ground_chunk("c4", negated, &space, &[]).is_empty());
    }

    #[test]
    fn template_surfaces_are_substituted_literally_once() {
        let space: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [
                {"name": "Source", "type": "org", "aliases": ["{target}"]},
                {"name": "Target", "type": "org", "aliases": ["Widget"]}
            ],
            "relation_rules": [
                {"source": "Source", "relation": "SUPPLIES", "target": "Target",
                 "when_any": ["{source} supplies {target}"]}
            ]
        }))
        .unwrap();

        let literal = "The contract says {target} supplies Widget.";
        let grounded = ground_chunk("literal", literal, &space, &[]);
        assert_eq!(grounded.len(), 1);
        assert_eq!(grounded[0].trigger, "{target} supplies Widget");
        let (start, end) = grounded[0].trigger_span;
        assert_eq!(&literal[start..end], "{target} supplies Widget");

        // A placeholder-shaped source surface must not be reinterpreted by
        // the later target substitution.
        assert!(ground_chunk("re-expanded", "Widget supplies Widget.", &space, &[]).is_empty());
    }

    #[test]
    fn sentence_gate_caches_one_decision_per_sentence() {
        let text = "trigger trigger trigger. Source trigger.";
        let text_cf = text.to_lowercase();
        let required_surfaces = ["source".to_string()];
        let required = vec![required_surfaces.as_slice()];
        let sentence_ranges = sentence_ranges_cf(&text_cf);
        let cache = RefCell::new(HashMap::new());
        let decisions: Vec<bool> = text_cf
            .match_indices("trigger")
            .map(|(offset, _)| {
                sentence_gate_accepts(&text_cf, &required, &sentence_ranges, &cache, offset)
            })
            .collect();

        assert_eq!(decisions, vec![false, false, false, true]);
        assert_eq!(
            cache.borrow().len(),
            2,
            "repeated occurrences must share the sentence-scoped scan"
        );
    }

    #[test]
    fn effective_config_indexes_large_alias_batches_without_changing_order() {
        let space: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [{"name": "Source", "type": "org", "aliases": []}],
            "relation_rules": []
        }))
        .unwrap();
        let neurons = (0..10_000)
            .map(|index| Neuron {
                id: format!("alias-{index}"),
                kind: NeuronKind::Alias,
                status: NeuronStatus::Accepted,
                evidence: vec!["reviewed evidence".into()],
                entity: "Source".into(),
                aliases: vec![format!("surface-{index}")],
                ..Neuron::default()
            })
            .collect::<Vec<_>>();

        let effective = effective_config(&space, &neurons);

        assert_eq!(effective.entities[0].aliases.len(), neurons.len());
        assert_eq!(effective.entities[0].aliases[0], "surface-0");
        assert_eq!(effective.entities[0].aliases[9_999], "surface-9999");
    }

    #[test]
    fn trigger_span_points_at_the_licensing_occurrence() {
        let space: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [
                {"name": "Nimbus", "type": "vendor"},
                {"name": "DataCloud", "type": "platform"}
            ],
            "relation_rules": [
                {"source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
                 "when_any": ["datacloud runs on nimbus"]}
            ]
        }))
        .unwrap();

        // Plain ASCII: the span slices the original text to the trigger.
        let text = "Everyone knows DataCloud runs on Nimbus these days.";
        let grounded = ground_chunk("c1", text, &space, &[]);
        let (start, end) = grounded[0].trigger_span;
        assert_eq!(&text[start..end], "DataCloud runs on Nimbus");

        // First occurrence negated, second affirms: the span must point
        // at the SECOND — the occurrence that actually licenses the fact.
        let text = "It is not true that DataCloud runs on Nimbus, some say. \
                    But in production, DataCloud runs on Nimbus.";
        let grounded = ground_chunk("c2", text, &space, &[]);
        let (start, end) = grounded[0].trigger_span;
        assert_eq!(&text[start..end], "DataCloud runs on Nimbus");
        assert!(start > 60, "span must be the affirmed second occurrence");

        // Non-ASCII prefix whose casefold EXPANDS ('İ' folds to 2 chars):
        // casefolded offsets diverge from original bytes; the span must
        // still slice the original text correctly.
        let text = "İİ say: DataCloud runs on Nimbus.";
        let grounded = ground_chunk("c3", text, &space, &[]);
        let (start, end) = grounded[0].trigger_span;
        assert_eq!(&text[start..end], "DataCloud runs on Nimbus");
    }

    #[test]
    fn sentence_bounds_keep_dotted_names_whole() {
        // "Chorus.ai" / "OpenProtein.AI": a '.' flanked by non-space is
        // part of the token — the chunk-sensitivity simulation showed
        // mid-name splits were the entire measured cost of adversarial
        // re-chunking, and they truncate the sentence a gate judges.
        let space: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [
                {"name": "Chorus.ai", "type": "org", "aliases": []},
                {"name": "ZoomInfo", "type": "org", "aliases": []}
            ],
            "relation_rules": [
                {"source": "Chorus.ai", "relation": "ACQUIRED_BY", "target": "ZoomInfo",
                 "when_any": ["chorus.ai (owned by zoominfo)"],
                 "require_in_sentence": ["source", "target"]}
            ]
        }))
        .unwrap();
        let text = "Rivals shifted. Chorus.ai (owned by ZoomInfo) grew fast.";
        let grounded = ground_chunk("c1", text, &space, &[]);
        assert_eq!(
            grounded.len(),
            1,
            "dotted name must not split the gated sentence"
        );
        let (start, end) = sentence_bounds(text, text.find("owned").unwrap());
        assert_eq!(
            &text[start..end].trim_start(),
            &"Chorus.ai (owned by ZoomInfo) grew fast."
        );
    }

    #[test]
    fn sentence_gate_survives_decimal_points() {
        // "$3.09B" must not split the sentence at the decimal point: the
        // gate judges the whole sentence, which names the source entity.
        let space: SpaceType = serde_json::from_value(serde_json::json!({
            "id": "s",
            "entities": [
                {"name": "CIP Market", "type": "market", "aliases": []},
                {"name": "$3.09B in 2024", "type": "figure", "aliases": ["$3.09 billion"]}
            ],
            "relation_rules": [
                {"source": "CIP Market", "relation": "HAS_SIZE", "target": "$3.09B in 2024",
                 "when_any": ["estimated at $3.09 billion"],
                 "require_in_sentence": ["source"]}
            ]
        }))
        .unwrap();
        let text = "Analysts disagree. The CIP Market was estimated at $3.09 billion in 2024.";
        let grounded = ground_chunk("c1", text, &space, &[]);
        assert_eq!(
            grounded.len(),
            1,
            "decimal point must not truncate the gated sentence"
        );

        // A real sentence boundary right after a digit still bounds:
        // "in 2024. Elsewhere..." — '.' followed by a space is a boundary.
        let (start, end) = sentence_bounds(text, text.find("estimated").unwrap());
        assert_eq!(text[start..end].trim_start(), &text[19..]);
    }

    #[test]
    fn case_bravo_negation_regression() {
        // The exact failure the research fixed: a retraction must not ground.
        assert!(!affirms_phrase(
            "Later analysis showed it is not true that the iPhone 14 contained Signal.",
            "iPhone 14 contained Signal"
        ));
        // An affirmative later clause still grounds despite an earlier negation.
        assert!(affirms_phrase(
            "It was not clear at first. The iPhone 14 contained Signal.",
            "iPhone 14 contained Signal"
        ));
        // Contractions count as negation cues.
        assert!(!affirms_phrase(
            "The device didn't run the Signal app build",
            "run the Signal app"
        ));
        assert!(affirms_phrase(
            "We stand with Ukraine.",
            "stand with Ukraine"
        ));
    }

    #[test]
    fn repeated_negated_occurrences_use_one_linear_clause_index() {
        let text = format!("not {}", "x ".repeat(524_286));
        assert_eq!(text.len(), 1_048_576);
        assert!(!affirms_phrase(&text, "x"));
    }

    /// The motivating cases: an entity name that CONTAINS an abbreviation dot
    /// must not be cut in half by sentence splitting, or the relation-semantics
    /// detector reports a correct fact as target-absent.
    #[test]
    fn abbreviation_dots_do_not_end_a_sentence() {
        let text = "Avoid concomitant use with St. John's Wort or Rifampin.";
        let (start, end) = sentence_bounds(text, text.find("Avoid").unwrap());
        assert_eq!(
            text[start..end].trim(),
            text,
            "St. must not split the sentence"
        );

        let text = "The 10 mg strength contains D C Yellow No. 10 Aluminum Lake.";
        let (start, end) = sentence_bounds(text, 0);
        assert!(
            text[start..end].contains("Aluminum Lake"),
            "No. must not split mid-entity-name: {:?}",
            &text[start..end]
        );
    }

    /// The dangerous direction. Failing to split MERGES sentences, and a
    /// `require_in_sentence` gate then judges a longer span and admits more
    /// groundings — so tokens that genuinely end sentences in label prose must
    /// keep splitting, and an ordinary sentence must be unaffected.
    #[test]
    fn genuine_sentence_ends_still_split() {
        for text in [
            // Corporate suffixes end the manufacturer line constantly.
            "Manufactured by Aurobindo Pharma USA, Inc. Distributed nationwide.",
            "Store below 25C, protect from light, etc. Dispense in a tight container.",
            "Alpha selected Beta. Gamma declined.",
        ] {
            let second = text.rfind(". ").expect("two sentences") + 2;
            let (start, end) = sentence_bounds(text, second);
            // `start` lands just after the boundary dot, i.e. on the space.
            assert_eq!(
                text[start..end].trim(),
                text[second..].trim(),
                "must still split before {:?} in {text:?}",
                &text[second..]
            );
        }
    }

    /// The abbreviation check runs on casefolded text too (the gate's hot path
    /// indexes sentences over `text.to_lowercase()`), so both coordinate systems
    /// must agree or a gate and its advisor would disagree about the sentence.
    #[test]
    fn abbreviation_handling_is_case_insensitive() {
        let upper = "TAKE WITH ST. JOHN'S WORT DAILY.";
        let (start, end) = sentence_bounds(upper, 0);
        assert_eq!(upper[start..end].trim(), upper);
        let lower = upper.to_lowercase();
        let (lstart, lend) = sentence_bounds(&lower, 0);
        assert_eq!(
            (start, end),
            (lstart, lend),
            "casefolded and original splitting must agree"
        );
    }
}
