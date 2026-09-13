//! Trace-to-answer diagnostic (deterministic, no LLM): for each eval
//! question, how many EXPECTED facts survive into the ranked trace the
//! answerer sees? Separates the retrieval bound (fact not in trace →
//! model cannot assert it) from answerer selectivity (in trace, not
//! asserted). Sweeps the edge-selection limit to show where the ceiling
//! sits.
//!
//! Run: cargo run -p cognigraph-construct --example answer_trace_diag -- DIR

use std::collections::HashSet;
use std::path::Path;

use cognigraph_construct::ingest::entity_key;
use cognigraph_construct::*;
use cognigraph_core::{FieldPredicate, GraphBackend, PredicateOp};
use cognigraph_native::NativeBackend;
use serde_json::Value;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: answer_trace_diag DIR"))?;
    let dir = Path::new(&dir);
    let space: SpaceType =
        serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
    let spec: EvalSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &space, &chunks, &[]).await?;

    let predicate = [FieldPredicate {
        path: vec!["space_id".into()],
        op: PredicateOp::Eq,
        value: Value::String(spec.space_id.clone()),
    }];
    let edges: Vec<GraphEdge> = backend
        .list_documents_filtered("facts", &predicate, None, None, None)
        .await?
        .iter()
        .filter_map(GraphEdge::from_value)
        .collect();
    let boosts = std::collections::HashMap::new();

    let display = |vertex: &str| -> String {
        let key = vertex.strip_prefix("entities/").unwrap_or(vertex);
        space
            .entities
            .iter()
            .find(|e| entity_key(&e.name) == key)
            .map(|e| e.name.clone())
            .unwrap_or_else(|| key.to_string())
    };

    println!(
        "kit {} — {} fact edges in graph\n",
        dir.display(),
        edges.len()
    );
    println!(
        "{:<42} {:>5} {:>8} {:>8} {:>8} {:>8} {:>6}",
        "question", "exp", "lim=12", "lim=24", "lim=48", "lim=all", "seeds"
    );
    for question in &spec.questions {
        let seeds: HashSet<String> = mentions(&question.question, &space.entities)
            .into_iter()
            .map(|e| format!("entities/{}", entity_key(&e.name)))
            .collect();
        let expected: Vec<Fact> = question
            .expected_facts
            .iter()
            .filter_map(|raw| Fact::parse(raw))
            .collect();

        let in_trace = |limit: usize| -> usize {
            let opts = EdgeSelectOpts {
                limit,
                ..EdgeSelectOpts::default()
            };
            let trace = select_graph_edges(&edges, &seeds, &boosts, &opts);
            let lines: HashSet<String> = trace
                .iter()
                .map(|e| {
                    format!(
                        "{} --{}--> {}",
                        display(&e.source),
                        e.relation,
                        display(&e.target)
                    )
                })
                .collect();
            expected
                .iter()
                .filter(|f| {
                    lines.contains(&format!("{} --{}--> {}", f.source, f.relation, f.target))
                })
                .count()
        };

        println!(
            "{:<42} {:>5} {:>8} {:>8} {:>8} {:>8} {:>6}",
            &question.id[..question.id.len().min(42)],
            expected.len(),
            in_trace(12),
            in_trace(24),
            in_trace(48),
            in_trace(usize::MAX),
            seeds.len(),
        );
    }
    Ok(())
}
