//! Grounding benchmark: throughput of `ground_chunk` as vetoes scale, plus
//! the blocker quality matrix. Run: cargo run --release -p
//! cognigraph-construct --example grounding_bench

use std::path::Path;
use std::time::Instant;

use cognigraph_construct::*;

fn main() -> anyhow::Result<()> {
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
    let config = effective_config(&space, &set.neurons);
    let base_chunks: Vec<Chunk> =
        std::fs::read_to_string(dir.join("crowdstrike_2024.chunks.jsonl"))?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
    // Replicate to 4000 chunks for stable timing.
    let chunks: Vec<Chunk> = (0..100)
        .flat_map(|i| {
            base_chunks.iter().map(move |c| Chunk {
                id: format!("{}-{i}", c.id),
                title: c.title.clone(),
                text: c.text.clone(),
            })
        })
        .collect();

    println!(
        "corpus: {} chunks, {} relation rules",
        chunks.len(),
        config.relation_rules.len()
    );

    // Synthetic vetoes: triple-scoped on the config's rules, phrases that
    // never match (worst case: every veto is checked, none short-circuits).
    let synthetic_vetoes = |count: usize| -> Vec<VetoRule> {
        (0..count)
            .map(|i| {
                let rule = &config.relation_rules[i % config.relation_rules.len()];
                VetoRule {
                    source: rule.source.clone(),
                    relation: rule.relation.clone(),
                    target: rule.target.clone(),
                    when_any: vec![format!("veto-phrase-that-never-appears-{i}")],
                }
            })
            .collect()
    };

    for veto_count in [0usize, 8, 64] {
        let vetoes = synthetic_vetoes(veto_count);
        // Warm-up + best-of-5 (matches the workspace's benchmark habit).
        let mut best = f64::MAX;
        let mut grounded = 0usize;
        for _ in 0..5 {
            let start = Instant::now();
            grounded = chunks
                .iter()
                .map(|c| ground_chunk(&c.id, &c.text, &config, &vetoes).len())
                .sum();
            best = best.min(start.elapsed().as_secs_f64() * 1000.0);
        }
        println!(
            "vetoes={veto_count:>3}  {:>8.2} ms / {} chunks  ({:.1} µs/chunk, {grounded} facts)",
            best,
            chunks.len(),
            best * 1000.0 / chunks.len() as f64
        );
    }
    Ok(())
}
