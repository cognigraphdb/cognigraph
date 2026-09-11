//! The template-proposal loop — the favorable recall lever (better *precise*
//! templates, not looser matching; see the decision-doc recall addendum). A
//! [`TemplateProposer`] turns per-predicate examples into candidate
//! `{source}`/`{target}` trigger templates that feed the EXACT matcher, so they
//! fire precisely (precision held) while covering more phrasings (recall up).
//!
//! Two proposers: the deterministic [`CorpusProposer`] (templatizes attributed
//! corpus sentences — reproducible, measurable now) and a future LLM proposer
//! (would implement [`TemplateProposer`] by prompting a model with [`llm_prompt`]
//! and parsing [`parse_proposed_templates`]). The LLM path is gated: it is an
//! outward, paid, non-deterministic call that trades the pilot's byte
//! reproducibility, so it runs only behind an explicit provider, never here.
//! Everything a proposer emits is a *candidate* — a human reviews before it is
//! frozen (W1).

use std::collections::BTreeMap;

use crate::webnlg::mining::{MineOpts, disambiguate, templatize};
use crate::webnlg::model::{OracleRec, PilotDoc};
use crate::webnlg::rules::RuleSet;

/// One example of how a predicate is expressed: the entity surfaces and the
/// sentence they appear in.
#[derive(Debug, Clone)]
pub struct Example {
    pub subject: String,
    pub object: String,
    pub sentence: String,
}

/// Proposes candidate `{source}`/`{target}` trigger templates for a predicate.
/// Returned templates may repeat — repetition is frequency evidence.
pub trait TemplateProposer {
    fn propose(&self, predicate: &str, examples: &[Example]) -> Vec<String>;
}

/// A trigger template is usable only if it binds both roles and carries some
/// literal connective text (not just the two placeholders adjacent, which would
/// ground almost anything).
pub fn valid_template(template: &str) -> bool {
    if !template.contains("{source}") || !template.contains("{target}") {
        return false;
    }
    let literal = template.replace("{source}", "").replace("{target}", "");
    literal.trim().len() >= 2
}

/// The deterministic proposer: templatize each example sentence with the miner's
/// exact [`templatize`]. Reproducible; the measurable stand-in for the LLM.
pub struct CorpusProposer;

impl TemplateProposer for CorpusProposer {
    fn propose(&self, _predicate: &str, examples: &[Example]) -> Vec<String> {
        examples
            .iter()
            .filter_map(|e| templatize(&e.sentence, &e.subject, &e.object))
            .collect()
    }
}

/// Build the proposal prompt a live LLM proposer would send: it shows how a
/// predicate is expressed and asks for generalized `{source}`/`{target}`
/// templates. (Plumbing only — no model is called here.)
pub fn llm_prompt(predicate: &str, examples: &[Example]) -> String {
    let mut prompt = format!(
        "The DBpedia relation `{predicate}` links a subject to an object. Below \
are sentences that express it, with the subject and object marked.\n\n"
    );
    for e in examples.iter().take(20) {
        prompt.push_str(&format!(
            "- subject={:?} object={:?} :: {}\n",
            e.subject.replace('_', " "),
            e.object.replace('_', " "),
            e.sentence.trim()
        ));
    }
    prompt.push_str(
        "\nWrite up to 10 general trigger templates that would match NEW sentences \
expressing this exact relation. Use `{source}` for the subject and `{target}` \
for the object. Keep them precise — each must contain a connective phrase, not \
just the two placeholders. One template per line, nothing else.",
    );
    prompt
}

/// Parse an LLM proposal response: one template per line, keep only valid ones.
/// (Used by a live LLM proposer impl; unit-testable without a model.)
pub fn parse_proposed_templates(response: &str) -> Vec<String> {
    response
        .lines()
        .map(|l| l.trim().trim_start_matches(['-', '*', '•', ' ']).trim())
        .filter(|l| valid_template(l))
        .map(|l| l.to_lowercase())
        .collect()
}

/// Gather per-predicate examples from the authoring corpus (never `test`, W7):
/// single-triple documents (whole text), plus multi-triple documents where a
/// triple is unambiguously attributable to one sentence (both surfaces present
/// and no other triple of that document shares the sentence).
pub fn gather_examples(
    documents: &[PilotDoc],
    oracle: &[OracleRec],
) -> BTreeMap<String, Vec<Example>> {
    let text_by_id: BTreeMap<&str, &str> = documents
        .iter()
        .map(|d| (d.document_id.as_str(), d.text.as_str()))
        .collect();

    let mut by_predicate: BTreeMap<String, Vec<Example>> = BTreeMap::new();
    for rec in oracle {
        let Some(text) = text_by_id.get(rec.document_id.as_str()) else {
            continue;
        };
        if rec.triples.len() == 1 {
            let t = &rec.triples[0];
            by_predicate
                .entry(t.predicate.clone())
                .or_default()
                .push(Example {
                    subject: t.subject.clone(),
                    object: t.object.clone(),
                    sentence: (*text).to_string(),
                });
            continue;
        }
        // Multi-triple: attribute a triple to a sentence only when it is the
        // ONLY triple of the document with both surfaces in that sentence.
        for sentence in text.split(['.', '!', '?']) {
            let low = sentence.to_lowercase();
            let here: Vec<&_> = rec
                .triples
                .iter()
                .filter(|t| {
                    low.contains(&t.subject.replace('_', " ").to_lowercase())
                        && low.contains(&t.object.replace('_', " ").to_lowercase())
                })
                .collect();
            if let [t] = here.as_slice() {
                by_predicate
                    .entry(t.predicate.clone())
                    .or_default()
                    .push(Example {
                        subject: t.subject.clone(),
                        object: t.object.clone(),
                        sentence: sentence.to_string(),
                    });
            }
        }
    }
    by_predicate
}

/// Run the proposal loop: gather examples, propose templates per predicate,
/// validate, frequency-rank, disambiguate cross-predicate collisions, and merge
/// into a candidate `RuleSet`. Same ranking/disambiguation as the miner, so a
/// proposed set composes with the frozen policy.
pub fn propose_ruleset<P: TemplateProposer>(
    name: &str,
    documents: &[PilotDoc],
    oracle: &[OracleRec],
    proposer: &P,
    opts: &MineOpts,
) -> RuleSet {
    let examples = gather_examples(documents, oracle);

    let mut counts: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (predicate, exs) in &examples {
        for template in proposer.propose(predicate, exs) {
            if valid_template(&template) {
                *counts
                    .entry(predicate.clone())
                    .or_default()
                    .entry(template)
                    .or_default() += 1;
            }
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

/// Merge proposed templates onto a mined baseline and re-run cross-predicate
/// disambiguation — the review step the loop specifies. Mined templates outweigh
/// proposals, so a proposed template that collides with a mined one owned by
/// another predicate is dropped, and a proposal shared across predicates is kept
/// for a single owner. (Does not fix an intrinsically over-general single-
/// predicate template — that is the residual a human reviewer prunes.)
pub fn merge_and_disambiguate(
    baseline: &RuleSet,
    proposals: &BTreeMap<String, Vec<String>>,
) -> RuleSet {
    let mut counts: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (predicate, templates) in &baseline.predicates {
        for t in templates {
            *counts
                .entry(predicate.clone())
                .or_default()
                .entry(t.clone())
                .or_default() += 1000;
        }
    }
    for (predicate, templates) in proposals {
        for t in templates {
            if valid_template(t) {
                *counts
                    .entry(predicate.clone())
                    .or_default()
                    .entry(t.clone())
                    .or_default() += 1;
            }
        }
    }
    disambiguate(&mut counts);
    let mut predicates: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (predicate, templates) in counts {
        let kept: Vec<String> = templates.into_keys().collect();
        if !kept.is_empty() {
            predicates.insert(predicate, kept);
        }
    }
    RuleSet {
        name: format!("{}+llm-reviewed", baseline.name),
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
    fn t(s: &str, p: &str, o: &str) -> Triple {
        Triple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
        }
    }

    #[test]
    fn gathers_single_and_attributed_multi_triple_examples() {
        let docs = vec![
            doc("a", "The leader of Aarhus is Jacob Bundsgaard."),
            doc(
                "b",
                "Aarhus is in Denmark. The leader of Aarhus is Jacob Bundsgaard.",
            ),
        ];
        let oracle = vec![
            rec("a", vec![t("Aarhus", "leader", "Jacob_Bundsgaard")]),
            rec(
                "b",
                vec![
                    t("Aarhus", "country", "Denmark"),
                    t("Aarhus", "leader", "Jacob_Bundsgaard"),
                ],
            ),
        ];
        let ex = gather_examples(&docs, &oracle);
        // 'leader' has the single-triple example plus the attributed multi-triple one.
        assert_eq!(ex["leader"].len(), 2);
        // 'country' is attributed from "Aarhus is in Denmark".
        assert_eq!(ex["country"].len(), 1);
    }

    #[test]
    fn corpus_proposer_templatizes_examples() {
        let ex = vec![Example {
            subject: "Aarhus".into(),
            object: "Jacob_Bundsgaard".into(),
            sentence: "The leader of Aarhus is Jacob Bundsgaard".into(),
        }];
        let out = CorpusProposer.propose("leader", &ex);
        assert_eq!(out, vec!["the leader of {source} is {target}".to_string()]);
    }

    #[test]
    fn llm_prompt_and_parse_round_trip() {
        let ex = vec![Example {
            subject: "Aarhus".into(),
            object: "Jacob_Bundsgaard".into(),
            sentence: "Jacob Bundsgaard heads Aarhus.".into(),
        }];
        let prompt = llm_prompt("leader", &ex);
        assert!(prompt.contains("{source}") && prompt.contains("{target}"));
        assert!(prompt.contains("Jacob Bundsgaard heads Aarhus"));
        // A model's reply — bullets, a bad line, mixed case — parses to valid only.
        let parsed = parse_proposed_templates(
            "- {target} heads {source}\n* {source} is led by {target}\ngarbage with no slots\n{source}{target}",
        );
        assert_eq!(
            parsed,
            vec![
                "{target} heads {source}".to_string(),
                "{source} is led by {target}".to_string()
            ]
        );
    }

    #[test]
    fn propose_ruleset_with_a_mock_proposer_merges_and_disambiguates() {
        struct Mock;
        impl TemplateProposer for Mock {
            fn propose(&self, predicate: &str, _e: &[Example]) -> Vec<String> {
                // Both predicates propose the SAME ambiguous template; the more
                // frequent owner (leader, 2 examples) keeps it.
                match predicate {
                    "leader" => vec![
                        "{source} leads {target}".into(),
                        "{source} leads {target}".into(),
                    ],
                    _ => vec!["{source} leads {target}".into()],
                }
            }
        }
        let docs = vec![doc("a", "irrelevant"), doc("b", "irrelevant")];
        let oracle = vec![
            rec("a", vec![t("A", "leader", "B")]),
            rec("b", vec![t("C", "governor", "D")]),
        ];
        let opts = MineOpts {
            min_count: 1,
            max_per_predicate: 8,
            disambiguate: true,
        };
        let rs = propose_ruleset("m", &docs, &oracle, &Mock, &opts);
        assert_eq!(
            rs.predicates["leader"],
            vec!["{source} leads {target}".to_string()]
        );
        assert!(!rs.predicates.contains_key("governor")); // template reassigned to leader
    }

    #[test]
    fn merge_and_disambiguate_drops_a_proposal_colliding_with_a_mined_template() {
        let mut base = BTreeMap::new();
        base.insert(
            "birthPlace".to_string(),
            vec!["{source} was born in {target}".to_string()],
        );
        let baseline = RuleSet {
            name: "b".into(),
            predicates: base,
        };
        // The LLM proposes the SAME template for `origin`; the mined owner wins.
        let mut proposals: BTreeMap<String, Vec<String>> = BTreeMap::new();
        proposals.insert(
            "origin".to_string(),
            vec![
                "{source} was born in {target}".to_string(),
                "{source} hails from {target}".to_string(),
            ],
        );
        let merged = merge_and_disambiguate(&baseline, &proposals);
        assert!(
            merged.predicates["birthPlace"].contains(&"{source} was born in {target}".to_string())
        );
        // origin keeps only its non-colliding novel proposal.
        assert_eq!(
            merged.predicates["origin"],
            vec!["{source} hails from {target}".to_string()]
        );
    }
}
