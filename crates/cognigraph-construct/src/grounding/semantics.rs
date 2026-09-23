//! The advisory relation-semantics detector (`SEMANTICS_REV`).

use super::*;

/// Revision of the relation-semantics signal set, stamped on every stored
/// verdict so a consumer can tell which detector produced it (and re-run when
/// it moves). The detector is advisory and expected to evolve; the governed
/// fact record deliberately does not carry it
/// (decision_pilot_clinical_graph.md, D2).
pub const SEMANTICS_REV: &str = "semantics-v1";

/// Words carrying no relation meaning, so their absence says nothing.
pub(super) const RELATION_STOPWORDS: [&str; 21] = [
    "of", "to", "in", "with", "for", "by", "on", "at", "as", "the", "a", "an", "is", "are", "has",
    "have", "be", "and", "or", "from", "into",
];

/// Crude prefix stem, enough to survive inflection (`treats`/`treatment`,
/// `causes`/`caused`, `indicated`/`indications`) without a stemmer dependency.
pub(super) fn stem(word: &str) -> &str {
    let keep = word.len().saturating_sub(3).max(4).min(word.len());
    let mut end = keep;
    while end > 0 && !word.is_char_boundary(end) {
        end -= 1;
    }
    &word[..end]
}

pub(super) fn entity_surfaces<'a>(name: &'a str, entities: &'a [EntityDef]) -> Vec<String> {
    let mut out = vec![name.to_lowercase()];
    for entity in entities.iter().filter(|e| e.name == name) {
        out.extend(entity.aliases.iter().map(|a| a.to_lowercase()));
    }
    out.retain(|s| !s.trim().is_empty());
    out
}

pub(super) fn names_entity(name: &str, entities: &[EntityDef], sentence_cf: &str) -> bool {
    entity_surfaces(name, entities)
        .iter()
        .any(|surface| sentence_cf.contains(surface.as_str()))
}

/// Source and target sit adjacent in a coordinated list with nothing but a
/// separator between them: the sentence ENUMERATES the two, it does not relate
/// them. `myopathy and rhabdomyolysis` licensed `myopathy --CAUSES-->
/// rhabdomyolysis`; `on moles, birthmarks, warts` licensed `common warts
/// --LOCATED_ON--> moles`.
pub(super) fn merely_co_listed(
    source: &str,
    target: &str,
    entities: &[EntityDef],
    sentence_cf: &str,
) -> bool {
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
pub(super) fn relation_vocabulary_absent(relation: &str, sentence_cf: &str) -> bool {
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
