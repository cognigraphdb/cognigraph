//! Loaded WebNLG records and score structs. The loader structs deserialize the
//! prepared pilot JSONL (documents/{split}.jsonl, oracle/{split}.jsonl); they
//! deliberately accept only the fields scoring needs, so a schema addition in
//! the generator does not break loading.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One text document visible to construction (documents/{split}.jsonl).
#[derive(Debug, Clone, Deserialize)]
pub struct PilotDoc {
    pub document_id: String,
    pub split: String,
    pub category: String,
    pub text: String,
}

/// One oracle record: the triples a document was lexicalized from
/// (oracle/{split}.jsonl). Evaluation-only.
#[derive(Debug, Clone, Deserialize)]
pub struct OracleRec {
    pub document_id: String,
    pub split: String,
    pub triples: Vec<Triple>,
}

/// A raw DBpedia triple, surface strings exactly as supplied.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Triple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// Recall/precision plus partial-credit diagnostics for one lane.
#[derive(Debug, Clone, Serialize)]
pub struct WebnlgScore {
    /// Honest lane label, e.g. "generic (relation-end-to-end; entities provided)".
    pub lane: String,
    /// False if this lane received the predicate/oracle (diagnostic only).
    pub end_to_end: bool,
    pub documents: usize,
    pub oracle_total: usize,
    pub constructed_total: usize,
    pub correct: usize,
    pub recall: Option<f64>,
    pub precision: Option<f64>,
    pub diagnostics: Diagnostics,
    pub by_predicate: BTreeMap<String, PredicateScore>,
}

/// Partial-credit counts, reported alongside the headline, never inside it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Diagnostics {
    /// subject and object match, predicate wrong ("linked, mislabeled").
    pub mislabeled: usize,
    /// the {subject, object} pair is linked by some edge in either direction,
    /// predicate irrelevant. A superset of `mislabeled`.
    pub entity_pair_only: usize,
}

/// Per-predicate recall (oracle side) and precision (constructed side).
#[derive(Debug, Clone, Default, Serialize)]
pub struct PredicateScore {
    pub oracle_total: usize,
    pub constructed_total: usize,
    pub correct: usize,
}
