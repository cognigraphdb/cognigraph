//! One-shot WebNLG TEST evaluation (W7). Opens the held-out `test` oracle
//! EXACTLY ONCE and scores the FROZEN committed neuron-authored ruleset (mined
//! on train+validation) alongside the `generic` and `oracle-diagnostic` lanes.
//!
//! The frozen policy (`decision_webnlg_scoring.md`, W1-W7) and the ruleset were
//! fixed BEFORE this ran. `test` never fed mining; only validation informed the
//! mining hyperparameters (standard dev-set tuning). Per W7 the test oracle is
//! opened once — re-running this against a changed ruleset invalidates the
//! result and must be recorded as such. This binary is the only one that names
//! `Corpus::Evaluation`.
//!
//! Usage:
//!   cargo run --release -p cognigraph-construct --bin webnlg-test -- \
//!     [--root data/webnlg-pilot] [--ruleset <path>]

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use cognigraph_construct::webnlg::rules::RuleSet;
use cognigraph_construct::webnlg::{Corpus, Lane, WebnlgScore, load_corpus, score_documents};

fn main() -> Result<()> {
    let mut root = PathBuf::from("data/webnlg-pilot");
    let mut ruleset_path =
        PathBuf::from("crates/cognigraph-construct/fixtures/webnlg/neuron-ruleset.mined.json");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                root = PathBuf::from(args.next().ok_or_else(|| anyhow!("--root needs a value"))?)
            }
            "--ruleset" => {
                ruleset_path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow!("--ruleset needs a value"))?,
                )
            }
            "-h" | "--help" => {
                println!("One-shot WebNLG test evaluation of the frozen neuron ruleset (W7).");
                println!("Usage: ... --bin webnlg-test -- [--root DIR] [--ruleset FILE]");
                return Ok(());
            }
            other => return Err(anyhow!("unknown argument '{other}'")),
        }
    }

    let generic = RuleSet::generic();
    let neuron = RuleSet::from_json(
        &std::fs::read_to_string(&ruleset_path)
            .with_context(|| format!("reading frozen ruleset {ruleset_path:?}"))?,
    )
    .with_context(|| format!("parsing frozen ruleset {ruleset_path:?}"))?;

    // W7: the one and only `Evaluation` read. Everything above is frozen.
    let (test_docs, test_oracle) = load_corpus(&root, Corpus::Evaluation)
        .with_context(|| format!("loading TEST corpus from {root:?}"))?;

    // W2: entities provided per document from its oracle surface forms.
    let mut entities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rec in &test_oracle {
        let list = entities.entry(rec.document_id.clone()).or_default();
        for triple in &rec.triples {
            list.push(triple.subject.clone());
            list.push(triple.object.clone());
        }
        list.sort();
        list.dedup();
    }
    let entities_for = |id: &str| entities.get(id).cloned().unwrap_or_default();

    println!("=== WebNLG ONE-SHOT TEST (W7) ===");
    println!(
        "frozen ruleset: {} ({} predicates)",
        neuron.name,
        neuron.predicates.len()
    );
    println!("test split: {} documents\n", test_docs.len());

    for (label, lane, rules) in [
        ("generic", Lane::Generic, &generic),
        ("neuron-authored (frozen)", Lane::NeuronAuthored, &neuron),
        ("oracle-diagnostic", Lane::OracleDiagnostic, &neuron),
    ] {
        let score = score_documents(lane, &test_docs, &test_oracle, entities_for, rules);
        print_score(label, &score);
    }
    Ok(())
}

fn print_score(label: &str, s: &WebnlgScore) {
    println!("=== {label} ===");
    println!(
        "  docs {}  oracle {}  constructed {}  correct {}",
        s.documents, s.oracle_total, s.constructed_total, s.correct
    );
    println!(
        "  recall {}   precision {}   (mislabeled {}, entity-pair-only {})\n",
        pct(s.recall),
        pct(s.precision),
        s.diagnostics.mislabeled,
        s.diagnostics.entity_pair_only
    );
}

fn pct(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.1}%", x * 100.0),
        None => "—".to_string(),
    }
}
