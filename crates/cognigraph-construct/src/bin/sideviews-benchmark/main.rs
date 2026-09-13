//! LIVE benchmark for context-expansion side-view generation. The eventual
//! feature makes 10-20 LLM calls PER DOCUMENT, so the model choice here is the
//! dominant cost lever — this harness compares candidate `provider:model` pairs
//! on a fixed set of sample passages BEFORE we commit to one, so a cheaper model
//! can be chosen for side-views while completions keep their own model.
//!
//! It makes outward, paid, NON-DETERMINISTIC calls, so it is an opt-in binary
//! run only with providers configured (keys in `.env`). Every call goes through
//! the strict JSON-schema path (`generate_sideviews`) — we never parse prose.
//!
//! Metrics are cheap automatic proxies, not a judge: pair count vs target,
//! within-doc question distinctness, self-contained questions (standalone
//! queries, not "the passage above"), and lexical answer grounding (answer
//! content words found in the source — a hallucination smell test). Pair the
//! table with `--out` to eyeball the actual Q&A.
//!
//! Usage (needs the provider keys, e.g. via .env):
//!   cargo run --release -p cognigraph-construct --bin sideviews-benchmark -- \
//!     [--docs <file>] [--n 12] \
//!     [--model openai:gpt-5.6-luna] [--model gemini:gemini-3.8-flash] \
//!     [--out <dump.json>]
//! With no --model, the configured side-view provider (COGNIGRAPH_SIDEVIEWS_*)
//! is used.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, anyhow};
use cognigraph_construct::sideviews::{QaPair, generate_sideviews};
use cognigraph_embeddings::completion::{provider_named, sideviews_provider_from_env};
use serde::Deserialize;
use serde_json::json;

/// A benchmark document is either a bare string or `{ id?, text }`.
#[derive(Deserialize)]
#[serde(untagged)]
enum DocInput {
    Text(String),
    Rec { id: Option<String>, text: String },
}

struct Doc {
    id: String,
    text: String,
}

/// A model to benchmark: a display label plus the resolved `provider:model`.
struct ModelSpec {
    label: String,
    provider: String,
    model: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut docs_path =
        PathBuf::from("crates/cognigraph-construct/fixtures/sideviews/benchmark-docs.json");
    let mut count = 12usize;
    let mut model_args: Vec<String> = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--docs" => {
                docs_path = PathBuf::from(args.next().ok_or_else(|| anyhow!("--docs value"))?)
            }
            "--n" => count = args.next().ok_or_else(|| anyhow!("--n value"))?.parse()?,
            "--model" => model_args.push(args.next().ok_or_else(|| anyhow!("--model value"))?),
            "--out" => {
                out = Some(PathBuf::from(
                    args.next().ok_or_else(|| anyhow!("--out value"))?,
                ))
            }
            "-h" | "--help" => {
                println!(
                    "Benchmark side-view generation across provider:model pairs.\n\
                     --docs <file> --n <pairs> --model <provider[:model]> (repeatable) --out <dump.json>"
                );
                return Ok(());
            }
            other => return Err(anyhow!("unknown argument '{other}'")),
        }
    }

    dotenvy::dotenv().ok();

    let docs = load_docs(&docs_path)?;
    println!(
        "benchmarking {} passages, target {count} pairs each\n",
        docs.len()
    );

    // Resolve the model specs. With none given, fall back to the configured
    // side-view provider so a plain run still measures the real default.
    let specs = if model_args.is_empty() {
        vec![ModelSpec {
            label: "sideviews-env".into(),
            provider: String::new(),
            model: None,
        }]
    } else {
        model_args.iter().map(|m| parse_spec(m)).collect()
    };

    let mut summaries = Vec::new();
    let mut dump = serde_json::Map::new();
    for spec in &specs {
        let provider = if spec.provider.is_empty() {
            sideviews_provider_from_env().context("resolving COGNIGRAPH_SIDEVIEWS_* provider")?
        } else {
            provider_named(&spec.provider, spec.model.clone())
                .with_context(|| format!("building provider for '{}'", spec.label))?
        };
        let label = if spec.provider.is_empty() {
            format!("{} ({})", spec.label, provider.model_name())
        } else {
            spec.label.clone()
        };
        println!("--> {label}");

        let mut agg = Aggregate::default();
        let mut per_doc = serde_json::Map::new();
        for doc in &docs {
            let start = Instant::now();
            match generate_sideviews(provider.as_ref(), &doc.text, count).await {
                Ok(pairs) => {
                    let elapsed_ms = start.elapsed().as_millis();
                    agg.record(&doc.text, &pairs, count, elapsed_ms);
                    per_doc.insert(doc.id.clone(), json!(pairs));
                    println!(
                        "    {:<12} {:>3} pairs  {:>6} ms",
                        doc.id,
                        pairs.len(),
                        elapsed_ms
                    );
                }
                Err(e) => {
                    agg.errors += 1;
                    // Print only the top-level cause; bodies can carry secrets.
                    println!("    {:<12} ERROR: {}", doc.id, e);
                }
            }
        }
        dump.insert(label.clone(), json!(per_doc));
        summaries.push((label, agg));
        println!();
    }

    print_table(&summaries);

    if let Some(path) = out {
        std::fs::write(&path, serde_json::to_vec_pretty(&json!(dump))?)
            .with_context(|| format!("writing dump to {}", path.display()))?;
        println!("\nfull side-views written to {}", path.display());
    }
    Ok(())
}

/// Parse `provider` or `provider:model` (model may itself contain colons).
fn parse_spec(raw: &str) -> ModelSpec {
    match raw.split_once(':') {
        Some((provider, model)) => ModelSpec {
            label: raw.to_string(),
            provider: provider.to_string(),
            model: Some(model.to_string()),
        },
        None => ModelSpec {
            label: raw.to_string(),
            provider: raw.to_string(),
            model: None,
        },
    }
}

fn load_docs(path: &PathBuf) -> Result<Vec<Doc>> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading benchmark docs {}", path.display()))?;
    let inputs: Vec<DocInput> = serde_json::from_str(&raw).context("parsing benchmark docs")?;
    Ok(inputs
        .into_iter()
        .enumerate()
        .map(|(i, d)| match d {
            DocInput::Text(text) => Doc {
                id: format!("doc{i}"),
                text,
            },
            DocInput::Rec { id, text } => Doc {
                id: id.unwrap_or_else(|| format!("doc{i}")),
                text,
            },
        })
        .collect())
}

/// Running metrics for one model across all sample passages.
#[derive(Default)]
struct Aggregate {
    docs_ok: usize,
    errors: usize,
    total_pairs: usize,
    target_hits: usize,
    distinct_rate_sum: f64,
    self_contained_pairs: usize,
    grounding_sum: f64,
    answer_words: usize,
    latency_sum_ms: u128,
    latency_max_ms: u128,
}

impl Aggregate {
    fn record(&mut self, source: &str, pairs: &[QaPair], target: usize, elapsed_ms: u128) {
        self.docs_ok += 1;
        self.total_pairs += pairs.len();
        if pairs.len() >= target {
            self.target_hits += 1;
        }
        self.latency_sum_ms += elapsed_ms;
        self.latency_max_ms = self.latency_max_ms.max(elapsed_ms);

        // Within-doc question distinctness.
        let distinct: BTreeSet<String> = pairs.iter().map(|p| norm(&p.question)).collect();
        if !pairs.is_empty() {
            self.distinct_rate_sum += distinct.len() as f64 / pairs.len() as f64;
        }

        let source_words = content_words(source);
        for pair in pairs {
            if self_contained(&pair.question) {
                self.self_contained_pairs += 1;
            }
            let answer_words = content_words(&pair.answer);
            self.answer_words += pair.answer.split_whitespace().count();
            if !answer_words.is_empty() {
                let overlap = answer_words
                    .iter()
                    .filter(|w| source_words.contains(*w))
                    .count();
                self.grounding_sum += overlap as f64 / answer_words.len() as f64;
            }
        }
    }

    fn pairs_per_doc(&self) -> f64 {
        ratio(self.total_pairs, self.docs_ok)
    }
    fn target_hit_pct(&self) -> f64 {
        100.0 * ratio(self.target_hits, self.docs_ok)
    }
    fn distinct_pct(&self) -> f64 {
        100.0 * ratio_f(self.distinct_rate_sum, self.docs_ok)
    }
    fn self_contained_pct(&self) -> f64 {
        100.0 * ratio(self.self_contained_pairs, self.total_pairs)
    }
    fn grounding_pct(&self) -> f64 {
        100.0 * ratio_f(self.grounding_sum, self.total_pairs)
    }
    fn avg_answer_words(&self) -> f64 {
        ratio(self.answer_words, self.total_pairs)
    }
    fn avg_latency_ms(&self) -> f64 {
        ratio(self.latency_sum_ms as usize, self.docs_ok)
    }
}

fn ratio(num: usize, den: usize) -> f64 {
    if den == 0 {
        0.0
    } else {
        num as f64 / den as f64
    }
}
fn ratio_f(num: f64, den: usize) -> f64 {
    if den == 0 { 0.0 } else { num / den as f64 }
}

fn print_table(summaries: &[(String, Aggregate)]) {
    println!(
        "{:<28} {:>4} {:>4} {:>7} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "model", "ok", "err", "pairs", "hit%", "distinct", "selfC%", "ground%", "ansWd", "lat_ms"
    );
    for (label, a) in summaries {
        println!(
            "{:<28} {:>4} {:>4} {:>7.1} {:>5.0}% {:>7.0}% {:>6.0}% {:>7.0}% {:>8.1} {:>8.0}",
            truncate(label, 28),
            a.docs_ok,
            a.errors,
            a.pairs_per_doc(),
            a.target_hit_pct(),
            a.distinct_pct(),
            a.self_contained_pct(),
            a.grounding_pct(),
            a.avg_answer_words(),
            a.avg_latency_ms(),
        );
    }
    println!(
        "\nhit% = docs returning >= target pairs; distinct = within-doc unique questions;\n\
         selfC% = standalone questions (no \"the passage\"); ground% = answer content words\n\
         found in source (lexical proxy for grounding); lat_ms = avg per-doc latency."
    );
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

/// Normalize a question for within-doc dedup: lowercase, collapse whitespace,
/// strip trailing punctuation.
fn norm(q: &str) -> String {
    q.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(['?', '.', '!'])
        .to_string()
}

/// Content words: lowercased alphanumeric tokens of length >= 3 that are not
/// common function words. Used for both grounding overlap and (via the source)
/// the reference vocabulary.
fn content_words(s: &str) -> BTreeSet<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "was", "were", "with", "that", "this", "from", "are", "has", "had",
        "have", "its", "his", "her", "their", "them", "which", "what", "who", "whom", "when",
        "where", "why", "how", "did", "does", "into", "than", "then", "they", "she", "him",
    ];
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() >= 3)
        .map(|w| w.to_lowercase())
        .filter(|w| !STOP.contains(&w.as_str()))
        .collect()
}

/// A question is self-contained if it does not lean on the source frame — a
/// standalone search query, not "what does the passage say".
fn self_contained(question: &str) -> bool {
    const META: &[&str] = &[
        "the passage",
        "this passage",
        "the text",
        "this text",
        "the document",
        "this document",
        "the article",
        "the author",
        "mentioned",
        "described",
        "above",
        "according to the",
    ];
    let q = question.to_lowercase();
    !META.iter().any(|m| q.contains(m))
}
