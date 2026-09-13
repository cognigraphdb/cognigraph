//! The recall-frontier matcher (webnlg only — does NOT touch the shared
//! `ground_chunk` engine). Where exact grounding requires a template to appear
//! as a verbatim substring, this fires when a template's connective tokens
//! appear *in order* within one sentence that also contains both entity
//! surfaces in their template roles, with arbitrary words allowed between the
//! parts. It deliberately trades precision for recall; the trade is measured on
//! held-out validation (`webnlg-score`), never on the spent `test` split (W7).

use crate::webnlg::model::Triple;
use crate::webnlg::rules::{RuleSet, date_aliases};

/// One ordered part of a template: a run of literal connective tokens, or an
/// entity role marker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    Lit(Vec<String>),
    Source,
    Target,
}

/// Articles ignored on both sides so "born in the {target}" and "born in
/// {target}" match the same text. Deliberately tiny — dropping real content
/// words would gut precision.
const ARTICLES: [&str; 3] = ["the", "a", "an"];

/// Lowercase, split on any non-alphanumeric, drop articles. Numbers survive
/// (date/measurement objects tokenize to their digits).
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .filter(|t| !ARTICLES.contains(&t.as_str()))
        .collect()
}

/// Split a template into ordered slots on the `{source}`/`{target}` markers.
fn parse_template(template: &str) -> Vec<Slot> {
    let mut slots = Vec::new();
    let mut rest = template;
    while !rest.is_empty() {
        let src = rest.find("{source}");
        let tgt = rest.find("{target}");
        let next = match (src, tgt) {
            (Some(s), Some(t)) => Some((s.min(t), if s < t { Slot::Source } else { Slot::Target })),
            (Some(s), None) => Some((s, Slot::Source)),
            (None, Some(t)) => Some((t, Slot::Target)),
            (None, None) => None,
        };
        match next {
            Some((at, marker)) => {
                let lit = tokenize(&rest[..at]);
                if !lit.is_empty() {
                    slots.push(Slot::Lit(lit));
                }
                slots.push(marker);
                rest = &rest[at + "{source}".len()..]; // both markers are 8 bytes
            }
            None => {
                let lit = tokenize(rest);
                if !lit.is_empty() {
                    slots.push(Slot::Lit(lit));
                }
                break;
            }
        }
    }
    slots
}

/// The surface token sequences an entity may appear as: the underscore→space
/// form, the raw token, and (for ISO dates) its prose renderings.
fn entity_surfaces(token: &str) -> Vec<Vec<String>> {
    let mut surfaces = vec![tokenize(&token.replace('_', " ")), tokenize(token)];
    for alias in date_aliases(token) {
        surfaces.push(tokenize(&alias));
    }
    surfaces.retain(|s| !s.is_empty());
    surfaces.sort();
    surfaces.dedup();
    surfaces
}

/// Earliest contiguous occurrence of any `surfaces` variant at or after `from`,
/// as `(start, end)`.
fn find_contiguous(
    tokens: &[String],
    surfaces: &[Vec<String>],
    from: usize,
) -> Option<(usize, usize)> {
    (from..tokens.len()).find_map(|i| {
        surfaces.iter().find_map(|s| {
            (!s.is_empty() && tokens[i..].starts_with(s.as_slice())).then_some((i, i + s.len()))
        })
    })
}

/// Does the slot sequence match this sentence, left to right? Each part —
/// literal connective run OR entity surface — must match CONTIGUOUSLY, and at
/// most `max_gap` unmatched tokens may sit between consecutive parts. Small
/// `max_gap` keeps the connective bound to its entities (precision); large
/// `max_gap` approaches raw co-occurrence (recall). The first part may sit
/// anywhere in the sentence.
fn matches(
    tokens: &[String],
    slots: &[Slot],
    src: &[Vec<String>],
    tgt: &[Vec<String>],
    max_gap: usize,
) -> bool {
    let mut cursor = 0usize; // scan start
    let mut prev_end: Option<usize> = None; // end of the previous matched part
    for slot in slots {
        let found = match slot {
            Slot::Lit(lits) => (cursor..tokens.len())
                .find(|&i| tokens[i..].starts_with(lits.as_slice()))
                .map(|i| (i, i + lits.len())),
            Slot::Source => find_contiguous(tokens, src, cursor),
            Slot::Target => find_contiguous(tokens, tgt, cursor),
        };
        let (start, end) = match found {
            Some(pair) => pair,
            None => return false,
        };
        if let Some(pe) = prev_end
            && start - pe > max_gap
        {
            return false;
        }
        prev_end = Some(end);
        cursor = end;
    }
    true
}

/// Fuzzily ground triples for one document. `only_predicates`, when `Some`,
/// restricts to the gold predicates (oracle-diagnostic lane).
pub fn ground_fuzzy(
    text: &str,
    entities: &[String],
    rules: &RuleSet,
    only_predicates: Option<&[String]>,
    max_gap: usize,
) -> Vec<Triple> {
    // Precompute surfaces + parsed templates once.
    let surfaces: Vec<(&String, Vec<Vec<String>>)> =
        entities.iter().map(|e| (e, entity_surfaces(e))).collect();
    let parsed: Vec<(&String, Vec<Vec<Slot>>)> = rules
        .predicates
        .iter()
        .filter(|(p, _)| only_predicates.is_none_or(|allow| allow.iter().any(|a| a == *p)))
        .map(|(p, templates)| (p, templates.iter().map(|t| parse_template(t)).collect()))
        .collect();

    let mut out: Vec<Triple> = Vec::new();
    for sentence in text.split(['.', '!', '?']) {
        let tokens = tokenize(sentence);
        if tokens.is_empty() {
            continue;
        }
        // Which provided entities appear in this sentence at all.
        let present: Vec<usize> = surfaces
            .iter()
            .enumerate()
            .filter(|(_, (_, surf))| find_contiguous(&tokens, surf, 0).is_some())
            .map(|(i, _)| i)
            .collect();
        for &si in &present {
            for &ti in &present {
                if si == ti {
                    continue;
                }
                let (src_tok, src_surf) = &surfaces[si];
                let (tgt_tok, tgt_surf) = &surfaces[ti];
                for (predicate, templates) in &parsed {
                    let hit = templates
                        .iter()
                        .any(|slots| matches(&tokens, slots, src_surf, tgt_surf, max_gap));
                    if hit {
                        out.push(Triple {
                            subject: (*src_tok).clone(),
                            predicate: (*predicate).clone(),
                            object: (*tgt_tok).clone(),
                        });
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| {
        (&a.subject, &a.predicate, &a.object).cmp(&(&b.subject, &b.predicate, &b.object))
    });
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn ruleset(pred: &str, templates: &[&str]) -> RuleSet {
        let mut predicates = BTreeMap::new();
        predicates.insert(
            pred.to_string(),
            templates.iter().map(|t| t.to_string()).collect(),
        );
        RuleSet {
            name: "t".into(),
            predicates,
        }
    }
    fn triple(s: &str, p: &str, o: &str) -> Triple {
        Triple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
        }
    }

    #[test]
    fn fires_across_inserted_words_that_exact_matching_would_miss() {
        // "leader of {source} is {target}" — the sentence inserts words the
        // exact substring matcher could never absorb.
        let entities = vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let rs = ruleset("leader", &["leader of {source} is {target}"]);
        let built = ground_fuzzy(
            "The current leader of the city of Aarhus is, notably, Jacob Bundsgaard.",
            &entities,
            &rs,
            None,
            usize::MAX,
        );
        assert!(built.contains(&triple("Aarhus", "leader", "Jacob_Bundsgaard")));
    }

    #[test]
    fn respects_direction_via_slot_order() {
        let entities = vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let rs = ruleset("leader", &["leader of {source} is {target}"]);
        // Text supports Jacob-leads-Aarhus phrasing but the template's roles put
        // {source}=Aarhus, {target}=Jacob; a reversed sentence must NOT ground
        // (Aarhus, leader, Jacob) here.
        let built = ground_fuzzy(
            "Jacob Bundsgaard is leader of Aarhus.",
            &entities,
            &rs,
            None,
            usize::MAX,
        );
        assert!(!built.contains(&triple("Aarhus", "leader", "Jacob_Bundsgaard")));
    }

    #[test]
    fn requires_the_connective_tokens_precision_guard() {
        // Both entities co-occur but the connective "leader"/"born" is absent.
        let entities = vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let rs = ruleset("leader", &["leader of {source} is {target}"]);
        let built = ground_fuzzy(
            "Aarhus and Jacob Bundsgaard were photographed.",
            &entities,
            &rs,
            None,
            usize::MAX,
        );
        assert!(built.is_empty());
    }

    #[test]
    fn stays_within_one_sentence() {
        // Connective in one sentence, the object in another — must not ground.
        let entities = vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let rs = ruleset("leader", &["leader of {source} is {target}"]);
        let built = ground_fuzzy(
            "The leader of Aarhus resigned. Jacob Bundsgaard was elsewhere.",
            &entities,
            &rs,
            None,
            usize::MAX,
        );
        assert!(built.is_empty());
    }

    #[test]
    fn date_object_grounds_from_prose_via_aliases() {
        let entities = vec!["Ace_Wilder".to_string(), "1982-07-23".to_string()];
        let rs = ruleset("birthDate", &["{source} was born on {target}"]);
        let built = ground_fuzzy(
            "Ace Wilder was born on July 23, 1982.",
            &entities,
            &rs,
            None,
            usize::MAX,
        );
        assert!(built.contains(&triple("Ace_Wilder", "birthDate", "1982-07-23")));
    }
}
