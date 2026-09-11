//! B1 blind-eval runner. Point it at a directory holding the three authored
//! files — space_type.json, chunks.jsonl, eval.json — and it runs the full
//! loop cold:
//!
//!   1. BASELINE construction (the naive ontology as authored): recall +
//!      restraint, with every MISS and VIOLATION listed.
//!   2. PROPOSE (live, needs OPENAI_API_KEY): gap-directed repairs for the
//!      misses, retrieved via the backend's own BM25.
//!   3. IF ACCEPTED: re-measure construction with the proposals accepted —
//!      does the loop close gaps WITHOUT opening violations?
//!   4. ANSWER-LEVEL: per-question recall/restraint, answering strictly
//!      from the ranked graph-facts trace.
//!
//! Proposals are written to {DIR}/proposed.neurons.json for the human
//! review pass. NOTHING is auto-accepted in production; the "IF ACCEPTED"
//! and answer numbers simulate acceptance for the experiment readout only.
//!
//! Run: cargo run -p cognigraph-construct --example blind_eval -- DIR

use std::path::Path;

use cognigraph_construct::*;
use cognigraph_embeddings::completion::{CompletionProvider, OpenAiCompletion};
use cognigraph_native::NativeBackend;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let dir = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: blind_eval DIR"))?;
    let dir = Path::new(&dir);
    let space: SpaceType =
        serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
    let spec: EvalSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    // 1. Baseline: ingest the naive ontology exactly as authored.
    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &space, &chunks, &[]).await?;
    let baseline = evaluate(&backend, &spec.space_id, &spec).await?;
    println!(
        "BASELINE   recall {}/{}  restraint violations {}/{}",
        baseline.expected_found,
        baseline.expected_total,
        baseline.forbidden_triggered,
        baseline.forbidden_total
    );
    for f in &baseline.missing {
        println!("  MISS       {} --{}--> {}", f.source, f.relation, f.target);
    }
    for f in &baseline.violations {
        println!("  VIOLATION  {} --{}--> {}", f.source, f.relation, f.target);
    }

    let Ok(key) = std::env::var("OPENAI_API_KEY") else {
        println!("\nOPENAI_API_KEY not set — baseline only (propose/answer stages skipped).");
        return Ok(());
    };
    let provider = OpenAiCompletion::new(key, None, None)?;
    println!("\ncompletion model: {}", provider.model_name());

    // 2. Propose repairs for the gaps (BM25-retrieved evidence).
    let accepted = if baseline.missing.is_empty() {
        println!("\nno gaps to repair.");
        Vec::new()
    } else {
        let report = propose_neurons_via_backend(
            &provider,
            &space,
            &baseline.missing,
            &backend,
            &spec.space_id,
        )
        .await?;
        for skip in &report.skipped {
            println!(
                "  SKIPPED    {} --{}--> {}: {}",
                skip.fact.source, skip.fact.relation, skip.fact.target, skip.reason
            );
        }
        println!(
            "\nPROPOSED {} repairs (status=proposed, awaiting review):",
            report.set.neurons.len()
        );
        for n in &report.set.neurons {
            println!(
                "  {}  {} --{}--> {}  triggers={:?}",
                n.id, n.source, n.relation, n.target, n.triggers
            );
        }
        std::fs::write(
            dir.join("proposed.neurons.json"),
            serde_json::to_string_pretty(&report.set)?,
        )?;
        println!("written: {}/proposed.neurons.json", dir.display());
        report
            .set
            .neurons
            .into_iter()
            .map(|mut n| {
                n.status = NeuronStatus::Accepted;
                n
            })
            .collect()
    };

    // 3. IF ACCEPTED: re-measure construction with the repairs applied.
    let healed = NativeBackend::new();
    ingest_chunks(
        &healed,
        &spec.space_id,
        &effective_config(&space, &accepted),
        &chunks,
        &effective_vetoes(&accepted),
    )
    .await?;
    let after = evaluate(&healed, &spec.space_id, &spec).await?;
    println!(
        "\nIF ACCEPTED  recall {}/{}  restraint violations {}/{}",
        after.expected_found,
        after.expected_total,
        after.forbidden_triggered,
        after.forbidden_total
    );

    // 4. Answer-level recall/restraint over the constructed graph.
    let answers = answer_eval(&healed, &spec.space_id, &space, &accepted, &spec, &provider).await?;
    println!("\nANSWER-LEVEL SCORECARD:");
    for q in &answers.questions {
        println!(
            "  [{}] recall {}/{}  forbidden {}/{}  — {}",
            q.id,
            q.expected_found,
            q.expected_total,
            q.forbidden_asserted,
            q.forbidden_total,
            q.question
        );
    }
    println!(
        "\nanswer recall_ok={} restraint_ok={}  (review the proposals before trusting these)",
        answers.recall_ok(),
        answers.restraint_ok()
    );
    Ok(())
}
