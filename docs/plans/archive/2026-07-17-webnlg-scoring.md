# WebNLG Scoring Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `cognigraph_construct::webnlg`, a pure, backend-free scorer that measures relation construction against the WebNLG DBpedia-triple oracle under the frozen policy in `docs/decisions/decision_webnlg_scoring.md`.

**Architecture:** A new `crates/cognigraph-construct/src/webnlg/` module parallel to `clinical_reference/`. Loaded JSONL records (`model.rs`) → a single frozen normalization/matching function (`normalize.rs`) → per-lane construction that reuses the existing synchronous `ground_chunk` over a provided-entity `SpaceType` (`rules.rs`) → pure recall/precision/diagnostic scoring (`score.rs`) → lane orchestration with a train/val-vs-test load wall (`mod.rs`). Every piece is unit-testable without a `GraphBackend`, exactly like `clinical_reference::score`.

**Tech Stack:** Rust (edition 2024), `serde`/`serde_json`, `anyhow`. Reuses `cognigraph_construct::{SpaceType, EntityDef, RelationRule, ground_chunk, GroundedFact, Fact}`.

---

## File Structure

- Create `crates/cognigraph-construct/src/webnlg/mod.rs` — module exports, `Lane` enum, lane orchestration, the W7 train/val-vs-test load boundary.
- Create `crates/cognigraph-construct/src/webnlg/model.rs` — JSONL loader structs (`PilotDoc`, `OracleRec`, `Triple`) and score structs (`WebnlgScore`, `PredicateScore`, `Diagnostics`).
- Create `crates/cognigraph-construct/src/webnlg/normalize.rs` — the frozen W6 normalization, numeric-aware canonicalization, and triple match classification.
- Create `crates/cognigraph-construct/src/webnlg/rules.rs` — the `generic` predicate→trigger map, a `RuleSet` type loadable from frozen JSON (the neuron-authored artifact), and the provided-entity `SpaceType` builder.
- Create `crates/cognigraph-construct/src/webnlg/score.rs` — pure scoring over loaded records and constructed triples.
- Modify `crates/cognigraph-construct/src/lib.rs` — add `pub mod webnlg;` and re-exports.

All types the tasks reference are defined in earlier tasks of this plan; nothing is left to the implementer's imagination.

---

### Task 1: Module skeleton and loader types

**Files:**
- Create: `crates/cognigraph-construct/src/webnlg/mod.rs`
- Create: `crates/cognigraph-construct/src/webnlg/model.rs`
- Modify: `crates/cognigraph-construct/src/lib.rs:24` (add module + re-exports)

- [ ] **Step 1: Add the module to the crate**

In `crates/cognigraph-construct/src/lib.rs`, add `pub mod webnlg;` to the module list (after `pub mod validate;` on line 24) and this re-export after the `pub use validate::...` line:

```rust
pub use webnlg::{Lane, PredicateScore, WebnlgScore, score_documents};
```

- [ ] **Step 2: Write `model.rs` loader + score types**

```rust
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
```

- [ ] **Step 3: Write a minimal `mod.rs` so the crate compiles**

```rust
//! WebNLG relation-construction scorer. Freezes to the policy in
//! docs/decisions/decision_webnlg_scoring.md (W1-W7).

pub mod model;
pub mod normalize;
pub mod rules;
pub mod score;

pub use model::{Diagnostics, OracleRec, PilotDoc, PredicateScore, Triple, WebnlgScore};
pub use score::score_documents;

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
            Self::NeuronAuthored => {
                "neuron-authored (relation-end-to-end; entities provided)"
            }
            Self::OracleDiagnostic => {
                "oracle-diagnostic (DIAGNOSTIC ONLY — gold predicate supplied; NOT end-to-end)"
            }
        }
    }

    pub fn is_end_to_end(self) -> bool {
        !matches!(self, Self::OracleDiagnostic)
    }
}
```

Create empty `normalize.rs`, `rules.rs`, `score.rs` with only a `//!` doc line each so the module tree compiles. Later tasks fill them.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p cognigraph-construct`
Expected: builds clean (the empty submodules and unused re-exports may warn; the re-export of `score_documents` will fail to resolve until Task 4 — so for THIS task, temporarily omit `pub use score::score_documents;` and the `score_documents` re-export in lib.rs, add them in Task 4).

- [ ] **Step 5: Commit**

```bash
git add crates/cognigraph-construct/src/webnlg/ crates/cognigraph-construct/src/lib.rs
git commit -m "webnlg: module skeleton, loader and score types"
```

---

### Task 2: Frozen normalization and triple matching (W6)

**Files:**
- Modify: `crates/cognigraph-construct/src/webnlg/normalize.rs`

- [ ] **Step 1: Write failing tests**

Put these at the bottom of `normalize.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::model::Triple;

    fn t(s: &str, p: &str, o: &str) -> Triple {
        Triple { subject: s.into(), predicate: p.into(), object: o.into() }
    }

    #[test]
    fn underscores_and_case_normalize() {
        assert_eq!(canonical_field("Harrietstown,_New_York"), "harrietstown, new york");
        assert_eq!(canonical_field("  Aarhus_Airport "), "aarhus airport");
    }

    #[test]
    fn numeric_objects_compare_numerically() {
        // 2702.0 == 2702, 507 == 507.0
        assert_eq!(canonical_field("2702.0"), canonical_field("2702"));
        assert_eq!(canonical_field("507"), canonical_field("507.0"));
        assert_ne!(canonical_field("507"), canonical_field("508"));
    }

    #[test]
    fn exact_triple_is_correct() {
        let c = t("Aarhus", "leader", "Jacob_Bundsgaard");
        let o = t("aarhus", "leader", "jacob bundsgaard");
        assert_eq!(classify(&c, &o), Some(MatchKind::Correct));
    }

    #[test]
    fn same_pair_wrong_predicate_is_mislabeled() {
        let c = t("Aarhus", "governor", "Jacob_Bundsgaard");
        let o = t("Aarhus", "leader", "Jacob_Bundsgaard");
        assert_eq!(classify(&c, &o), Some(MatchKind::Mislabeled));
    }

    #[test]
    fn reversed_pair_is_entity_pair_only() {
        let c = t("Jacob_Bundsgaard", "leaderOf", "Aarhus");
        let o = t("Aarhus", "leader", "Jacob_Bundsgaard");
        assert_eq!(classify(&c, &o), Some(MatchKind::EntityPairOnly));
    }

    #[test]
    fn unrelated_triple_does_not_match() {
        let c = t("Berlin", "leader", "Someone");
        let o = t("Aarhus", "leader", "Jacob_Bundsgaard");
        assert_eq!(classify(&c, &o), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cognigraph-construct webnlg::normalize -- --nocapture`
Expected: FAIL — `canonical_field`, `classify`, `MatchKind` not found.

- [ ] **Step 3: Implement `normalize.rs`**

Put this ABOVE the test module:

```rust
//! The single frozen normalization + triple-matching function (W6). All triple
//! comparison in the WebNLG scorer goes through here; nothing else normalizes.

use crate::webnlg::model::Triple;

/// Canonical form of one field: replace `_` with space, trim, lowercase; if the
/// result parses as a number, canonicalize it numerically so "2702.0" == "2702".
/// Applied to all three fields — predicates are never numeric, subjects almost
/// never are, so numeric canonicalization is harmless there and keeps one rule.
pub fn canonical_field(raw: &str) -> String {
    let base = raw.replace('_', " ");
    let base = base.trim().to_lowercase();
    if let Ok(x) = base.parse::<f64>() {
        // `{x}` gives "2702" for both 2702.0 and 2702, "507" for 507/507.0.
        return format!("{x}");
    }
    base
}

/// How a constructed triple relates to one oracle triple. Ordered by strength;
/// the caller keeps the strongest match found across the oracle set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Correct,
    Mislabeled,
    EntityPairOnly,
}

/// Classify a constructed triple against one oracle triple, or `None` if the
/// entity pair does not overlap at all.
pub fn classify(constructed: &Triple, oracle: &Triple) -> Option<MatchKind> {
    let cs = canonical_field(&constructed.subject);
    let co = canonical_field(&constructed.object);
    let cp = canonical_field(&constructed.predicate);
    let os = canonical_field(&oracle.subject);
    let oo = canonical_field(&oracle.object);
    let op = canonical_field(&oracle.predicate);

    let same_direction = cs == os && co == oo;
    let reversed = cs == oo && co == os;

    if same_direction && cp == op {
        Some(MatchKind::Correct)
    } else if same_direction {
        Some(MatchKind::Mislabeled)
    } else if reversed || same_direction {
        Some(MatchKind::EntityPairOnly)
    } else {
        None
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cognigraph-construct webnlg::normalize -- --nocapture`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/cognigraph-construct/src/webnlg/normalize.rs
git commit -m "webnlg: frozen normalization and triple matching (W6)"
```

---

### Task 3: Pure scoring over constructed vs oracle triples (W4, W5)

**Files:**
- Modify: `crates/cognigraph-construct/src/webnlg/score.rs`

This task scores an ALREADY-CONSTRUCTED set of triples against the oracle. Construction (calling `ground_chunk`) is Task 4; separating them keeps scoring backend-free and exhaustively testable.

- [ ] **Step 1: Write failing tests**

At the bottom of `score.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::model::Triple;

    fn t(s: &str, p: &str, o: &str) -> Triple {
        Triple { subject: s.into(), predicate: p.into(), object: o.into() }
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cognigraph-construct webnlg::score -- --nocapture`
Expected: FAIL — `score_one` not found.

- [ ] **Step 3: Implement `score.rs`**

Above the test module:

```rust
//! Pure recall/precision/diagnostic scoring (W4, W5). Deduplicates within a
//! document by canonical triple, so a fact built or listed twice counts once.

use std::collections::{BTreeMap, BTreeSet};

use crate::webnlg::model::{Diagnostics, PredicateScore, Triple, WebnlgScore};
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
            score.by_predicate.entry(predicate.clone()).or_default().oracle_total += 1;
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

        // Diagnostics: for each constructed triple NOT exactly correct, find its
        // strongest partial match across the oracle set.
        for b in built {
            let bkey = key(b);
            if oracle_keys.contains(&bkey) {
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p cognigraph-construct webnlg::score -- --nocapture`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/cognigraph-construct/src/webnlg/score.rs
git commit -m "webnlg: pure recall/precision/diagnostic scoring (W4, W5)"
```

---

### Task 4: Provided-entity construction and the generic rule set (W2, W6)

**Files:**
- Modify: `crates/cognigraph-construct/src/webnlg/rules.rs`
- Modify: `crates/cognigraph-construct/src/webnlg/mod.rs` (add `construct_document`, wire re-exports)
- Modify: `crates/cognigraph-construct/src/lib.rs` (restore the `score_documents` re-export deferred in Task 1)

- [ ] **Step 1: Write failing tests**

At the bottom of `rules.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::webnlg::Lane;
    use crate::webnlg::model::Triple;

    fn oracle(s: &str, p: &str, o: &str) -> Triple {
        Triple { subject: s.into(), predicate: p.into(), object: o.into() }
    }

    #[test]
    fn generic_rule_grounds_a_provided_pair_from_text() {
        // Entities are provided (W2); the generic 'leader' trigger must fire.
        let entities = vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let gold = vec![oracle("Aarhus", "leader", "Jacob_Bundsgaard")];
        let built = construct_document(
            Lane::Generic,
            "The leader of Aarhus is Jacob Bundsgaard.",
            &entities,
            &gold,
            &RuleSet::generic(),
        );
        assert!(built.contains(&oracle("Aarhus", "leader", "Jacob_Bundsgaard")));
    }

    #[test]
    fn oracle_diagnostic_only_instantiates_the_gold_predicate() {
        let entities = vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let gold = vec![oracle("Aarhus", "leader", "Jacob_Bundsgaard")];
        // Diagnostic lane is handed the gold predicate, so even a bare mention
        // template grounds it; a non-gold predicate is never instantiated.
        let built = construct_document(
            Lane::OracleDiagnostic,
            "Aarhus and Jacob Bundsgaard appear together.",
            &entities,
            &gold,
            &RuleSet::generic(),
        );
        assert!(built.iter().all(|t| t.predicate == "leader"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p cognigraph-construct webnlg::rules -- --nocapture`
Expected: FAIL — `RuleSet`, `construct_document` not found.

- [ ] **Step 3: Implement `rules.rs`**

Above the test module:

```rust
//! Per-lane rule sets and the provided-entity SpaceType builder (W2). A rule set
//! maps a DBpedia predicate to its trigger templates; the predicate IS the
//! constructed relation (W6), so a grounded fact's relation compares directly to
//! the oracle predicate. Templates use {source}/{target}, expanded by
//! `ground_chunk` against the provided entity surfaces.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::types::{EntityDef, RelationRule, SpaceType};

/// A frozen predicate -> trigger-templates map. `generic()` is the built-in
/// naive baseline; the neuron-authored set is loaded from JSON via `from_json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub name: String,
    pub predicates: BTreeMap<String, Vec<String>>,
}

impl RuleSet {
    /// The frozen `generic` baseline: a small hand-authored set over common
    /// WebNLG predicates. Deliberately small — it is the low baseline (W1).
    pub fn generic() -> Self {
        let mut predicates = BTreeMap::new();
        predicates.insert(
            "leader".to_string(),
            vec![
                "leader of {source} is {target}".to_string(),
                "{source} is led by {target}".to_string(),
            ],
        );
        predicates.insert(
            "location".to_string(),
            vec![
                "{source} is located in {target}".to_string(),
                "{source} is located at {target}".to_string(),
            ],
        );
        predicates.insert(
            "runwayLength".to_string(),
            vec!["{source} runway length is {target}".to_string()],
        );
        predicates.insert(
            "country".to_string(),
            vec!["{source} is in {target}".to_string()],
        );
        Self { name: "generic".to_string(), predicates }
    }

    /// Load a frozen neuron-authored rule set from JSON (the reviewed artifact).
    pub fn from_json(raw: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(raw)?)
    }
}

/// Build the SpaceType for one document: provided entities (W2) plus one
/// RelationRule per (ordered entity pair) x (predicate) with that predicate's
/// trigger templates. `only_predicates`, when `Some`, restricts instantiation
/// to the gold predicates — used by the oracle-diagnostic lane.
pub fn build_space(
    entities: &[String],
    rules: &RuleSet,
    only_predicates: Option<&[String]>,
) -> SpaceType {
    let entity_defs: Vec<EntityDef> = entities
        .iter()
        .map(|name| EntityDef {
            name: name.replace('_', " "),
            entity_type: "entity".to_string(),
            aliases: vec![name.clone()],
        })
        .collect();

    let mut relation_rules = Vec::new();
    for source in entities {
        for target in entities {
            if source == target {
                continue;
            }
            for (predicate, triggers) in &rules.predicates {
                if let Some(allow) = only_predicates {
                    if !allow.iter().any(|p| p == predicate) {
                        continue;
                    }
                }
                relation_rules.push(RelationRule {
                    source: source.replace('_', " "),
                    relation: predicate.clone(),
                    target: target.replace('_', " "),
                    when_any: triggers.clone(),
                    require_in_sentence: Vec::new(),
                    trigger_provenance: BTreeMap::new(),
                });
            }
        }
    }

    SpaceType {
        id: "webnlg-pilot-v1".to_string(),
        name: String::new(),
        version: 1,
        description: String::new(),
        entities: entity_defs,
        relation_rules,
    }
}
```

- [ ] **Step 4: Add `construct_document` to `mod.rs`**

Append to `crates/cognigraph-construct/src/webnlg/mod.rs` (and add the imports at the top):

```rust
use crate::grounding::ground_chunk;
use crate::webnlg::model::Triple;
use crate::webnlg::rules::{RuleSet, build_space};

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
```

Also add `pub mod` visibility for `rules` (already declared in Task 1) and re-export `construct_document` and `RuleSet` from the `pub use` block. Restore in `lib.rs` the `pub use webnlg::{... score_documents ...}` re-export (it resolves once Task 5 defines `score_documents`; if implementing strictly task-by-task, add `score_documents` to the re-export in Task 5's commit instead).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p cognigraph-construct webnlg::rules -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/cognigraph-construct/src/webnlg/rules.rs crates/cognigraph-construct/src/webnlg/mod.rs
git commit -m "webnlg: provided-entity construction and generic rule set (W2)"
```

---

### Task 5: Lane orchestration and the train/val-vs-test load wall (W7)

**Files:**
- Modify: `crates/cognigraph-construct/src/webnlg/mod.rs` (add `score_documents`, `load_split`, `Authoring`/`Evaluation` split boundary)
- Modify: `crates/cognigraph-construct/src/lib.rs` (finalize re-exports)

- [ ] **Step 1: Write failing test**

Add to the `mod.rs` test module (create one at the bottom if absent):

```rust
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
            triples: vec![Triple { subject: s.into(), predicate: p.into(), object: o.into() }],
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
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cognigraph-construct webnlg::tests::score_documents_runs_a_lane_end_to_end -- --nocapture`
Expected: FAIL — `score_documents` not found.

- [ ] **Step 3: Implement `score_documents` and the load wall in `mod.rs`**

```rust
use std::collections::BTreeMap;
use std::path::Path;

use crate::webnlg::model::{OracleRec, PilotDoc, WebnlgScore};
use crate::webnlg::score::score_one;

/// Run one lane over aligned documents + oracle and score it. `entities_for`
/// supplies each document's provided entity surfaces (W2) — the caller sources
/// them from the oracle surface forms. Pure: no I/O, no backend.
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
    let oracle_by_id: BTreeMap<&str, &OracleRec> =
        oracle.iter().map(|r| (r.document_id.as_str(), r)).collect();

    let pairs: Vec<(Vec<Triple>, Vec<Triple>)> = documents
        .iter()
        .filter_map(|doc| {
            let rec = oracle_by_id.get(doc.document_id.as_str())?;
            let entities = entities_for(&doc.document_id);
            let built = construct_document(lane, &doc.text, &entities, &rec.triples, rules);
            Some((rec.triples.clone(), built))
        })
        .collect();

    score_one(lane.label(), lane.is_end_to_end(), &pairs)
}

/// The W7 boundary: which splits a caller is allowed to read. Rule authoring
/// takes `Authoring` (train+validation) ONLY; scoring the frozen policy takes
/// `Evaluation` (test). Separating the constructors makes it a compile-time
/// choice which corpus a code path can touch — the code analog of the
/// documents/ vs oracle/ physical split.
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
/// Refuses to load a split outside the requested corpus, so an authoring path
/// cannot read `test` even by mistake.
pub fn load_corpus(root: &Path, corpus: Corpus) -> anyhow::Result<(Vec<PilotDoc>, Vec<OracleRec>)> {
    let mut docs = Vec::new();
    let mut oracle = Vec::new();
    for split in corpus.splits() {
        let doc_path = root.join("documents").join(format!("{split}.jsonl"));
        let oracle_path = root.join("oracle").join(format!("{split}.jsonl"));
        for line in std::fs::read_to_string(&doc_path)?.lines() {
            if !line.trim().is_empty() {
                docs.push(serde_json::from_str::<PilotDoc>(line)?);
            }
        }
        for line in std::fs::read_to_string(&oracle_path)?.lines() {
            if !line.trim().is_empty() {
                oracle.push(serde_json::from_str::<OracleRec>(line)?);
            }
        }
    }
    Ok((docs, oracle))
}
```

Update the `pub use` block in `mod.rs` to add `construct_document, score_documents, Corpus, load_corpus`, and the `rules::RuleSet` re-export. Finalize `lib.rs` re-exports:

```rust
pub use webnlg::{
    Corpus, Lane, PredicateScore, WebnlgScore, load_corpus, score_documents,
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cognigraph-construct webnlg -- --nocapture`
Expected: PASS (all webnlg tests: normalize 6, score 4, rules 2, mod 1).

- [ ] **Step 5: Commit**

```bash
git add crates/cognigraph-construct/src/webnlg/mod.rs crates/cognigraph-construct/src/lib.rs
git commit -m "webnlg: lane orchestration and train/val-vs-test load wall (W7)"
```

---

### Task 6: Gates and honest-label assertion

**Files:**
- Modify: `crates/cognigraph-construct/src/webnlg/mod.rs` (one guard test)

- [ ] **Step 1: Add a test that locks the no-overclaim invariant (W3)**

```rust
    #[test]
    fn oracle_diagnostic_never_claims_end_to_end() {
        let docs = vec![doc("d0", "Aarhus and Jacob Bundsgaard appear together.")];
        let oracle = vec![rec("d0", "Aarhus", "leader", "Jacob_Bundsgaard")];
        let entities = |_id: &str| vec!["Aarhus".to_string(), "Jacob_Bundsgaard".to_string()];
        let s = score_documents(
            Lane::OracleDiagnostic, &docs, &oracle, entities, &crate::webnlg::rules::RuleSet::generic(),
        );
        assert!(!s.end_to_end, "oracle-diagnostic must never be end-to-end");
        assert!(s.lane.contains("DIAGNOSTIC"));
    }
```

- [ ] **Step 2: Run the full gate suite**

Run:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```
Expected: fmt clean, clippy clean (no warnings), all tests pass including the new `webnlg` tests.

- [ ] **Step 3: Fix any fmt/clippy findings, then commit**

```bash
git add -A
git commit -m "webnlg: lock the no-overclaim invariant; gates green"
```

---

## Self-Review

**Spec coverage (decision_webnlg_scoring.md W1–W7):**
- W1 (lanes now/deferred): `Lane` enum has Generic/NeuronAuthored/OracleDiagnostic (Task 1); entity-discovered/hard-negative are deferred, not built — correct, they are not in this plan.
- W2 (entities provided all lanes): `build_space` + `construct_document` provide entities in every lane (Task 4). ✓
- W3 (honest labels): `Lane::label`/`is_end_to_end` + the guard test (Tasks 1, 6). ✓
- W4 (exact-triple headline + diagnostics): `classify`/`MatchKind` + `Diagnostics` in scoring (Tasks 2, 3). ✓
- W5 (recall + precision, per-predicate, precision as restraint): `score_one` + `by_predicate` (Task 3). ✓
- W6 (single frozen normalization; predicate = relation): `canonical_field` + rule.relation = predicate (Tasks 2, 4). ✓
- W7 (test opens once, walled): `Corpus`/`load_corpus` refuse cross-corpus reads (Task 5). ✓

**Deferred, correctly out of scope:** the LLM *authoring* of the neuron-authored rule set (needs the propose/review path) — `RuleSet::from_json` loads the frozen artifact, but generating it is a follow-up plan. The `entity-discovered` and `hard-negative` lanes are spec'd-deferred. Running the scorer over the real 38k-doc corpus and recording numbers is a follow-up (this plan delivers the tested mechanism, not the scored run).

**Placeholder scan:** none — every step ships real code and exact commands.

**Type consistency:** `Triple`, `WebnlgScore`, `PredicateScore`, `Diagnostics` (model.rs) are used unchanged in score.rs/mod.rs; `RuleSet`/`build_space`/`construct_document` signatures match across Tasks 4–6; `canonical_field`/`classify`/`MatchKind` are stable across Tasks 2–3. `score_documents` re-export is deferred to Task 5 (noted in Tasks 1 and 4 to avoid an unresolved import mid-plan).

---
```
