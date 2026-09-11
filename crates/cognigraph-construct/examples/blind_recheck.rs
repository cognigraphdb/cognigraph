//! Deterministic reviewer tool: what would accepting a proposal set do?
//! Loads a kit plus a neurons.json (e.g. the proposed.neurons.json a
//! blind_eval run wrote), applies every neuron as accepted, re-measures
//! construction, and lists what still misses. No LLM, no mutation — the
//! symbolic half of the review loop, runnable offline.
//!
//! Run: cargo run -p cognigraph-construct --example blind_recheck -- DIR [NEURONS_JSON]
//! (NEURONS_JSON defaults to DIR/proposed.neurons.json)

use std::path::Path;

use cognigraph_construct::*;
use cognigraph_native::NativeBackend;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: blind_recheck DIR [NEURONS_JSON]"))?;
    let dir = Path::new(&dir);
    let neurons_path = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dir.join("proposed.neurons.json"));

    let space: SpaceType =
        serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
    let spec: EvalSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let set: NeuronSet = serde_json::from_str(&std::fs::read_to_string(&neurons_path)?)?;
    let accepted: Vec<Neuron> = set
        .neurons
        .into_iter()
        .map(|mut n| {
            n.status = NeuronStatus::Accepted;
            n
        })
        .collect();
    println!(
        "kit {} + {} neurons from {} (all treated as accepted)",
        dir.display(),
        accepted.len(),
        neurons_path.display()
    );

    let backend = NativeBackend::new();
    ingest_chunks(
        &backend,
        &spec.space_id,
        &effective_config(&space, &accepted),
        &chunks,
        &effective_vetoes(&accepted),
    )
    .await?;
    let report = evaluate(&backend, &spec.space_id, &spec).await?;
    println!(
        "recall {}/{}  restraint violations {}/{}",
        report.expected_found,
        report.expected_total,
        report.forbidden_triggered,
        report.forbidden_total
    );
    for f in &report.missing {
        println!(
            "  STILL MISSING  {} --{}--> {}",
            f.source, f.relation, f.target
        );
        for n in &accepted {
            if n.source == f.source && n.relation == f.relation && n.target == f.target {
                println!("    proposed triggers were: {:?}", n.triggers);
            }
        }
    }
    for f in &report.violations {
        println!(
            "  VIOLATION      {} --{}--> {}",
            f.source, f.relation, f.target
        );
    }
    Ok(())
}
