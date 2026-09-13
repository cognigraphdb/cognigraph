//! DailyMed drift experiment, Method B — synthetic controlled drift
//! (decision_dailymed_drift.md). No clean historical structured oracle
//! exists (every DailyMed/openFDA structured path serves current state
//! only), so instead of scraping fragile historical HTML we inject KNOWN
//! deltas into real baseline documents — whose clean current XML oracle we
//! already have — and measure whether the frozen `gate3-policy-v1`
//! narrative grounding tracks the change.
//!
//! Per document, one controlled edit (cycled deterministically):
//! - REMOVAL: delete a grounded active ingredient's name from the prose;
//!   grounding should drop that fact.
//! - ADDITION: append a composition sentence for a foreign ingredient;
//!   grounding should pick that fact up.
//! - ADMINISTRATIVE: append a boilerplate sentence with no ingredient;
//!   the grounded fact set should be unchanged (restraint).
//!
//! Ground truth is the injected delta — exact by construction. Method A
//! (real revisions via the HTML ingredient table) is the recorded
//! follow-up. Deterministic, offline, no LLM, no embeddings.
//!
//! Run: cargo run --release -p cognigraph-construct --example dailymed_drift -- [--cohort 90]

use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use cognigraph_construct::ingest::entity_key;
use cognigraph_construct::*;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use serde::Deserialize;
use serde_json::json;

const SPACE_ID: &str = "dailymed-drift";
const CHUNK_CHARS: usize = 2_000;

/// Frozen CONTAINS_INGREDIENT triggers (gate3-policy-v1).
fn contains_triggers() -> Vec<String> {
    ["contains {target}", "{target}, usp"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[derive(Deserialize)]
struct ManifestEntry {
    set_id: String,
    document_path: String,
    selection_rank: u64,
    title: String,
}

#[derive(Deserialize)]
struct NormalizedDoc {
    text: String,
}

/// Active ingredient substance names from the cached SPL XML.
fn parse_active(xml: &str) -> Result<BTreeSet<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut active = BTreeSet::new();
    let (mut cls, mut depth, mut idepth) = (None::<String>, 0usize, 0usize);
    let (mut in_sub, mut cap) = (false, false);
    loop {
        match reader.read_event() {
            Err(e) => anyhow::bail!("xml: {e}"),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                depth += 1;
                match e.name().as_ref() {
                    "ingredient" => {
                        let c = e
                            .try_get_attribute("classCode")?
                            .map(|a| {
                                a.normalized_value(XmlVersion::default())
                                    .unwrap_or_default()
                                    .to_string()
                            })
                            .unwrap_or_default();
                        if c.starts_with("ACTI") {
                            cls = Some(c);
                            idepth = depth;
                        }
                    }
                    "ingredientSubstance" if cls.is_some() => in_sub = true,
                    "name" if in_sub && !cap => cap = true,
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if cap {
                    let n = t.trim().to_string();
                    if !n.is_empty() {
                        active.insert(n);
                    }
                    cap = false;
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    "ingredient" if cls.is_some() && depth == idepth => {
                        cls = None;
                        in_sub = false;
                    }
                    "ingredientSubstance" => in_sub = false,
                    _ => {}
                }
                depth = depth.saturating_sub(1);
            }
            Ok(_) => {}
        }
    }
    Ok(active)
}

fn chunk(text: &str) -> Vec<Chunk> {
    text.as_bytes()
        .chunks(CHUNK_CHARS)
        .enumerate()
        .map(|(i, b)| Chunk {
            id: format!("c{i}"),
            title: String::new(),
            text: String::from_utf8_lossy(b).into_owned(),
        })
        .collect()
}

/// Case-insensitive removal of every occurrence of `needle` from `text`.
fn redact(text: &str, needle: &str) -> String {
    let (hay, need) = (text.to_lowercase(), needle.to_lowercase());
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if hay[i..].starts_with(&need) {
            out.push_str(" [redacted] ");
            i += need.len();
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Ground CONTAINS_INGREDIENT over `text` for `product`, given the vocab of
/// candidate ingredient names; return the grounded ingredient-name keys.
async fn grounded(product: &str, vocab: &BTreeSet<String>, text: &str) -> Result<HashSet<String>> {
    let backend = NativeBackend::new();
    let chunks = chunk(text);
    let entities: Vec<serde_json::Value> = std::iter::once(json!({
        "name": product, "type": "product", "aliases": Vec::<String>::new()
    }))
    .chain(
        vocab
            .iter()
            .map(|n| json!({"name": n, "type": "ingredient", "aliases": []})),
    )
    .collect();
    let low = text.to_lowercase();
    let rules: Vec<serde_json::Value> = vocab
        .iter()
        .filter(|n| low.contains(&n.to_lowercase()))
        .map(|n| {
            json!({"source": product, "relation": "CONTAINS_INGREDIENT",
                   "target": n, "when_any": contains_triggers()})
        })
        .collect();
    let config: SpaceType = serde_json::from_value(json!({
        "id": SPACE_ID, "entities": entities, "relation_rules": rules,
    }))?;
    ingest_chunks(&backend, SPACE_ID, &config, &chunks, &[]).await?;
    let facts = backend.list_documents("facts", None, None).await?;
    Ok(facts
        .iter()
        .filter_map(|f| {
            f["_to"]
                .as_str()
                .map(|s| s.trim_start_matches("entities/").to_string())
        })
        .collect())
}

fn arg(flag: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter()
        .position(|x| x == flag)
        .and_then(|i| a.get(i + 1).cloned())
}

fn cohort(corpus: &Path, name: &str, take: usize) -> Result<Vec<ManifestEntry>> {
    let mut e: Vec<ManifestEntry> = std::fs::read_to_string(corpus.join(name))
        .with_context(|| format!("read {name}"))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    e.sort_by_key(|x| x.selection_rank);
    e.truncate(take);
    Ok(e)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let n: usize = arg("--cohort").and_then(|v| v.parse().ok()).unwrap_or(90);
    let corpus = PathBuf::from("data/dailymed-pilot");
    let entries = cohort(&corpus, "manifest-rx.jsonl", n)?;

    // Load prose + active-ingredient oracle; build a global vocab for
    // addition targets.
    struct Doc {
        product: String,
        prose: String,
        active: BTreeSet<String>,
    }
    let mut docs = Vec::new();
    let mut global: BTreeSet<String> = BTreeSet::new();
    for e in &entries {
        let nd: NormalizedDoc = serde_json::from_str(
            &std::fs::read_to_string(corpus.join(&e.document_path))
                .with_context(|| format!("read {}", e.document_path))?,
        )?;
        let xml = std::fs::read_to_string(corpus.join("raw").join(format!("{}.xml", e.set_id)))?;
        let active = parse_active(&xml)?;
        global.extend(active.iter().cloned());
        let product = e
            .title
            .rfind('[')
            .map(|i| e.title[..i].trim())
            .unwrap_or(&e.title)
            .to_string();
        docs.push(Doc {
            product,
            prose: nd.text,
            active,
        });
    }

    // Per-type tallies.
    let mut rows: Vec<serde_json::Value> = Vec::new();
    let (mut rem_ok, mut rem_n) = (0usize, 0usize);
    let (mut add_ok, mut add_n) = (0usize, 0usize);
    let (mut adm_stable, mut adm_n) = (0usize, 0usize);

    for (i, doc) in docs.iter().enumerate() {
        // Vocab for this doc: its own actives + all global (so a foreign
        // addition target is groundable). Small enough; no prefilter.
        let vocab: BTreeSet<String> = doc.active.iter().chain(global.iter()).cloned().collect();
        let base = grounded(&doc.product, &vocab, &doc.prose).await?;

        match i % 3 {
            // REMOVAL: pick a grounded active ingredient; redact its name.
            0 => {
                let target = doc
                    .active
                    .iter()
                    .find(|a| base.contains(&entity_key(a)))
                    .cloned();
                let Some(target) = target else {
                    continue; // no grounded active ingredient to remove
                };
                rem_n += 1;
                let revised = redact(&doc.prose, &target);
                let after = grounded(&doc.product, &vocab, &revised).await?;
                let tk = entity_key(&target);
                let correct = base.contains(&tk) && !after.contains(&tk);
                if correct {
                    rem_ok += 1;
                }
                rows.push(json!({"set": i, "type": "removal", "target": target,
                    "grounded_before": base.len(), "grounded_after": after.len(), "correct": correct}));
            }
            // ADDITION: append a composition sentence for a foreign ingredient.
            1 => {
                let foreign = global
                    .iter()
                    .find(|g| {
                        !doc.active.contains(*g)
                            && !doc.prose.to_lowercase().contains(&g.to_lowercase())
                    })
                    .cloned();
                let Some(foreign) = foreign else {
                    continue;
                };
                add_n += 1;
                // Prepend the composition sentence so a fixed-size chunk
                // boundary can't split the trigger phrase (whole-section
                // chunking would never split it; this models that).
                let revised = format!("Each tablet contains {foreign}.\n{}", doc.prose);
                let after = grounded(&doc.product, &vocab, &revised).await?;
                let fk = entity_key(&foreign);
                let correct = !base.contains(&fk) && after.contains(&fk);
                if correct {
                    add_ok += 1;
                }
                rows.push(json!({"set": i, "type": "addition", "target": foreign,
                    "grounded_before": base.len(), "grounded_after": after.len(), "correct": correct}));
            }
            // ADMINISTRATIVE: append boilerplate with no ingredient name.
            _ => {
                adm_n += 1;
                let revised = format!(
                    "{}\nThis labeling was revised for administrative purposes; \
                     no change was made to the product composition.",
                    doc.prose
                );
                let after = grounded(&doc.product, &vocab, &revised).await?;
                let stable = base == after;
                if stable {
                    adm_stable += 1;
                }
                rows.push(json!({"set": i, "type": "administrative",
                    "grounded_before": base.len(), "grounded_after": after.len(), "stable": stable}));
            }
        }
    }

    let pct = |a: usize, b: usize| {
        if b > 0 {
            100.0 * a as f64 / b as f64
        } else {
            0.0
        }
    };
    println!(
        "=== Synthetic controlled drift over {} documents ===",
        docs.len()
    );
    println!(
        "  REMOVAL        {rem_ok}/{rem_n} correctly dropped ({:.0}%) — grounding tracks a deleted ingredient",
        pct(rem_ok, rem_n)
    );
    println!(
        "  ADDITION       {add_ok}/{add_n} correctly grounded ({:.0}%) — grounding picks up a new ingredient",
        pct(add_ok, add_n)
    );
    println!(
        "  ADMINISTRATIVE {adm_stable}/{adm_n} stable ({:.0}%) — no spurious drift on a no-op revision (restraint)",
        pct(adm_stable, adm_n)
    );
    println!(
        "  production-graph drift is exact by construction (production = structured-agreed; D4)"
    );

    let out = PathBuf::from("fixtures/semantic-neurons/dailymed/drift-synthetic-results.json");
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&json!({
            "decision": "decision_dailymed_drift.md",
            "method": "B (synthetic controlled drift; clean injected-delta oracle)",
            "documents": docs.len(),
            "removal": {"correct": rem_ok, "total": rem_n},
            "addition": {"correct": add_ok, "total": add_n},
            "administrative": {"stable": adm_stable, "total": adm_n},
            "rows": rows,
        }))?,
    )?;
    println!("saved       {}", out.display());
    Ok(())
}
