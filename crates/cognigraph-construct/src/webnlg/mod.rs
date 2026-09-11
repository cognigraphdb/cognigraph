//! WebNLG relation-construction scorer. Freezes to the policy in
//! docs/decisions/decision_webnlg_scoring.md (W1-W7).

pub mod fuzzy;
pub mod mining;
pub mod model;
pub mod normalize;
pub mod propose;
pub mod rules;
pub mod score;

pub use fuzzy::ground_fuzzy;
pub use mining::{MineOpts, mine_ruleset};
pub use model::{Diagnostics, OracleRec, PilotDoc, PredicateScore, Triple, WebnlgScore};
pub use propose::{CorpusProposer, TemplateProposer, merge_and_disambiguate, propose_ruleset};
pub use rules::{RuleSet, build_space};

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;

use crate::grounding::ground_chunk;
use crate::webnlg::score::score_one;

/// Construct triples for one document under a lane. Entities are provided in all
/// lanes (W2); OracleDiagnostic additionally restricts rules to the gold
/// predicates. Returns constructed triples with predicate = the rule relation.
pub fn construct_document(
    lane: Lane,
    text: &str,
    entities: &[String],
    gold: &[Triple],
    rules: &RuleSet,
) -> Vec<Triple> {
    let only: Option<Vec<String>> = match lane {
        Lane::OracleDiagnostic => Some(gold.iter().map(|t| t.predicate.clone()).collect()),
        Lane::Generic | Lane::NeuronAuthored => None,
    };
    let space = build_space(entities, rules, only.as_deref());
    ground_chunk("webnlg", text, &space, &[])
        .into_iter()
        .map(|g| Triple {
            subject: g.fact.source,
            predicate: g.fact.relation,
            object: g.fact.target,
        })
        .collect()
}

/// Like [`construct_document`], but grounds with the looser [`ground_fuzzy`]
/// matcher (the recall-frontier experiment) instead of exact `ground_chunk`.
pub fn construct_document_fuzzy(
    lane: Lane,
    text: &str,
    entities: &[String],
    gold: &[Triple],
    rules: &RuleSet,
    max_gap: usize,
) -> Vec<Triple> {
    let only: Option<Vec<String>> = match lane {
        Lane::OracleDiagnostic => Some(gold.iter().map(|t| t.predicate.clone()).collect()),
        Lane::Generic | Lane::NeuronAuthored => None,
    };
    ground_fuzzy(text, entities, rules, only.as_deref(), max_gap)
}

/// Run one lane over aligned documents + oracle and score it. `entities_for`
/// supplies each document's provided entity surfaces (W2) — the caller sources
/// them from the oracle surface forms. Pure: no I/O, no backend.
///
/// Documents and oracle are aligned 1:1 by `document_id`; a document with no
/// matching oracle record is skipped, so `WebnlgScore.documents` is the MATCHED
/// count, which a caller can compare against `documents.len()` to detect
/// misalignment. In the prepared pilot the two align by construction.
pub fn score_documents<F>(
    lane: Lane,
    documents: &[PilotDoc],
    oracle: &[OracleRec],
    entities_for: F,
    rules: &RuleSet,
) -> WebnlgScore
where
    F: Fn(&str) -> Vec<String>,
{
    score_documents_impl(lane, documents, oracle, entities_for, rules, None)
}

/// Like [`score_documents`] but grounds with the looser [`ground_fuzzy`] matcher
/// (the recall-frontier experiment). Held-out validation only, never `test` (W7).
pub fn score_documents_fuzzy<F>(
    lane: Lane,
    documents: &[PilotDoc],
    oracle: &[OracleRec],
    entities_for: F,
    rules: &RuleSet,
    max_gap: usize,
) -> WebnlgScore
where
    F: Fn(&str) -> Vec<String>,
{
    score_documents_impl(lane, documents, oracle, entities_for, rules, Some(max_gap))
}

fn score_documents_impl<F>(
    lane: Lane,
    documents: &[PilotDoc],
    oracle: &[OracleRec],
    entities_for: F,
    rules: &RuleSet,
    fuzzy_gap: Option<usize>,
) -> WebnlgScore
where
    F: Fn(&str) -> Vec<String>,
{
    let oracle_by_id: BTreeMap<&str, &OracleRec> =
        oracle.iter().map(|r| (r.document_id.as_str(), r)).collect();

    let pairs: Vec<(Vec<Triple>, Vec<Triple>)> = documents
        .iter()
        .filter_map(|doc| {
            let rec = oracle_by_id.get(doc.document_id.as_str())?;
            let entities = entities_for(&doc.document_id);
            let built = match fuzzy_gap {
                Some(gap) => {
                    construct_document_fuzzy(lane, &doc.text, &entities, &rec.triples, rules, gap)
                }
                None => construct_document(lane, &doc.text, &entities, &rec.triples, rules),
            };
            Some((rec.triples.clone(), built))
        })
        .collect();

    score_one(lane.label(), lane.is_end_to_end(), &pairs)
}

/// The W7 boundary: which splits a caller may read. `load_corpus` is the single
/// choke point for split I/O, so every test-reading path must visibly name
/// `Corpus::Evaluation` (greppable), and a caller that asks for `Authoring`
/// cannot accidentally read `test`. This is a convention guardrail enforced at
/// the one I/O site — NOT a compile-time wall: `Evaluation` is freely
/// constructible, so it does not prevent a path from deliberately (or wrongly)
/// requesting the test split. A true structural wall would gate `Evaluation`
/// behind a separate module/feature; the decision doc's "opened once" discipline
/// is procedural, and this helper makes violations explicit rather than silent.
#[derive(Debug, Clone, Copy)]
pub enum Corpus {
    /// train + validation — for freezing rules and normalization.
    Authoring,
    /// test — opened once, never fed back into authoring.
    Evaluation,
}

impl Corpus {
    pub fn splits(self) -> &'static [&'static str] {
        match self {
            Self::Authoring => &["train", "validation"],
            Self::Evaluation => &["test"],
        }
    }
}

/// Load documents + oracle for a corpus from the prepared pilot directory.
/// Reads only the splits for the requested corpus; an `Authoring` request
/// therefore never touches `test`. (Requesting `Evaluation` is a deliberate,
/// greppable act — see `Corpus`.)
pub fn load_corpus(root: &Path, corpus: Corpus) -> anyhow::Result<(Vec<PilotDoc>, Vec<OracleRec>)> {
    let mut docs = Vec::new();
    let mut oracle = Vec::new();
    for split in corpus.splits() {
        let doc_path = root.join("documents").join(format!("{split}.jsonl"));
        let oracle_path = root.join("oracle").join(format!("{split}.jsonl"));
        let doc_text =
            std::fs::read_to_string(&doc_path).with_context(|| format!("reading {doc_path:?}"))?;
        for (n, line) in doc_text.lines().enumerate() {
            if !line.trim().is_empty() {
                docs.push(
                    serde_json::from_str::<PilotDoc>(line)
                        .with_context(|| format!("{doc_path:?} line {}", n + 1))?,
                );
            }
        }
        let oracle_text = std::fs::read_to_string(&oracle_path)
            .with_context(|| format!("reading {oracle_path:?}"))?;
        for (n, line) in oracle_text.lines().enumerate() {
            if !line.trim().is_empty() {
                oracle.push(
                    serde_json::from_str::<OracleRec>(line)
                        .with_context(|| format!("{oracle_path:?} line {}", n + 1))?,
                );
            }
        }
    }
    Ok((docs, oracle))
}

/// The measurement lane (W1). The label each lane reports (W3) is produced by
/// [`Lane::label`]; only [`Lane::OracleDiagnostic`] is not end-to-end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    Generic,
    NeuronAuthored,
    OracleDiagnostic,
}

impl Lane {
    pub fn label(self) -> &'static str {
        match self {
            Self::Generic => "generic (relation-end-to-end; entities provided)",
            Self::NeuronAuthored => "neuron-authored (relation-end-to-end; entities provided)",
            Self::OracleDiagnostic => {
                "oracle-diagnostic (DIAGNOSTIC ONLY — gold predicate supplied; NOT end-to-end)"
            }
        }
    }

    pub fn is_end_to_end(self) -> bool {
        !matches!(self, Self::OracleDiagnostic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::model::{OracleRec, PilotDoc, Triple};
    use crate::webnlg::rules::RuleSet;

    fn doc(id: &str, text: &str) -> PilotDoc {
        PilotDoc {
            document_id: id.into(),
            split: "validation".into(),
            category: "Airport".into(),
            text: text.into(),
        }
    }
    fn rec(id: &str, s: &str, p: &str, o: &str) -> OracleRec {
        OracleRec {
            document_id: id.into(),
            split: "validation".into(),
            triples: vec![Triple {
                subject: s.into(),
                predicate: p.into(),
                object: o.into(),
            }],
        }
    }

    #[test]
    fn score_documents_runs_a_lane_end_to_end() {
        let docs = vec![doc("d0", "The leader of Aarhus is Jacob Bundsgaard.")];
        let oracle = vec![rec("d0", "Aarhus", "leader", "Jacob_Bundsgaard")];
        let entities = |_id: &str| vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let s = score_documents(Lane::Generic, &docs, &oracle, entities, &RuleSet::generic());
        assert_eq!(s.lane, Lane::Generic.label());
        assert!(s.end_to_end);
        assert_eq!(s.correct, 1);
        assert_eq!(s.recall, Some(1.0));
    }

    #[test]
    fn oracle_diagnostic_never_claims_end_to_end() {
        let docs = vec![doc("d0", "Aarhus and Jacob Bundsgaard appear together.")];
        let oracle = vec![rec("d0", "Aarhus", "leader", "Jacob_Bundsgaard")];
        let entities = |_id: &str| vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let s = score_documents(
            Lane::OracleDiagnostic,
            &docs,
            &oracle,
            entities,
            &RuleSet::generic(),
        );
        assert!(!s.end_to_end, "oracle-diagnostic must never be end-to-end");
        assert!(s.lane.contains("DIAGNOSTIC"));
    }
}
