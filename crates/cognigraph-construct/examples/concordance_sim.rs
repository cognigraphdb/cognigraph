//! The agreement-lane attestation instrument
//! (decision_agreement_lane.md): given recorded replay verdicts for two
//! judges (one or more runs each), compute — deterministically, offline,
//! no LLM — whether the PAIR has any concordant false-accept at the
//! policy threshold, worst-case across all provided runs. A policy's
//! `agreement.concordance_measured: true` attests a zero result from
//! this instrument for the named pair.
//!
//! Run: cargo run --release -p cognigraph-construct --example concordance_sim -- \
//!        [--threshold 0.9] A_RUN.json... --vs B_RUN.json...

use std::collections::BTreeMap;

use serde_json::Value;

struct Verdict {
    label: String,
    accept_at: Vec<f64>, // confidences of accept verdicts across runs
}

fn load(paths: &[String]) -> anyhow::Result<BTreeMap<String, Verdict>> {
    let mut out: BTreeMap<String, Verdict> = BTreeMap::new();
    for path in paths {
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        for v in doc["verdicts"].as_array().into_iter().flatten() {
            let case = v["neuron"].as_str().unwrap_or_default().to_string();
            let entry = out.entry(case).or_insert_with(|| Verdict {
                label: v["label"].as_str().unwrap_or_default().to_string(),
                accept_at: Vec::new(),
            });
            if v["verdict"] == "accept" {
                entry
                    .accept_at
                    .push(v["judge"]["confidence"].as_f64().unwrap_or(0.0));
            }
        }
    }
    Ok(out)
}

fn main() -> anyhow::Result<()> {
    let mut threshold = 0.9f64;
    let mut a_paths: Vec<String> = Vec::new();
    let mut b_paths: Vec<String> = Vec::new();
    let mut into_b = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--threshold" => {
                threshold = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| anyhow::anyhow!("--threshold needs a number"))?;
            }
            "--vs" => into_b = true,
            path if into_b => b_paths.push(path.to_string()),
            path => a_paths.push(path.to_string()),
        }
    }
    if a_paths.is_empty() || b_paths.is_empty() {
        anyhow::bail!("usage: concordance_sim [--threshold T] A_RUN.json... --vs B_RUN.json...");
    }

    let a = load(&a_paths)?;
    let b = load(&b_paths)?;
    let worst = |v: Option<&Verdict>| -> bool {
        v.is_some_and(|v| v.accept_at.iter().any(|c| *c >= threshold))
    };

    let cases: Vec<&String> = a.keys().filter(|k| b.contains_key(*k)).collect();
    let mut concordant_fa: Vec<&String> = Vec::new();
    let (mut good_total, mut good_concordant) = (0usize, 0usize);
    for case in &cases {
        let (va, vb) = (a.get(*case), b.get(*case));
        let label = va.map(|v| v.label.as_str()).unwrap_or_default();
        let both = worst(va) && worst(vb);
        if label == "reject_or_flag" && both {
            concordant_fa.push(case);
        }
        if label == "accept" {
            good_total += 1;
            if both {
                good_concordant += 1;
            }
        }
    }
    println!(
        "pair over {} shared cases at threshold {threshold} \
         (worst-case across {} + {} runs):",
        cases.len(),
        a_paths.len(),
        b_paths.len()
    );
    println!(
        "  concordant FALSE-ACCEPTS on reject-labeled cases: {}  {:?}",
        concordant_fa.len(),
        concordant_fa
    );
    println!("  concordant accepts on good cases: {good_concordant}/{good_total}");
    println!(
        "\nATTESTATION {}: agreement.concordance_measured may be set to true for this \
         pair only when the first number is ZERO.",
        if concordant_fa.is_empty() {
            "PASSES"
        } else {
            "FAILS"
        }
    );
    Ok(())
}
