//! Deterministic mining of the `neuron-authored` rule set from the WebNLG
//! authoring corpus (train+validation), the analog of clinical's corpus-derived
//! vocabulary lane. For each single-triple document where both entity surfaces
//! appear verbatim, the sentence is templatized — the subject surface becomes
//! `{source}`, the object surface `{target}` — yielding a trigger template for
//! that predicate. Templates are frequency-ranked per predicate and frozen.
//!
//! This never reads the `test` split (W7); callers pass only the authoring
//! corpus. The output is a reviewable candidate set: a human prunes it before it
//! is treated as the reviewed neuron-authored artifact. Mining is fully
//! deterministic (BTreeMap ordering + a lexical tie-break), so the frozen
//! artifact is byte-reproducible per `decision_rebuildable_derivatives.md`.

use std::collections::BTreeMap;

use crate::webnlg::model::{OracleRec, PilotDoc};
use crate::webnlg::rules::RuleSet;

/// Mining knobs. `min_count` drops one-off noise; `max_per_predicate` caps the
/// template count so a common predicate cannot swamp the rule set;
/// `disambiguate` prunes cross-predicate collisions (see [`mine_ruleset`]).
#[derive(Debug, Clone, Copy)]
pub struct MineOpts {
    pub min_count: usize,
    pub max_per_predicate: usize,
    pub disambiguate: bool,
}

impl Default for MineOpts {
    /// Measured-best on held-out validation: keeping singleton phrasings
    /// (`min_count = 1`) with a generous per-predicate cap lifts recall ~40%
    /// relative at no precision cost (the long-tail phrasings are mostly correct
    /// and disambiguation guards collisions). See the decision-doc recall
    /// addendum.
    fn default() -> Self {
        Self {
            min_count: 1,
            max_per_predicate: 20,
            disambiguate: true,
        }
    }
}

/// Turn one single-triple sentence into a `{source}`/`{target}` trigger
/// template, or `None` if either surface is absent or the result carries no
/// lexical content of its own. Surfaces are matched case-insensitively with
/// underscores expanded to spaces; the longer surface is replaced first so an
/// entity that is a substring of the other (e.g. "Aarhus" within
/// "Aarhus Airport") does not corrupt the replacement.
pub fn templatize(text: &str, subject: &str, object: &str) -> Option<String> {
    let low = text.to_lowercase();
    let subj = subject.replace('_', " ").to_lowercase();
    let obj = object.replace('_', " ").to_lowercase();
    if subj.is_empty() || obj.is_empty() || !low.contains(&subj) || !low.contains(&obj) {
        return None;
    }
    // Replace the longer surface first to avoid partial-overlap corruption.
    let (first, first_ph, second, second_ph) = if subj.len() >= obj.len() {
        (&subj, "{source}", &obj, "{target}")
    } else {
        (&obj, "{target}", &subj, "{source}")
    };
    let templated = low
        .replace(first.as_str(), first_ph)
        .replace(second.as_str(), second_ph);
    let trimmed = templated
        .trim()
        .trim_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | '!' | '?'))
        .trim()
        .to_string();

    // Both roles must survive (a self-relation collapses to one placeholder),
    // and the template must anchor on some literal text, not just the two
    // placeholders adjacent — otherwise it grounds almost anything.
    if !trimmed.contains("{source}") || !trimmed.contains("{target}") {
        return None;
    }
    let literal = trimmed.replace("{source}", "").replace("{target}", "");
    if literal.trim().len() < 2 {
        return None;
    }
    Some(trimmed)
}

/// Assign each template to the single predicate it most frequently lexicalizes
/// and remove it from all others. A template claimed by N predicates is correct
/// for at most one on any given text match; instantiated for the other N-1 it
/// manufactures false edges (e.g. `{source} is a {target}` under
/// course/occupation/profession/type). The owner is the highest-count predicate,
/// ties broken by lexical predicate name — deterministic.
pub(crate) fn disambiguate(counts: &mut BTreeMap<String, BTreeMap<String, usize>>) {
    // template -> (owning predicate, its count)
    let mut owner: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (predicate, templates) in counts.iter() {
        for (template, count) in templates {
            match owner.get(template) {
                Some((_, best)) if *count <= *best => {} // keep current owner (tie -> earlier, i.e. lexically smaller predicate)
                _ => {
                    owner.insert(template.clone(), (predicate.clone(), *count));
                }
            }
        }
    }
    for (predicate, templates) in counts.iter_mut() {
        templates.retain(|template, _| match owner.get(template) {
            Some((owning, _)) => owning == predicate,
            None => true,
        });
    }
}

/// Mine a `RuleSet` from the authoring corpus. Only single-triple documents are
/// used, so each templatized sentence is unambiguously attributable to one
/// predicate. When `opts.disambiguate`, cross-predicate collisions are pruned
/// (see [`disambiguate`]). Templates below `min_count` are dropped; the top
/// `max_per_predicate` by frequency (lexical tie-break) are kept.
pub fn mine_ruleset(
    name: &str,
    documents: &[PilotDoc],
    oracle: &[OracleRec],
    opts: &MineOpts,
) -> RuleSet {
    let text_by_id: BTreeMap<&str, &str> = documents
        .iter()
        .map(|d| (d.document_id.as_str(), d.text.as_str()))
        .collect();

    // predicate -> template -> occurrence count
    let mut counts: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for rec in oracle {
        if rec.triples.len() != 1 {
            continue;
        }
        let triple = &rec.triples[0];
        let Some(text) = text_by_id.get(rec.document_id.as_str()) else {
            continue;
        };
        if let Some(template) = templatize(text, &triple.subject, &triple.object) {
            *counts
                .entry(triple.predicate.clone())
                .or_default()
                .entry(template)
                .or_default() += 1;
        }
    }

    if opts.disambiguate {
        disambiguate(&mut counts);
    }

    let mut predicates: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (predicate, templates) in counts {
        let mut ranked: Vec<(String, usize)> = templates
            .into_iter()
            .filter(|(_, count)| *count >= opts.min_count)
            .collect();
        // Frequency descending, then template text ascending — deterministic.
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked.truncate(opts.max_per_predicate);
        if !ranked.is_empty() {
            predicates.insert(predicate, ranked.into_iter().map(|(t, _)| t).collect());
        }
    }

    RuleSet {
        name: name.to_string(),
        predicates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::model::Triple;

    fn doc(id: &str, text: &str) -> PilotDoc {
        PilotDoc {
            document_id: id.into(),
            split: "train".into(),
            category: "x".into(),
            text: text.into(),
        }
    }
    fn rec(id: &str, triples: Vec<Triple>) -> OracleRec {
        OracleRec {
            document_id: id.into(),
            split: "train".into(),
            triples,
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
    fn templatize_binds_roles_not_text_position() {
        // Reversed lexicalization still binds {source}=subject, {target}=object.
        let t = templatize(
            "Jacob Bundsgaard is the leader of Aarhus.",
            "Aarhus",
            "Jacob_Bundsgaard",
        );
        assert_eq!(t, Some("{target} is the leader of {source}".to_string()));
    }

    #[test]
    fn templatize_replaces_longer_surface_first() {
        // Subject "Aarhus" is a substring of object "Aarhus Airport": the object
        // must be captured whole, not shredded by the subject replacement.
        let t = templatize("Aarhus Airport serves Aarhus.", "Aarhus", "Aarhus_Airport");
        assert_eq!(t, Some("{target} serves {source}".to_string()));
    }

    #[test]
    fn templatize_rejects_absent_surface_and_degenerate_output() {
        // object surface not in text
        assert_eq!(
            templatize("The leader of Aarhus.", "Aarhus", "Jacob_Bundsgaard"),
            None
        );
        // no literal anchor between the placeholders
        assert_eq!(templatize("Aarhus Denmark", "Aarhus", "Denmark"), None);
    }

    #[test]
    fn mine_keeps_frequent_templates_and_drops_one_offs() {
        let documents = vec![
            doc("a", "The leader of Aarhus is Jacob Bundsgaard."),
            doc("b", "The leader of Paris is Anne Hidalgo."),
            doc("c", "Aarhus is governed by Jacob Bundsgaard."), // one-off phrasing
        ];
        let oracle = vec![
            rec("a", vec![triple("Aarhus", "leader", "Jacob_Bundsgaard")]),
            rec("b", vec![triple("Paris", "leader", "Anne_Hidalgo")]),
            rec("c", vec![triple("Aarhus", "leader", "Jacob_Bundsgaard")]),
        ];
        // Explicit min_count=2 (not the default) to exercise the noise filter.
        let opts = MineOpts {
            min_count: 2,
            max_per_predicate: 8,
            disambiguate: true,
        };
        let rs = mine_ruleset("mined", &documents, &oracle, &opts);
        // "the leader of {source} is {target}" occurs twice (>= min_count 2);
        // "{source} is governed by {target}" occurs once and is dropped.
        assert_eq!(
            rs.predicates["leader"],
            vec!["the leader of {source} is {target}".to_string()]
        );
    }

    #[test]
    fn disambiguation_assigns_colliding_template_to_dominant_predicate() {
        // "{source} is a {target}" is mined for occupation (2 docs) and
        // profession (1 doc). Disambiguation keeps it only for occupation.
        let documents = vec![
            doc("a", "Alan is a writer."),
            doc("b", "Beth is a painter."),
            doc("c", "Carl is a doctor."),
        ];
        let oracle = vec![
            rec("a", vec![triple("Alan", "occupation", "writer")]),
            rec("b", vec![triple("Beth", "occupation", "painter")]),
            rec("c", vec![triple("Carl", "profession", "doctor")]),
        ];
        let opts = MineOpts {
            min_count: 1,
            max_per_predicate: 8,
            disambiguate: true,
        };
        let rs = mine_ruleset("m", &documents, &oracle, &opts);
        assert_eq!(
            rs.predicates["occupation"],
            vec!["{source} is a {target}".to_string()]
        );
        // profession's only template was reassigned away, so it drops entirely.
        assert!(!rs.predicates.contains_key("profession"));
    }

    #[test]
    fn without_disambiguation_a_collision_stays_under_both_predicates() {
        let documents = vec![
            doc("a", "Alan is a writer."),
            doc("b", "Beth is a painter."),
            doc("c", "Carl is a doctor."),
        ];
        let oracle = vec![
            rec("a", vec![triple("Alan", "occupation", "writer")]),
            rec("b", vec![triple("Beth", "occupation", "painter")]),
            rec("c", vec![triple("Carl", "profession", "doctor")]),
        ];
        let opts = MineOpts {
            min_count: 1,
            max_per_predicate: 8,
            disambiguate: false,
        };
        let rs = mine_ruleset("m", &documents, &oracle, &opts);
        assert!(rs.predicates.contains_key("occupation"));
        assert!(rs.predicates.contains_key("profession"));
    }

    #[test]
    fn mine_skips_multi_triple_documents() {
        // A two-triple document is not cleanly attributable, so it is ignored.
        let documents = vec![doc(
            "a",
            "The leader of Aarhus is Jacob Bundsgaard, in Denmark.",
        )];
        let oracle = vec![rec(
            "a",
            vec![
                triple("Aarhus", "leader", "Jacob_Bundsgaard"),
                triple("Aarhus", "country", "Denmark"),
            ],
        )];
        let rs = mine_ruleset("mined", &documents, &oracle, &MineOpts::default());
        assert!(rs.predicates.is_empty());
    }

    #[test]
    fn mined_ruleset_round_trips_through_json() {
        let documents = vec![
            doc("a", "The leader of Aarhus is Jacob Bundsgaard."),
            doc("b", "The leader of Paris is Anne Hidalgo."),
        ];
        let oracle = vec![
            rec("a", vec![triple("Aarhus", "leader", "Jacob_Bundsgaard")]),
            rec("b", vec![triple("Paris", "leader", "Anne_Hidalgo")]),
        ];
        let rs = mine_ruleset("mined", &documents, &oracle, &MineOpts::default());
        let json = serde_json::to_string(&rs).unwrap();
        let loaded = RuleSet::from_json(&json).unwrap();
        assert_eq!(loaded.predicates, rs.predicates);
    }
}
