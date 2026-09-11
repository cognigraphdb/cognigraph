//! The restraint-repair experiment runner — `blind_eval`'s mirror image.
//! Where blind_eval repairs RECALL (misses → relation_hint proposals),
//! this repairs RESTRAINT: baseline violations → violation-directed
//! `relation_blocker` proposals (B3: veto phrases copied verbatim from
//! the violating chunks, symbolically self-checked, coverage simulated),
//! then an IF-ACCEPTED re-measure that must close violations WITHOUT
//! costing recall, plus the answer-level scorecard on the repaired graph.
//!
//! Proposals are written to {DIR}/proposed.blockers.json for the human
//! review pass; the IF-ACCEPTED numbers simulate acceptance for the
//! experiment readout only.
//!
//! `--rounds N` (default 1 = the original one-shot experiment) enables
//! coverage-guided iteration: each round re-proposes against ONLY the
//! chunks the accumulated vetoes still leave uncovered.
//! `--strip-gates` removes authored require_in_sentence gates first —
//! reproduces the pre-gates hostile baseline the original experiment
//! ran against.
//!
//! Run: cargo run --release -p cognigraph-construct --example blocker_repair -- [--rounds N] [--strip-gates] DIR

use std::path::Path;

use cognigraph_construct::*;
use cognigraph_embeddings::completion::{CompletionProvider, OpenAiCompletion};
use cognigraph_native::NativeBackend;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let mut rounds = 1usize;
    let mut strip_gates = false;
    let mut dir_arg: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rounds" => {
                rounds = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| anyhow::anyhow!("--rounds needs a number"))?;
            }
            "--strip-gates" => strip_gates = true,
            other => dir_arg = Some(other.to_string()),
        }
    }
    let dir = dir_arg
        .ok_or_else(|| anyhow::anyhow!("usage: blocker_repair [--rounds N] [--strip-gates] DIR"))?;
    let dir = Path::new(&dir);
    let mut space: SpaceType =
        serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
    if strip_gates {
        for rule in &mut space.relation_rules {
            rule.require_in_sentence.clear();
        }
        println!("(require_in_sentence gates stripped — pre-gates baseline)");
    }
    let spec: EvalSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    // 1. Baseline construction on the ontology as authored.
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

    // Distinct violated facts (the eval counts per mention across
    // questions; the proposer should see each triple once).
    let mut violations: Vec<Fact> = Vec::new();
    for fact in &baseline.violations {
        if !violations.contains(fact) {
            violations.push(fact.clone());
        }
    }
    println!("distinct violated facts: {}", violations.len());
    for fact in &violations {
        println!(
            "  VIOLATION  {} --{}--> {}",
            fact.source, fact.relation, fact.target
        );
    }
    if violations.is_empty() {
        println!("nothing to repair.");
        return Ok(());
    }

    let key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY required for the propose stage"))?;
    let provider = OpenAiCompletion::new(key, None, None)?;
    println!("\ncompletion model: {}", provider.model_name());

    // 2. Violation-directed blocker proposals, coverage-guided: each
    // round re-proposes against only the still-uncovered remainder.
    let report = propose_blockers_covering(&provider, &space, &violations, &chunks, rounds).await?;
    for skip in &report.skipped {
        println!(
            "  SKIPPED    {} --{}--> {}: {}",
            skip.fact.source, skip.fact.relation, skip.fact.target, skip.reason
        );
    }
    println!(
        "\nPROPOSED {} blockers over {} fact(s), max {} round(s) (status=proposed, awaiting review):",
        report.set.neurons.len(),
        report.facts.len(),
        rounds
    );
    for outcome in &report.facts {
        println!(
            "  {} --{}--> {}: {} violating chunk(s), {} round(s), {}{}",
            outcome.fact.source,
            outcome.fact.relation,
            outcome.fact.target,
            outcome.violating_total,
            outcome.rounds,
            if outcome.covered {
                "COVERED".to_string()
            } else {
                format!("{} uncovered", outcome.uncovered.len())
            },
            outcome
                .stopped
                .as_deref()
                .map(|s| format!("  [{s}]"))
                .unwrap_or_default()
        );
        for id in &outcome.neuron_ids {
            let neuron = report.set.neurons.iter().find(|n| &n.id == id).unwrap();
            let coverage = report.coverage.iter().find(|c| &c.neuron_id == id).unwrap();
            println!(
                "      {}  vetoes={:?}  (+{} chunk(s))",
                neuron.id,
                neuron.triggers,
                coverage.suppressed.len()
            );
        }
    }
    std::fs::write(
        dir.join("proposed.blockers.json"),
        serde_json::to_string_pretty(&report.set)?,
    )?;
    println!("written: {}/proposed.blockers.json", dir.display());

    // 3. IF ACCEPTED: re-ground with the vetoes live; violations must
    // close WITHOUT costing recall.
    let accepted: Vec<Neuron> = report
        .set
        .neurons
        .into_iter()
        .map(|mut n| {
            n.status = NeuronStatus::Accepted;
            n
        })
        .collect();
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
    for fact in &after.violations {
        println!(
            "  STILL VIOLATED  {} --{}--> {}",
            fact.source, fact.relation, fact.target
        );
    }
    for fact in &after.missing {
        println!(
            "  RECALL LOST     {} --{}--> {}",
            fact.source, fact.relation, fact.target
        );
    }

    // 4. Answer-level scorecard on the repaired graph.
    let answers = answer_eval(&healed, &spec.space_id, &space, &accepted, &spec, &provider).await?;
    println!("\nANSWER-LEVEL SCORECARD (repaired graph):");
    for q in &answers.questions {
        println!(
            "  [{}] recall {}/{}  forbidden {}/{}",
            q.id, q.expected_found, q.expected_total, q.forbidden_asserted, q.forbidden_total
        );
    }
    println!(
        "\nanswer recall_ok={} restraint_ok={}  (review the proposals before trusting these)",
        answers.recall_ok(),
        answers.restraint_ok()
    );
    Ok(())
}
