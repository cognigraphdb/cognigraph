//! The generalization experiment (decision 3): run the harness against a
//! third-party document with a blind eval, propose repairs for the misses,
//! and write them out as `proposed` for the human review pass.
//! Run: cargo run -p cognigraph-construct --example generalization

use cognigraph_construct::*;
use cognigraph_native::NativeBackend;
use std::path::Path;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let dir = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/semantic-neurons/generalization"
    ));
    let space: SpaceType = serde_json::from_str(&std::fs::read_to_string(
        dir.join("crowdstrike_2024.space_type.json"),
    )?)?;
    let spec: EvalSpec = serde_json::from_str(&std::fs::read_to_string(
        dir.join("crowdstrike_2024.eval.json"),
    )?)?;
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("crowdstrike_2024.chunks.jsonl"))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let backend = NativeBackend::new();
    let grounded = ingest_chunks(&backend, &spec.space_id, &space, &chunks, &[]).await?;
    let outcome = evaluate(&backend, &spec.space_id, &spec).await?;
    println!(
        "BASELINE  recall {}/{}  restraint violations {}/{}  grounded edges {}",
        outcome.expected_found,
        outcome.expected_total,
        outcome.forbidden_triggered,
        outcome.forbidden_total,
        grounded
    );
    for fact in &outcome.missing {
        println!(
            "  MISS  {} --{}--> {}",
            fact.source, fact.relation, fact.target
        );
    }
    for fact in &outcome.violations {
        println!(
            "  VIOLATION  {} --{}--> {}",
            fact.source, fact.relation, fact.target
        );
    }

    if outcome.missing.is_empty() {
        println!("no gaps; nothing to propose");
        return Ok(());
    }
    let key = std::env::var("OPENAI_API_KEY")?;
    let provider = cognigraph_embeddings::completion::OpenAiCompletion::new(key, None, None)?;
    let report = propose_neurons_report(&provider, &space, &outcome.missing, &chunks).await?;
    let proposals = report.set;
    for skip in &report.skipped {
        println!(
            "  SKIPPED  {} --{}--> {}: {}",
            skip.fact.source, skip.fact.relation, skip.fact.target, skip.reason
        );
    }
    println!(
        "\nPROPOSED {} neurons (status=proposed, awaiting review):",
        proposals.neurons.len()
    );
    for neuron in &proposals.neurons {
        println!(
            "  {}  {} --{}--> {}  triggers={:?}\n    rationale: {}",
            neuron.id,
            neuron.source,
            neuron.relation,
            neuron.target,
            neuron.triggers,
            neuron.rationale
        );
    }
    std::fs::write(
        dir.join("crowdstrike_2024.neurons.proposed.json"),
        serde_json::to_string_pretty(&proposals)?,
    )?;

    // Dry-run the review outcome: what the loop closes IF all are accepted.
    // Nothing is persisted as accepted — the review stays human.
    let mut accepted = proposals.neurons.clone();
    for neuron in &mut accepted {
        neuron.status = NeuronStatus::Accepted;
    }
    let repaired = effective_config(&space, &accepted);
    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &repaired, &chunks, &[]).await?;
    let after = evaluate(&backend, &spec.space_id, &spec).await?;
    println!(
        "\nIF ACCEPTED  recall {}/{}  restraint violations {}/{}",
        after.expected_found,
        after.expected_total,
        after.forbidden_triggered,
        after.forbidden_total
    );

    println!("\nwritten: crowdstrike_2024.neurons.proposed.json — review pass is yours");
    Ok(())
}
