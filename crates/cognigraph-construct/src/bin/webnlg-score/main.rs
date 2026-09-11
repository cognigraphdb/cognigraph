//! Score the WebNLG lanes on the validation split and print a per-lane +
//! per-predicate comparison.
//!
//! To keep the neuron-authored number an honest GENERALIZATION estimate rather
//! than in-sample, the neuron ruleset is mined from `train` ONLY and scored on
//! the held-out `validation` split. (The committed artifact is mined from
//! train+validation per policy W1; this tool re-mines train-only purely for
//! measurement.) The `test` split is never touched here — that opens once,
//! against the frozen policy (W7).
//!
//! Usage:
//!   cargo run --release -p cognigraph-construct --bin webnlg-score -- \
//!     [--root data/webnlg-pilot]

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use cognigraph_construct::webnlg::mining::{MineOpts, mine_ruleset};
use cognigraph_construct::webnlg::rules::RuleSet;
use cognigraph_construct::webnlg::{
    Corpus, CorpusProposer, Lane, OracleRec, PilotDoc, WebnlgScore, load_corpus, propose_ruleset,
    score_documents, score_documents_fuzzy,
};

fn main() -> Result<()> {
    let mut root = PathBuf::from("data/webnlg-pilot");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => {
                root = PathBuf::from(args.next().ok_or_else(|| anyhow!("--root needs a value"))?)
            }
            "-h" | "--help" => {
                println!("Score WebNLG lanes on validation (neuron ruleset mined train-only).");
                println!("Usage: ... --bin webnlg-score -- [--root data/webnlg-pilot]");
                return Ok(());
            }
            other => return Err(anyhow!("unknown argument '{other}'")),
        }
    }

    // W7: authoring corpus only. Split it into train (for mining) and the
    // held-out validation (for scoring).
    let (documents, oracle) = load_corpus(&root, Corpus::Authoring)
        .with_context(|| format!("loading authoring corpus from {root:?}"))?;
    let train_docs: Vec<PilotDoc> = documents
        .iter()
        .filter(|d| d.split == "train")
        .cloned()
        .collect();
    let train_oracle: Vec<OracleRec> = oracle
        .iter()
        .filter(|r| r.split == "train")
        .cloned()
        .collect();
    let val_docs: Vec<PilotDoc> = documents
        .into_iter()
        .filter(|d| d.split == "validation")
        .collect();
    let val_oracle: Vec<OracleRec> = oracle
        .into_iter()
        .filter(|r| r.split == "validation")
        .collect();

    let generic = RuleSet::generic();
    // Default mining opts = disambiguated, recall-tuned (min_count=1, cap=20).
    let neuron = mine_ruleset(
        "neuron-train",
        &train_docs,
        &train_oracle,
        &MineOpts::default(),
    );
    println!(
        "corpus: {} train docs (mining), {} validation docs (scoring)",
        train_docs.len(),
        val_docs.len()
    );
    println!(
        "neuron ruleset (train-only, default opts): {} predicates\n",
        neuron.predicates.len()
    );

    // W2: entities provided per document from its oracle surface forms.
    let mut entities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for rec in &val_oracle {
        let list = entities.entry(rec.document_id.clone()).or_default();
        for triple in &rec.triples {
            list.push(triple.subject.clone());
            list.push(triple.object.clone());
        }
        list.sort();
        list.dedup();
    }
    let entities_for = |id: &str| entities.get(id).cloned().unwrap_or_default();

    // The proposal loop's deterministic proposer: templatize single-triple docs
    // PLUS multi-triple sentences with an unambiguous triple (richer precise
    // templates; the LLM proposer is the gated scale-up).
    let neuron_proposed = propose_ruleset(
        "neuron-proposed-train",
        &train_docs,
        &train_oracle,
        &CorpusProposer,
        &MineOpts::default(),
    );
    println!(
        "proposed ruleset (corpus proposer, single + multi-triple): {} predicates\n",
        neuron_proposed.predicates.len()
    );

    for (label, lane, rules) in [
        ("generic", Lane::Generic, &generic),
        (
            "neuron-authored (exact, baseline)",
            Lane::NeuronAuthored,
            &neuron,
        ),
        (
            "neuron-authored (exact, CorpusProposer)",
            Lane::NeuronAuthored,
            &neuron_proposed,
        ),
        (
            "oracle-diagnostic (exact, CorpusProposer)",
            Lane::OracleDiagnostic,
            &neuron_proposed,
        ),
    ] {
        let score = score_documents(lane, &val_docs, &val_oracle, entities_for, rules);
        print_score(label, &score);
    }

    // Recall-frontier experiment: the looser fuzzy matcher, swept over the
    // max-gap knob (held-out validation only; the one-shot test remains spent, W7).
    for gap in [0usize, 1, 2, 3] {
        let score = score_documents_fuzzy(
            Lane::NeuronAuthored,
            &val_docs,
            &val_oracle,
            entities_for,
            &neuron,
            gap,
        );
        print_score(&format!("neuron-authored (FUZZY, max_gap={gap})"), &score);
    }
    Ok(())
}

fn print_score(label: &str, s: &WebnlgScore) {
    println!("=== {label} ===");
    println!("  ({})", s.lane);
    println!(
        "  docs {}  oracle {}  constructed {}  correct {}",
        s.documents, s.oracle_total, s.constructed_total, s.correct
    );
    println!(
        "  recall {}   precision {}   (mislabeled {}, entity-pair-only {})",
        pct(s.recall),
        pct(s.precision),
        s.diagnostics.mislabeled,
        s.diagnostics.entity_pair_only
    );

    // The predicates that ground the most edges but least accurately — the
    // review queue for pruning over-broad templates.
    let mut over: Vec<(&String, f64, usize, usize)> = s
        .by_predicate
        .iter()
        .filter(|(_, p)| p.constructed_total >= 10)
        .map(|(name, p)| {
            let precision = if p.constructed_total == 0 {
                1.0
            } else {
                p.correct as f64 / p.constructed_total as f64
            };
            (name, precision, p.correct, p.constructed_total)
        })
        .collect();
    over.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(b.3.cmp(&a.3)));
    if !over.is_empty() {
        println!("  lowest-precision predicates (constructed >= 10):");
        for (name, precision, correct, constructed) in over.iter().take(6) {
            println!(
                "    {name:<28} precision {:.2}  ({correct}/{constructed})",
                precision
            );
        }
    }
    println!();
}

fn pct(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.1}%", x * 100.0),
        None => "—".to_string(),
    }
}
