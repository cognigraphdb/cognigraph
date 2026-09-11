//! Lane scoring: recall, precision, and partial-credit diagnostics (W2-W7).
//!
//! Pure recall/precision/diagnostic scoring (W4, W5). Deduplicates within a
//! document by canonical triple, so a fact built or listed twice counts once.

use std::collections::{BTreeMap, BTreeSet};

use crate::webnlg::model::{Diagnostics, Triple, WebnlgScore};
use crate::webnlg::normalize::{MatchKind, canonical_field, classify};

/// A canonical triple key: three canonicalized fields, hashable and set-safe.
fn key(t: &Triple) -> (String, String, String) {
    (
        canonical_field(&t.subject),
        canonical_field(&t.predicate),
        canonical_field(&t.object),
    )
}

/// Score a lane over `(oracle, constructed)` pairs — one pair per document.
/// `label`/`end_to_end` come from the [`crate::webnlg::Lane`] the caller ran.
pub fn score_one(
    label: &str,
    end_to_end: bool,
    docs: &[(Vec<Triple>, Vec<Triple>)],
) -> WebnlgScore {
    let mut score = WebnlgScore {
        lane: label.to_string(),
        end_to_end,
        documents: docs.len(),
        oracle_total: 0,
        constructed_total: 0,
        correct: 0,
        recall: None,
        precision: None,
        diagnostics: Diagnostics::default(),
        by_predicate: BTreeMap::new(),
    };

    for (oracle, built) in docs {
        // Distinct triples per document (W4-style distinctness).
        let oracle_keys: BTreeSet<_> = oracle.iter().map(key).collect();
        let built_keys: BTreeSet<_> = built.iter().map(key).collect();

        // Per-predicate oracle totals (recall denominator).
        for (_, predicate, _) in &oracle_keys {
            score
                .by_predicate
                .entry(predicate.clone())
                .or_default()
                .oracle_total += 1;
        }
        // Per-predicate constructed totals (precision denominator).
        for (_, predicate, _) in &built_keys {
            score
                .by_predicate
                .entry(predicate.clone())
                .or_default()
                .constructed_total += 1;
        }

        score.oracle_total += oracle_keys.len();
        score.constructed_total += built_keys.len();

        // Correct = exact triple intersection.
        for tkey in built_keys.intersection(&oracle_keys) {
            score.correct += 1;
            score.by_predicate.get_mut(&tkey.1).unwrap().correct += 1;
        }

        // Diagnostics: for each DISTINCT constructed triple NOT exactly correct,
        // find its strongest partial match across the oracle set. Dedup by key
        // so a triple emitted from two chunks of one document counts once, the
        // same distinctness the headline uses.
        let mut seen = BTreeSet::new();
        for b in built {
            let bkey = key(b);
            if oracle_keys.contains(&bkey) || !seen.insert(bkey) {
                continue;
            }
            let mut best: Option<MatchKind> = None;
            for o in oracle {
                if let Some(kind) = classify(b, o) {
                    best = Some(match (best, kind) {
                        (Some(MatchKind::Mislabeled), _) | (_, MatchKind::Mislabeled) => {
                            MatchKind::Mislabeled
                        }
                        _ => MatchKind::EntityPairOnly,
                    });
                }
            }
            match best {
                Some(MatchKind::Mislabeled) => {
                    score.diagnostics.mislabeled += 1;
                    score.diagnostics.entity_pair_only += 1; // superset
                }
                Some(MatchKind::EntityPairOnly) => score.diagnostics.entity_pair_only += 1,
                _ => {}
            }
        }
    }

    score.recall = ratio(score.correct, score.oracle_total);
    score.precision = ratio(score.correct, score.constructed_total);
    score
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::model::Triple;

    fn t(s: &str, p: &str, o: &str) -> Triple {
        Triple {
            subject: s.into(),
            predicate: p.into(),
            object: o.into(),
        }
    }

    #[test]
    fn recall_and_precision_on_a_clean_hit() {
        let oracle = vec![t("Aarhus", "leader", "Jacob_Bundsgaard")];
        let built = vec![t("Aarhus", "leader", "Jacob_Bundsgaard")];
        let s = score_one("generic", true, &[(oracle, built)]);
        assert_eq!(s.correct, 1);
        assert_eq!(s.recall, Some(1.0));
        assert_eq!(s.precision, Some(1.0));
    }

    #[test]
    fn off_oracle_triple_is_a_false_edge_costing_precision() {
        let oracle = vec![t("Aarhus", "leader", "Jacob_Bundsgaard")];
        // one correct, one hallucinated
        let built = vec![
            t("Aarhus", "leader", "Jacob_Bundsgaard"),
            t("Aarhus", "country", "Denmark"),
        ];
        let s = score_one("generic", true, &[(oracle, built)]);
        assert_eq!(s.correct, 1);
        assert_eq!(s.recall, Some(1.0));
        assert_eq!(s.precision, Some(0.5)); // 1 correct / 2 constructed
    }

    #[test]
    fn mislabeled_is_a_diagnostic_not_a_correct() {
        let oracle = vec![t("Aarhus", "leader", "Jacob_Bundsgaard")];
        let built = vec![t("Aarhus", "governor", "Jacob_Bundsgaard")];
        let s = score_one("generic", true, &[(oracle, built)]);
        assert_eq!(s.correct, 0);
        assert_eq!(s.recall, Some(0.0));
        assert_eq!(s.precision, Some(0.0));
        assert_eq!(s.diagnostics.mislabeled, 1);
        assert_eq!(s.diagnostics.entity_pair_only, 1); // superset
    }

    #[test]
    fn per_predicate_breakdown_splits_recall_and_precision() {
        let oracle = vec![
            t("Aarhus", "leader", "Jacob_Bundsgaard"),
            t("Aarhus_Airport", "runwayLength", "2702.0"),
        ];
        let built = vec![t("Aarhus_Airport", "runwayLength", "2702")];
        let s = score_one("generic", true, &[(oracle, built)]);
        assert_eq!(s.by_predicate["runwaylength"].correct, 1);
        assert_eq!(s.by_predicate["runwaylength"].oracle_total, 1);
        assert_eq!(s.by_predicate["leader"].oracle_total, 1);
        assert_eq!(s.by_predicate["leader"].correct, 0);
    }

    #[test]
    fn duplicate_false_edge_counts_once() {
        let oracle = vec![t("Aarhus", "leader", "Jacob_Bundsgaard")];
        // same hallucinated triple emitted twice (e.g. from two chunks)
        let built = vec![
            t("Aarhus", "governor", "Jacob_Bundsgaard"),
            t("Aarhus", "governor", "Jacob_Bundsgaard"),
        ];
        let s = score_one("generic", true, &[(oracle, built)]);
        assert_eq!(s.constructed_total, 1); // headline dedups
        assert_eq!(s.diagnostics.mislabeled, 1); // diagnostics now dedup too
        assert_eq!(s.diagnostics.entity_pair_only, 1);
    }

    #[test]
    fn totals_accumulate_across_documents() {
        let d0 = (
            vec![t("Aarhus", "leader", "Jacob_Bundsgaard")],
            vec![t("Aarhus", "leader", "Jacob_Bundsgaard")],
        );
        let d1 = (
            vec![t("Aarhus_Airport", "runwayLength", "2702.0")],
            vec![t("Aarhus_Airport", "runwayLength", "2702")],
        );
        let s = score_one("generic", true, &[d0, d1]);
        assert_eq!(s.documents, 2);
        assert_eq!(s.oracle_total, 2);
        assert_eq!(s.correct, 2);
        assert_eq!(s.recall, Some(1.0));
    }

    #[test]
    fn empty_oracle_and_empty_built_give_none_ratios() {
        // empty oracle -> recall None; empty built -> precision None
        let no_oracle = score_one("generic", true, &[(vec![], vec![t("A", "r", "B")])]);
        assert_eq!(no_oracle.recall, None);
        let no_built = score_one("generic", true, &[(vec![t("A", "r", "B")], vec![])]);
        assert_eq!(no_built.precision, None);
    }
}
