//! One-pass vs two-pass answer eval over fixture kits: does the
//! completeness-critic second pass recover the answer-recall residue
//! (misses concentrated on boundary/meta questions) without costing
//! restraint? Ingests each kit with its authored ontology (construction
//! on these kits is verified full elsewhere), then runs `answer_eval`
//! both ways and prints the per-kit and combined comparison.
//!
//! Run: cargo run --release -p cognigraph-construct --example answer_pass_compare -- DIR...

use std::path::Path;

use cognigraph_construct::*;
use cognigraph_embeddings::completion::{CompletionProvider, OpenAiCompletion};
use cognigraph_native::NativeBackend;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let mut evidence = false;
    let dirs: Vec<String> = std::env::args()
        .skip(1)
        .filter(|arg| {
            if arg == "--evidence" {
                evidence = true;
                false
            } else {
                true
            }
        })
        .collect();
    if dirs.is_empty() {
        anyhow::bail!("usage: answer_pass_compare [--evidence] DIR...");
    }
    let key =
        std::env::var("OPENAI_API_KEY").map_err(|_| anyhow::anyhow!("OPENAI_API_KEY required"))?;
    let provider = OpenAiCompletion::new(key, None, None)?;
    println!(
        "completion model: {} | evidence sentences: {}\n",
        provider.model_name(),
        evidence
    );

    let (mut r1, mut r2, mut total) = (0usize, 0usize, 0usize);
    let (mut v1, mut v2, mut vtotal) = (0usize, 0usize, 0usize);
    let (mut fab1, mut fab2) = (0usize, 0usize);

    for dir in &dirs {
        let dir = Path::new(dir);
        let space: SpaceType =
            serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
        let spec: EvalSpec =
            serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
        let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        let backend = NativeBackend::new();
        ingest_chunks(&backend, &spec.space_id, &space, &chunks, &[]).await?;

        let mut outcomes = Vec::new();
        for two_pass in [false, true] {
            let outcome = answer_eval_with(
                &backend,
                &spec.space_id,
                &space,
                &[],
                &spec,
                &provider,
                AnswerOpts {
                    two_pass,
                    evidence_sentences: evidence,
                },
            )
            .await?;
            outcomes.push(outcome);
        }
        let sum = |o: &AnswerOutcome| {
            (
                o.questions.iter().map(|q| q.expected_found).sum::<usize>(),
                o.questions.iter().map(|q| q.expected_total).sum::<usize>(),
                o.questions
                    .iter()
                    .map(|q| q.forbidden_asserted)
                    .sum::<usize>(),
                o.questions.iter().map(|q| q.forbidden_total).sum::<usize>(),
                o.questions
                    .iter()
                    .map(|q| q.fabricated.len())
                    .sum::<usize>(),
            )
        };
        let (a1, at, af, aft, afab) = sum(&outcomes[0]);
        let (b1, _, bf, _, bfab) = sum(&outcomes[1]);
        println!(
            "== {}  one-pass recall {a1}/{at} forbidden {af}/{aft} fab {afab}  |  \
             two-pass recall {b1}/{at} forbidden {bf}/{aft} fab {bfab}",
            dir.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        );
        for (qa, qb) in outcomes[0].questions.iter().zip(&outcomes[1].questions) {
            if qa.expected_found != qb.expected_found || qb.forbidden_asserted > 0 {
                println!(
                    "     [{}] {}/{} -> {}/{}  forbidden {} -> {}",
                    qa.id,
                    qa.expected_found,
                    qa.expected_total,
                    qb.expected_found,
                    qb.expected_total,
                    qa.forbidden_asserted,
                    qb.forbidden_asserted
                );
            }
        }
        r1 += a1;
        r2 += b1;
        total += at;
        v1 += af;
        v2 += bf;
        vtotal += aft;
        fab1 += afab;
        fab2 += bfab;
    }
    println!(
        "\nCOMBINED  one-pass recall {r1}/{total} forbidden {v1}/{vtotal} fab {fab1}  |  \
         two-pass recall {r2}/{total} forbidden {v2}/{vtotal} fab {fab2}"
    );
    Ok(())
}
