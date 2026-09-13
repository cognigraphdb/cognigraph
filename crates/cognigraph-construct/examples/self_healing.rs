//! B7 live: the self-healing loop with a real LLM doing the rewiring.
//! Run: cargo run -p cognigraph-construct --example self_healing

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
    let load = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    let space: SpaceType = serde_json::from_str(&load("crowdstrike_2024.space_type.json"))?;
    let set: NeuronSet = serde_json::from_str(&load("crowdstrike_2024.neurons.accepted.json"))?;
    let spec: EvalSpec = serde_json::from_str(&load("crowdstrike_2024.eval.json"))?;
    let chunks = |name: &str| -> Vec<Chunk> {
        load(name)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    };
    let v1 = chunks("crowdstrike_2024.chunks.jsonl");
    let v2 = chunks("crowdstrike_2024.chunks.v2.jsonl");
    let config = effective_config(&space, &set.neurons);

    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &config, &v1, &[]).await?;
    let healthy = evaluate(&backend, &spec.space_id, &spec).await?;
    println!(
        "v1 HEALTHY   recall {}/{}  restraint {}/{}",
        healthy.expected_found,
        healthy.expected_total,
        healthy.forbidden_triggered,
        healthy.forbidden_total
    );

    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &config, &v2, &[]).await?;
    let degraded = evaluate(&backend, &spec.space_id, &spec).await?;
    println!(
        "v2 DEGRADED  recall {}/{}  (document revision killed the pathways)",
        degraded.expected_found, degraded.expected_total
    );
    for path in degradation_report(&space, &set.neurons, &v2) {
        println!(
            "  DEAD PATH [{}] {} — {} --{}--> {}",
            match path.kind {
                PathwayKind::Neuron => "neuron",
                PathwayKind::BaseRule => "rule",
            },
            path.id,
            path.fact.source,
            path.fact.relation,
            path.fact.target
        );
    }

    let provider = cognigraph_embeddings::completion::OpenAiCompletion::new(key, None, None)?;
    let report = propose_neurons_via_backend(
        &provider,
        &space,
        &degraded.missing,
        &backend,
        &spec.space_id,
    )
    .await?;
    for skip in &report.skipped {
        println!(
            "  SKIPPED {} --{}--> {}: {}",
            skip.fact.source, skip.fact.relation, skip.fact.target, skip.reason
        );
    }
    println!(
        "REWIRED {} pathways (proposed, review pending):",
        report.set.neurons.len()
    );
    for n in &report.set.neurons {
        println!("  {}  triggers={:?}", n.id, n.triggers);
    }

    // Simulated review acceptance for the experiment run.
    let mut combined = set.neurons.clone();
    combined.extend(report.set.neurons.iter().cloned().map(|mut n| {
        n.status = NeuronStatus::Accepted;
        n
    }));
    let backend = NativeBackend::new();
    ingest_chunks(
        &backend,
        &spec.space_id,
        &effective_config(&space, &combined),
        &v2,
        &[],
    )
    .await?;
    let healed = evaluate(&backend, &spec.space_id, &spec).await?;
    println!(
        "v2 HEALED    recall {}/{}  restraint {}/{}",
        healed.expected_found,
        healed.expected_total,
        healed.forbidden_triggered,
        healed.forbidden_total
    );

    println!("PRUNE CANDIDATES (graduation, post-rewire):");
    for c in graduation_report(&space, &combined, &v2) {
        println!("  {}  ({:?})", c.neuron_id, c.reason);
    }
    Ok(())
}
