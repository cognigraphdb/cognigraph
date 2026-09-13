//! The vector-RAG control arm: the head-to-head the positioning dossier
//! was missing. Same kits, same questions, same answering model, same
//! one-pass/two-pass technique — the ONLY variable is the substrate:
//!
//!   vector arm — top-K chunks by cosine similarity (question embedding
//!                vs chunk embeddings, brute force), raw passages handed
//!                to the model
//!   graph arm  — the shipped pipeline: authored ontology → grounded
//!                facts → ranked 48-edge trace (`answer_eval_with`)
//!
//! Fairness notes, disclosed up front:
//! - The vector arm receives the ANSWER VOCABULARY (entity names +
//!   relation labels from the space type) so exact fact scoring is
//!   possible — the same vocabulary the graph trace exposes implicitly.
//! - The graph arm's fabrication guard has no mechanical equivalent for
//!   raw passages, so vector-arm recall is an UPPER BOUND (an asserted
//!   expected fact counts even if no passage supports it) and its
//!   restraint is raw assertions. Both asymmetries FAVOR the vector
//!   arm; any restraint loss it shows is conservative.
//! - K = 8 passages ≈ several times the token budget of the 48-line
//!   graph trace.
//!
//! Run: cargo run --release -p cognigraph-construct --example vector_rag_control -- DIR...

use std::collections::HashSet;
use std::path::Path;

use cognigraph_construct::*;
use cognigraph_embeddings::EmbeddingProvider;
use cognigraph_embeddings::completion::{CompletionProvider, OpenAiCompletion};
use cognigraph_embeddings::openai::OpenAiProvider;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

const TOP_K: usize = 8;

fn answer_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "facts": { "type": "array", "items": { "type": "string" } } },
        "required": ["facts"],
        "additionalProperties": false
    })
}

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let (mut dot, mut na, mut nb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    dot / (na.sqrt() * nb.sqrt()).max(f64::EPSILON)
}

fn parse_facts(response: &Value) -> Vec<String> {
    response["facts"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

async fn vector_answer(
    provider: &dyn CompletionProvider,
    passages: &[&str],
    vocabulary: &str,
    question: &str,
    two_pass: bool,
) -> anyhow::Result<Vec<String>> {
    let system = "You answer questions STRICTLY from the provided passages. \
                  Never use outside knowledge; never assert what the passages do not support.";
    let base = format!(
        "Passages (the ONLY knowledge you may use):\n{}\n\n{}\n\nQuestion: {}",
        passages.join("\n---\n"),
        vocabulary,
        question
    );
    let user = format!(
        "{base}\n\nReturn JSON {{\"facts\": [\"A --REL--> B\", ...]}} listing EVERY fact \
         the passages support that is relevant to answering the question — do not stop at \
         the most salient ones; include each fact that belongs in a complete answer. \
         Return an empty list if none apply."
    );
    let mut asserted = parse_facts(
        &provider
            .complete_json(system, &user, &answer_schema())
            .await?,
    );
    if two_pass {
        let critic = format!(
            "{base}\n\nA first pass selected these facts as the answer:\n{}\n\n\
             Review the passages once more for COMPLETENESS: which facts they support \
             were MISSED but also belong in a complete answer — including boundary, \
             negative, definitional, or context facts a careful answer would cite? \
             Return JSON {{\"facts\": [...]}} with ONLY the additional facts; empty list \
             if the selection is already complete.",
            if asserted.is_empty() {
                "(none)".to_string()
            } else {
                asserted.join("\n")
            }
        );
        for fact in parse_facts(
            &provider
                .complete_json(system, &critic, &answer_schema())
                .await?,
        ) {
            if !asserted.iter().any(|have| have == &fact) {
                asserted.push(fact);
            }
        }
    }
    Ok(asserted)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        anyhow::bail!("usage: vector_rag_control DIR...");
    }
    let key =
        std::env::var("OPENAI_API_KEY").map_err(|_| anyhow::anyhow!("OPENAI_API_KEY required"))?;
    let completion = OpenAiCompletion::new(key.clone(), None, None)?;
    let embedder = OpenAiProvider::new(key, None, None)?;
    println!(
        "answering model: {} | embeddings: {} | vector top-{TOP_K} vs graph 48-edge trace\n",
        completion.model_name(),
        embedder.model_name()
    );

    // [arm][pass] -> (recall, forbidden) accumulators.
    let mut totals = [[(0usize, 0usize); 2]; 2];
    let (mut exp_all, mut forb_all) = (0usize, 0usize);

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

        // Answer vocabulary: what the graph trace exposes implicitly.
        let entities: Vec<&str> = space.entities.iter().map(|e| e.name.as_str()).collect();
        let mut relations: Vec<&str> = space
            .relation_rules
            .iter()
            .map(|r| r.relation.as_str())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        relations.sort_unstable();
        let vocabulary = format!(
            "Answer vocabulary — express each fact as `A --REL--> B` using ONLY these \
             entity names and relation labels.\nEntities: {}\nRelations: {}",
            entities.join("; "),
            relations.join("; ")
        );

        // Embed the corpus once (batched), questions per kit.
        let mut chunk_vectors: Vec<Vec<f64>> = Vec::with_capacity(chunks.len());
        for batch in chunks.chunks(100) {
            let texts: Vec<&str> = batch.iter().map(|c| c.text.as_str()).collect();
            chunk_vectors.extend(embedder.embed(&texts).await?);
        }

        // Graph arm substrate: the shipped pipeline.
        let backend = NativeBackend::new();
        ingest_chunks(&backend, &spec.space_id, &space, &chunks, &[]).await?;

        let per_question_expected: Vec<Vec<Fact>> = spec
            .questions
            .iter()
            .map(|q| {
                q.expected_facts
                    .iter()
                    .filter_map(|f| Fact::parse(f))
                    .collect()
            })
            .collect();
        let per_question_forbidden: Vec<Vec<Fact>> = spec
            .questions
            .iter()
            .map(|q| {
                q.forbidden_facts
                    .iter()
                    .filter_map(|f| Fact::parse(f))
                    .collect()
            })
            .collect();
        let kit_expected: usize = per_question_expected.iter().map(Vec::len).sum();
        let kit_forbidden: usize = per_question_forbidden.iter().map(Vec::len).sum();
        exp_all += kit_expected;
        forb_all += kit_forbidden;

        let mut kit = [[(0usize, 0usize); 2]; 2];
        let mut leaked: Vec<String> = Vec::new();
        for (pass, two_pass) in [(0usize, false), (1usize, true)] {
            // Vector arm.
            for (qi, question) in spec.questions.iter().enumerate() {
                let qv = &embedder.embed(&[question.question.as_str()]).await?[0];
                let mut ranked: Vec<(f64, usize)> = chunk_vectors
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (cosine(qv, v), i))
                    .collect();
                ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
                let passages: Vec<&str> = ranked
                    .iter()
                    .take(TOP_K)
                    .map(|(_, i)| chunks[*i].text.as_str())
                    .collect();
                let asserted: Vec<Fact> = vector_answer(
                    &completion,
                    &passages,
                    &vocabulary,
                    &question.question,
                    two_pass,
                )
                .await?
                .iter()
                .filter_map(|raw| Fact::parse(raw))
                .collect();
                kit[0][pass].0 += per_question_expected[qi]
                    .iter()
                    .filter(|f| asserted.contains(f))
                    .count();
                let bad: Vec<&Fact> = per_question_forbidden[qi]
                    .iter()
                    .filter(|f| asserted.contains(f))
                    .collect();
                kit[0][pass].1 += bad.len();
                for fact in bad {
                    leaked.push(format!(
                        "vector {}-pass [{}]: {} --{}--> {}",
                        pass + 1,
                        question.id,
                        fact.source,
                        fact.relation,
                        fact.target
                    ));
                }
            }
            // Graph arm: the shipped answer pipeline over the same spec.
            let outcome = answer_eval_with(
                &backend,
                &spec.space_id,
                &space,
                &[],
                &spec,
                &completion,
                AnswerOpts {
                    two_pass,
                    ..Default::default()
                },
            )
            .await?;
            kit[1][pass].0 += outcome
                .questions
                .iter()
                .map(|q| q.expected_found)
                .sum::<usize>();
            kit[1][pass].1 += outcome
                .questions
                .iter()
                .map(|q| q.forbidden_asserted)
                .sum::<usize>();
        }

        println!(
            "== {}  ({} questions, {} expected, {} forbidden)",
            dir.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default(),
            spec.questions.len(),
            kit_expected,
            kit_forbidden
        );
        println!(
            "   vector: 1-pass {}/{} f{}  2-pass {}/{} f{}   |   graph: 1-pass {}/{} f{}  2-pass {}/{} f{}",
            kit[0][0].0,
            kit_expected,
            kit[0][0].1,
            kit[0][1].0,
            kit_expected,
            kit[0][1].1,
            kit[1][0].0,
            kit_expected,
            kit[1][0].1,
            kit[1][1].0,
            kit_expected,
            kit[1][1].1,
        );
        for line in &leaked {
            println!("   FORBIDDEN ASSERTED  {line}");
        }
        for (arm, row) in kit.iter().enumerate() {
            for (pass, cell) in row.iter().enumerate() {
                totals[arm][pass].0 += cell.0;
                totals[arm][pass].1 += cell.1;
            }
        }
        println!();
    }

    println!(
        "COMBINED ({exp_all} expected, {forb_all} forbidden)\n\
         vector arm: 1-pass {}/{exp_all} forbidden {}/{forb_all}  |  2-pass {}/{exp_all} forbidden {}/{forb_all}\n\
         graph arm:  1-pass {}/{exp_all} forbidden {}/{forb_all}  |  2-pass {}/{exp_all} forbidden {}/{forb_all}",
        totals[0][0].0,
        totals[0][0].1,
        totals[0][1].0,
        totals[0][1].1,
        totals[1][0].0,
        totals[1][0].1,
        totals[1][1].0,
        totals[1][1].1,
    );
    println!(
        "\nvector recall is an upper bound (no fabrication guard is possible over raw \
         passages); graph recall is trace-checked. Restraint is raw assertions on both arms."
    );
    Ok(())
}
