//! LIVE LLM run of the template-proposal loop (the recall lever). Proposes
//! precise `{source}`/`{target}` trigger templates for the highest-frequency
//! validation predicates via a real model, MERGES them into the train-mined
//! baseline, and measures the recall/precision effect on the held-out
//! `validation` split with the exact matcher.
//!
//! This makes outward, paid, NON-DETERMINISTIC calls (unlike the rest of the
//! deterministic pilot), so it is a separate opt-in binary, run only with a
//! provider configured. `test` is never touched (W7): templates are proposed
//! from `train` examples and measured on `validation`. Every proposed template
//! is a candidate for human review; nothing here freezes an artifact.
//!
//! Usage (needs OPENAI_API_KEY + COGNIGRAPH_COMPLETION_MODEL, e.g. via .env):
//!   cargo run --release -p cognigraph-construct --bin webnlg-llm-run -- \
//!     [--root data/webnlg-pilot] [--top 25] [--out <proposals.json>]

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use cognigraph_construct::webnlg::mining::MineOpts;
use cognigraph_construct::webnlg::propose::{
    gather_examples, llm_prompt, merge_and_disambiguate, valid_template,
};
use cognigraph_construct::webnlg::{
    Corpus, Lane, OracleRec, PilotDoc, load_corpus, mine_ruleset, score_documents,
};
use cognigraph_embeddings::completion::completion_from_env;
use serde_json::{Value, json};

#[tokio::main]
async fn main() -> Result<()> {
    let mut root = PathBuf::from("data/webnlg-pilot");
    let mut top = 25usize;
    let mut out = PathBuf::from("crates/cognigraph-construct/fixtures/webnlg/llm-proposals.json");
    let mut load: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or_else(|| anyhow!("--root value"))?),
            "--top" => top = args.next().ok_or_else(|| anyhow!("--top value"))?.parse()?,
            "--out" => out = PathBuf::from(args.next().ok_or_else(|| anyhow!("--out value"))?),
            // Re-measure saved proposals without calling the LLM again.
            "--load" => {
                load = Some(PathBuf::from(
                    args.next().ok_or_else(|| anyhow!("--load value"))?,
                ))
            }
            "-h" | "--help" => {
                println!("Live LLM template-proposal run over the top-N validation predicates.");
                return Ok(());
            }
            other => return Err(anyhow!("unknown argument '{other}'")),
        }
    }

    dotenvy::dotenv().ok();

    // W7: authoring corpus only, split train (examples/mining) vs validation (scoring).
    let (documents, oracle) = load_corpus(&root, Corpus::Authoring)?;
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

    let baseline = mine_ruleset(
        "neuron-train",
        &train_docs,
        &train_oracle,
        &MineOpts::default(),
    );
    let examples = gather_examples(&train_docs, &train_oracle);

    // Target the predicates that account for the most validation triples (max
    // recall headroom) and that we have train examples to prompt from.
    let mut freq: BTreeMap<String, usize> = BTreeMap::new();
    for r in &val_oracle {
        for t in &r.triples {
            *freq.entry(t.predicate.clone()).or_default() += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = freq
        .into_iter()
        .filter(|(p, _)| examples.contains_key(p))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.truncate(top);

    // Proposals: either loaded from a saved file (--load, no LLM call) or
    // produced live by the model over the top-N predicates.
    let proposals_log: BTreeMap<String, Vec<String>> = if let Some(path) = &load {
        eprintln!("loading proposals from {}", path.display());
        serde_json::from_str(&std::fs::read_to_string(path)?)?
    } else {
        let schema = json!({
            "type": "object",
            "properties": { "templates": { "type": "array", "items": { "type": "string" } } },
            "required": ["templates"],
            "additionalProperties": false
        });
        let system = "You write precise trigger templates for a knowledge-graph relation. \
Each template uses {source} for the subject and {target} for the object and must \
contain a real connective phrase. Return ONLY the JSON object.";
        let provider =
            completion_from_env().context("no completion provider (need OPENAI_API_KEY)")?;
        eprintln!("provider model: {}", provider.model_name());
        let mut log: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (i, (predicate, vfreq)) in ranked.iter().enumerate() {
            let sample: Vec<_> = examples[predicate].iter().take(12).cloned().collect();
            let user = llm_prompt(predicate, &sample);
            eprint!(
                "[{}/{}] {predicate} (val {vfreq}) ... ",
                i + 1,
                ranked.len()
            );
            let value: Value = match provider.complete_json(system, &user, &schema).await {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("skipped ({e})");
                    continue;
                }
            };
            let proposed: Vec<String> = value["templates"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| valid_template(s))
                        .collect()
                })
                .unwrap_or_default();
            eprintln!("{} valid templates", proposed.len());
            log.insert(predicate.clone(), proposed);
        }
        // Save the raw proposals for human review (a non-deterministic snapshot).
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out, format!("{}\n", serde_json::to_string_pretty(&log)?))?;
        eprintln!("wrote proposals to {}", out.display());
        log
    };

    // Two merges: RAW (append proposals, no review) and REVIEWED (the loop's
    // cross-predicate disambiguation — mined templates win collisions).
    let mut raw = baseline.clone();
    for (predicate, proposed) in &proposals_log {
        let slot = raw.predicates.entry(predicate.clone()).or_default();
        for t in proposed {
            if !slot.contains(t) {
                slot.push(t.clone());
            }
        }
    }
    let reviewed = merge_and_disambiguate(&baseline, &proposals_log);

    let mut entities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in &val_oracle {
        let list = entities.entry(r.document_id.clone()).or_default();
        for t in &r.triples {
            list.push(t.subject.clone());
            list.push(t.object.clone());
        }
        list.sort();
        list.dedup();
    }
    let entities_for = |id: &str| entities.get(id).cloned().unwrap_or_default();

    println!("\n=== held-out validation (exact matcher) ===");
    for (label, rules) in [
        ("baseline", &baseline),
        ("+ LLM proposals (raw)", &raw),
        ("+ LLM proposals (disambiguated)", &reviewed),
    ] {
        let s = score_documents(
            Lane::NeuronAuthored,
            &val_docs,
            &val_oracle,
            entities_for,
            rules,
        );
        println!(
            "{label:<34} recall {}  precision {}  (correct {} / constructed {})",
            pct(s.recall),
            pct(s.precision),
            s.correct,
            s.constructed_total
        );
    }
    Ok(())
}

fn pct(v: Option<f64>) -> String {
    v.map_or_else(|| "—".into(), |x| format!("{:.1}%", x * 100.0))
}
