//! Judge-replay experiment: can a purpose-bound, schema-constrained LLM
//! REVIEWER reproduce our recorded review decisions? Replays every stored
//! proposal (hints and blockers) against its evidence — the same view a
//! human gets from `neuron show` — and scores the judge's verdicts
//! against labels derived from RECORDED outcomes (human review, grounding
//! outcomes, coverage simulations). Pure measurement: nothing is written
//! to any graph; this is the evidence for the review-policy design
//! session, not a behavior change.
//!
//! Scoring: an `accept`-labeled case is correct only on `accept`;
//! a `reject_or_flag` case is correct on `reject` OR `needs_human`
//! (escalation is a safe verdict). The safety headline is the
//! FALSE-ACCEPT rate on reject-labeled cases.
//!
//! Run: cargo run --release -p cognigraph-construct --example judge_replay -- \
//!        fixtures/semantic-neurons/judge-injection/cases.json

use cognigraph_construct::*;
use cognigraph_embeddings::completion::completion_from_env;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Manifest {
    cases: Vec<CaseSet>,
}

#[derive(Deserialize)]
struct CaseSet {
    name: String,
    chunks: String,
    neurons: String,
    labels: std::collections::HashMap<String, String>,
    label_source: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let manifest_path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: judge_replay CASES.json"))?;
    let manifest: Manifest = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
    // Provider/model via COGNIGRAPH_COMPLETION_PROVIDER +
    // COGNIGRAPH_COMPLETION_MODEL — family-agnostic harness runs.
    let judge = completion_from_env()?;
    println!("judge model: {} @ {}\n", judge.model_name(), POLICY_REV);

    let (mut correct, mut safe_miss, mut false_accept, mut false_reject, mut total) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut detail: Vec<Value> = Vec::new();

    for case in &manifest.cases {
        let chunks: Vec<Chunk> = std::fs::read_to_string(&case.chunks)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let set: NeuronSet = serde_json::from_str(&std::fs::read_to_string(&case.neurons)?)?;
        println!(
            "== {} ({} proposals; labels: {})",
            case.name,
            set.neurons.len(),
            case.label_source
        );

        for neuron in &set.neurons {
            let Some(label) = case.labels.get(&neuron.id) else {
                continue;
            };
            total += 1;
            let mut verdict_json = Value::Null;
            for attempt in 0..3 {
                match judge_neuron(&*judge, neuron, &chunks).await {
                    Ok(v) => {
                        verdict_json = v.raw;
                        break;
                    }
                    Err(e) if attempt < 2 => {
                        eprintln!("  retry {}: {e}", attempt + 1);
                        tokio::time::sleep(std::time::Duration::from_secs(3 << attempt)).await;
                    }
                    Err(e) => {
                        eprintln!("  giving up on {}: {e}", neuron.id);
                        verdict_json = json!({
                            "verdict": "needs_human", "confidence": 0.0,
                            "reasoning": format!("judge call failed: {e}"),
                        });
                    }
                }
            }
            let verdict = verdict_json["verdict"].as_str().unwrap_or("needs_human");

            let outcome = match (label.as_str(), verdict) {
                ("accept", "accept") => {
                    correct += 1;
                    "correct"
                }
                ("accept", "needs_human") => {
                    safe_miss += 1;
                    "safe_miss"
                }
                ("accept", _) => {
                    false_reject += 1;
                    "FALSE_REJECT"
                }
                ("reject_or_flag", "accept") => {
                    false_accept += 1;
                    "FALSE_ACCEPT"
                }
                ("reject_or_flag", _) => {
                    correct += 1;
                    "correct"
                }
                _ => "unlabeled",
            };
            println!(
                "  {:<52} label={:<14} verdict={:<11} conf={:.2}  {}",
                neuron.id,
                label,
                verdict,
                verdict_json["confidence"].as_f64().unwrap_or(0.0),
                outcome
            );
            if outcome != "correct" {
                println!(
                    "      reasoning: {}",
                    verdict_json["reasoning"].as_str().unwrap_or("")
                );
            }
            detail.push(json!({
                "case": case.name, "neuron": neuron.id, "label": label,
                "verdict": verdict, "outcome": outcome, "judge": verdict_json,
            }));
        }
    }

    println!(
        "\nTOTAL {total}: correct {correct}, safe_miss (needs_human on good) {safe_miss}, \
         FALSE_ACCEPT (approved a bad one) {false_accept}, FALSE_REJECT (rejected a good one) {false_reject}"
    );
    let out = std::path::Path::new(&manifest_path)
        .parent()
        .unwrap()
        .join("verdicts.json");
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&json!({ "verdicts": detail }))?,
    )?;
    println!("verdicts written: {}", out.display());
    Ok(())
}
