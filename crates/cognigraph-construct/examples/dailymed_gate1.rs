//! DailyMed pilot, gate 1 (docs/dailymed-pilot.md): run 100 documents to
//! validate section-aware chunking and the bounded ontology BEFORE the
//! 1,000-document calibration gate spends anything.
//!
//! What it does, in order:
//! 1. Select the gate cohort deterministically from the corpus manifests
//!    (default 90 prescription + 10 OTC, lowest `selection_rank` first —
//!    the corpus's own seeded order, so the cohort is reproducible).
//! 2. Section-aware chunking: one chunk per SPL narrative section,
//!    oversized sections split at sentence boundaries. Distribution
//!    stats are the chunking verdict.
//! 3. Draft a bounded ontology from the cohort with the governed drafter
//!    (draft-policy-v1: verbatim/closure checks + gate advisor). The
//!    draft is written to `data/dailymed/` for
//!    human review — gate 1 validates the loop, it does not freeze
//!    policy (that is gate 3's job).
//! 4. Ingest the cohort with the drafted ontology and report grounding:
//!    fact edges, rules that fired, per-relation counts, restraint
//!    surface (advisor flags), wall-clock.
//!
//! LLM usage: exactly the drafter's two completion calls. Grounding and
//! ingest are deterministic. No embeddings.
//!
//! Run: cargo run --release -p cognigraph-construct --example dailymed_gate1 -- \
//!        [--rx 90] [--otc 10] [--sample-cap 24] [--corpus data/dailymed-pilot]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use cognigraph_construct::*;
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::completion::completion_from_env;
use cognigraph_native::NativeBackend;
use serde::Deserialize;
use serde_json::json;

const SPACE_ID: &str = "dailymed-gate1";
/// Sections longer than this are split at sentence boundaries…
const MAX_CHUNK_CHARS: usize = 2_800;
/// …into parts that stop growing once they pass this target.
const TARGET_CHUNK_CHARS: usize = 2_000;
/// Sections shorter than this are boilerplate stubs, not evidence.
const MIN_SECTION_CHARS: usize = 40;

#[derive(Deserialize)]
struct ManifestEntry {
    set_id: String,
    document_path: String,
    selection_rank: u64,
}

#[derive(Deserialize)]
struct NormalizedDoc {
    set_id: String,
    sections: Vec<DocSection>,
}

#[derive(Deserialize)]
struct DocSection {
    title: String,
    text: String,
}

fn manifest_cohort(corpus: &Path, name: &str, take: usize) -> Result<Vec<ManifestEntry>> {
    let raw = std::fs::read_to_string(corpus.join(name))
        .with_context(|| format!("reading {name} — run the collector first"))?;
    let mut entries: Vec<ManifestEntry> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()
        .with_context(|| format!("parsing {name}"))?;
    entries.sort_by_key(|e| e.selection_rank);
    entries.truncate(take);
    Ok(entries)
}

/// Split one oversized section at sentence boundaries ('.' counts as a
/// sentence end only before whitespace/end — the corpus is full of
/// decimals and dotted names).
fn split_sentences(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut current = String::new();
    for (i, b) in bytes.iter().enumerate() {
        let sentence_end = *b == b'.'
            && bytes
                .get(i + 1)
                .is_none_or(|next| next.is_ascii_whitespace());
        if sentence_end || i + 1 == bytes.len() {
            let sentence = text[start..=i].trim();
            start = i + 1;
            if sentence.is_empty() {
                continue;
            }
            if !current.is_empty() && current.len() + sentence.len() > TARGET_CHUNK_CHARS {
                parts.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(sentence);
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

struct ChunkStats {
    docs: usize,
    sections_total: usize,
    sections_skipped_short: usize,
    sections_split: usize,
}

fn chunk_documents(docs: &[NormalizedDoc]) -> (Vec<Chunk>, ChunkStats) {
    let mut chunks = Vec::new();
    let mut stats = ChunkStats {
        docs: docs.len(),
        sections_total: 0,
        sections_skipped_short: 0,
        sections_split: 0,
    };
    for doc in docs {
        for (idx, section) in doc.sections.iter().enumerate() {
            stats.sections_total += 1;
            let text = section.text.trim();
            if text.len() < MIN_SECTION_CHARS {
                stats.sections_skipped_short += 1;
                continue;
            }
            if text.len() <= MAX_CHUNK_CHARS {
                chunks.push(Chunk {
                    id: format!("{}-s{idx:02}", doc.set_id),
                    title: section.title.clone(),
                    text: text.to_string(),
                });
            } else {
                stats.sections_split += 1;
                for (part, piece) in split_sentences(text).into_iter().enumerate() {
                    chunks.push(Chunk {
                        id: format!("{}-s{idx:02}-p{part}", doc.set_id),
                        title: section.title.clone(),
                        text: piece,
                    });
                }
            }
        }
    }
    (chunks, stats)
}

fn percentile(sorted: &[usize], p: f64) -> usize {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

fn arg(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let corpus = PathBuf::from(arg("--corpus").unwrap_or_else(|| "data/dailymed-pilot".into()));
    let n_rx: usize = arg("--rx").and_then(|v| v.parse().ok()).unwrap_or(90);
    let n_otc: usize = arg("--otc").and_then(|v| v.parse().ok()).unwrap_or(10);
    let sample_cap: usize = arg("--sample-cap")
        .and_then(|v| v.parse().ok())
        .unwrap_or(24);
    let out_dir = PathBuf::from("data/dailymed");

    // 1. Cohort — the manifests' own deterministic selection order.
    let mut cohort = manifest_cohort(&corpus, "manifest-rx.jsonl", n_rx)?;
    cohort.extend(manifest_cohort(&corpus, "manifest-otc.jsonl", n_otc)?);
    let docs: Vec<NormalizedDoc> = cohort
        .iter()
        .map(|entry| {
            let raw = std::fs::read_to_string(corpus.join(&entry.document_path))
                .with_context(|| format!("reading {}", entry.document_path))?;
            serde_json::from_str(&raw).with_context(|| format!("parsing {}", entry.set_id))
        })
        .collect::<Result<_>>()?;
    println!("cohort      {} rx + {} otc documents", n_rx, n_otc);

    // 2. Section-aware chunking.
    let (chunks, stats) = chunk_documents(&docs);
    let mut sizes: Vec<usize> = chunks.iter().map(|c| c.text.len()).collect();
    sizes.sort_unstable();
    println!(
        "chunking    {} docs, {} sections -> {} chunks ({} short sections skipped, {} split)",
        stats.docs,
        stats.sections_total,
        chunks.len(),
        stats.sections_skipped_short,
        stats.sections_split
    );
    println!(
        "chunk size  min {} / p50 {} / mean {} / p95 {} / max {} chars",
        sizes.first().copied().unwrap_or(0),
        percentile(&sizes, 0.50),
        sizes.iter().sum::<usize>() / sizes.len().max(1),
        percentile(&sizes, 0.95),
        sizes.last().copied().unwrap_or(0)
    );

    // 3. Bounded ontology: draft it (2 LLM calls), or reload a saved
    //    draft with `--space FILE` for deterministic re-analysis.
    let report = if let Some(path) = arg("--space") {
        let space: SpaceType = serde_json::from_str(&std::fs::read_to_string(&path)?)
            .with_context(|| format!("parsing {path}"))?;
        println!(
            "space       loaded {} ({} entities, {} rules) — drafter skipped",
            path,
            space.entities.len(),
            space.relation_rules.len()
        );
        let advisor = advise_gates(&space, &[], &chunks);
        DraftReport {
            space,
            skips: Vec::new(),
            advisor,
            sampled_chunks: 0,
        }
    } else {
        let provider = completion_from_env()?;
        println!(
            "drafter     {DRAFT_REV}, sample cap {sample_cap} of {} chunks",
            chunks.len()
        );
        let t = Instant::now();
        let report = draft_space_type(provider.as_ref(), SPACE_ID, &chunks, sample_cap).await?;
        println!(
            "draft       {} entities, {} relation rules in {:.1}s ({} self-check drops, {} sampled chunks)",
            report.space.entities.len(),
            report.space.relation_rules.len(),
            t.elapsed().as_secs_f64(),
            report.skips.len(),
            report.sampled_chunks
        );
        for skip in &report.skips {
            println!("  drop      {skip}");
        }
        report
    };
    let flagged: Vec<&GateAdvice> = report
        .advisor
        .rules
        .iter()
        .filter(|a| a.suggestion.is_some() || !a.never_in_sentence.is_empty())
        .collect();
    println!(
        "advisor     {} of {} rules flagged for gate review",
        flagged.len(),
        report.advisor.rules.len()
    );
    for advice in &flagged {
        println!("  review    {} — {}", advice.fact, advice.reason);
    }

    // 4. Ingest with the draft and report grounding behaviour. Grounding
    //    EVENTS collapse into distinct (source, relation, target) edges
    //    on upsert; report both.
    let backend = NativeBackend::new();
    let t = Instant::now();
    let grounded = ingest_chunks(&backend, SPACE_ID, &report.space, &chunks, &[]).await?;
    let ingest_ms = t.elapsed().as_millis();
    let facts = backend.list_documents("facts", None, None).await?;
    let mut by_relation: BTreeMap<String, usize> = BTreeMap::new();
    let mut evidence_chunks: std::collections::HashSet<String> = Default::default();
    for fact in &facts {
        *by_relation
            .entry(fact["relation_type"].as_str().unwrap_or("?").to_string())
            .or_default() += 1;
        if let Some(chunk) = fact["evidence_chunk_id"].as_str() {
            evidence_chunks.insert(chunk.to_string());
        }
    }
    let rules_fired = report
        .space
        .relation_rules
        .iter()
        .filter(|r| {
            facts.iter().any(|f| {
                f["relation_type"].as_str() == Some(r.relation.as_str())
                    && f["_from"].as_str().is_some_and(|from| {
                        from.ends_with(&format!("/{}", ingest::entity_key(&r.source)))
                    })
            })
        })
        .count();
    println!(
        "grounding   {grounded} grounding events -> {} distinct fact edges from {} evidence chunks in {ingest_ms} ms; {} of {} rules fired",
        facts.len(),
        evidence_chunks.len(),
        rules_fired,
        report.space.relation_rules.len()
    );
    for (relation, count) in &by_relation {
        println!("  relation  {relation}: {count}");
    }

    // 5. Persist the draft for human review (gate 3 freezes policy). A
    //    `--space` reanalysis writes a separate analysis file and leaves
    //    the original draft artifacts untouched.
    let reanalysis = arg("--space").is_some();
    std::fs::create_dir_all(&out_dir)?;
    if !reanalysis {
        std::fs::write(
            out_dir.join("gate1-draft-space.json"),
            serde_json::to_string_pretty(&report.space)?,
        )?;
    }
    std::fs::write(
        out_dir.join(if reanalysis {
            "gate1-analysis.json"
        } else {
            "gate1-draft-report.json"
        }),
        serde_json::to_string_pretty(&json!({
            "draft_rev": DRAFT_REV,
            "cohort": { "rx": n_rx, "otc": n_otc },
            "sampled_chunks": report.sampled_chunks,
            "skips": report.skips,
            "advisor": report.advisor,
            "chunks": chunks.len(),
            "grounded_edges": grounded,
            "rules_fired": rules_fired,
            "by_relation": by_relation,
        }))?,
    )?;
    println!(
        "saved       {}",
        out_dir
            .join(if reanalysis {
                "gate1-analysis.json"
            } else {
                "gate1-draft-space.json"
            })
            .display()
    );
    Ok(())
}
