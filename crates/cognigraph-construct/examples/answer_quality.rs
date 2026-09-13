//! B2 live scorecard: answer-level recall/restraint on the CrowdStrike
//! generalization space with the human-accepted neurons.
//! Run: cargo run -p cognigraph-construct --example answer_quality

use cognigraph_construct::*;
use cognigraph_native::NativeBackend;
use std::path::Path;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let Ok(key) = std::env::var("OPENAI_API_KEY") else {
        eprintln!("Skipping: OPENAI_API_KEY not set");
        return Ok(());
    };
    let dir = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/semantic-neurons/generalization"
    ));
    let space: SpaceType = serde_json::from_str(&std::fs::read_to_string(
        dir.join("crowdstrike_2024.space_type.json"),
    )?)?;
    let set: NeuronSet = serde_json::from_str(&std::fs::read_to_string(
        dir.join("crowdstrike_2024.neurons.accepted.json"),
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
    let repaired = effective_config(&space, &set.neurons);
    ingest_chunks(
        &backend,
        &spec.space_id,
        &repaired,
        &chunks,
        &effective_vetoes(&set.neurons),
    )
    .await?;

    let provider = cognigraph_embeddings::completion::OpenAiCompletion::new(key, None, None)?;
    let outcome = answer_eval(
        &backend,
        &spec.space_id,
        &space,
        &set.neurons,
        &spec,
        &provider,
    )
    .await?;

    println!("ANSWER-LEVEL SCORECARD (construction was 6/6, 0/3)\n");
    for q in &outcome.questions {
        println!(
            "  [{}] recall {}/{}  forbidden {}/{}  — {}",
            q.id,
            q.expected_found,
            q.expected_total,
            q.forbidden_asserted,
            q.forbidden_total,
            q.question
        );
        for fact in &q.asserted {
            println!("        asserted: {fact}");
        }
    }
    println!(
        "\nanswer recall_ok={} restraint_ok={}",
        outcome.recall_ok(),
        outcome.restraint_ok()
    );
    Ok(())
}
