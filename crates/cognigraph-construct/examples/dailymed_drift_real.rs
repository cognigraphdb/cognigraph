//! DailyMed drift, Method A — real revisions via the HTML ingredient table
//! (decision_dailymed_drift.md). The external-validity follow-up to the
//! synthetic experiment: measure the frozen `gate3-policy-v1` against
//! ACTUAL label revisions. Historical structured data is only in the
//! versioned HTML page, so the oracle is the ingredient-table active names
//! (name-based, lower rigor than the XML oracle — stated honestly).
//!
//! Hard-capped and polite: never scans more than `4x` the cohort target,
//! never decrements its progress counter (the first cut had a decrement
//! bug that walked the whole manifest), sleeps between fetches, and caches
//! every response so re-runs are offline.
//!
//! Run: cargo run --release -p cognigraph-construct --example dailymed_drift_real -- [--cohort 25]

use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use cognigraph_construct::ingest::entity_key;
use cognigraph_construct::*;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const HISTORY_URL: &str = "https://dailymed.nlm.nih.gov/dailymed/services/v2/spls";
const LOOKUP_URL: &str = "https://dailymed.nlm.nih.gov/dailymed/lookup.cfm";
const SPACE_ID: &str = "dailymed-drift-real";
const CHUNK_CHARS: usize = 2_000;

fn contains_triggers() -> Vec<String> {
    ["contains {target}", "{target}, usp"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[derive(Deserialize)]
struct ManifestEntry {
    set_id: String,
    selection_rank: u64,
    title: String,
}

#[derive(Deserialize)]
struct History {
    data: HistoryData,
}
#[derive(Deserialize)]
struct HistoryData {
    history: Vec<HistoryItem>,
}
#[derive(Deserialize)]
struct HistoryItem {
    spl_version: u64,
}

async fn fetch(client: &Client, url: &str, cache: &PathBuf) -> Result<String> {
    if let Ok(body) = std::fs::read_to_string(cache)
        && !body.trim().is_empty()
    {
        return Ok(body);
    }
    let mut last = None;
    for _ in 0..3 {
        match client.get(url).send().await {
            Ok(r) if r.status().is_success() => {
                let body = r.text().await?;
                std::fs::write(cache, &body).ok();
                // Politeness delay only on a real network hit.
                tokio::time::sleep(Duration::from_millis(250)).await;
                return Ok(body);
            }
            Ok(r) => last = Some(anyhow::anyhow!("HTTP {}", r.status())),
            Err(e) => last = Some(e.into()),
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("fetch failed")))
}

/// Active ingredient names from the SPL HTML ingredient table. Rows render
/// the name in `<td class="formItem"><strong>NAME</strong>`, bounded to the
/// "Active Ingredient/Active Moiety" section (before "Inactive
/// Ingredients"). Robust to the inline-UNII and no-UNII page formats.
fn parse_active(html: &str) -> BTreeSet<String> {
    let start = html.find("Active Ingredient/Active Moiety").unwrap_or(0);
    let end = html[start..]
        .find("Inactive Ingredient")
        .map(|i| start + i)
        .unwrap_or(html.len());
    let region = &html[start..end];
    let mut names = BTreeSet::new();
    let marker = "class=\"formItem\"><strong>";
    let mut i = 0;
    while let Some(rel) = region[i..].find(marker) {
        let s = i + rel + marker.len();
        if let Some(e) = region[s..].find("</strong>") {
            let name = region[s..s + e].trim();
            if !name.is_empty() && name.len() < 120 {
                names.insert(name.to_uppercase());
            }
            i = s + e;
        } else {
            break;
        }
    }
    names
}

/// Byte-safe tag strip. Works purely on bytes (the lowercased-String
/// indexing that preceded this panicked on multi-byte chars like the
/// non-breaking space); non-ASCII bytes become spaces, which is harmless
/// because ingredient names and triggers are ASCII.
fn strip_html(html: &str) -> String {
    fn starts_ci(b: &[u8], i: usize, pat: &[u8]) -> bool {
        b.len() >= i + pat.len() && b[i..i + pat.len()].eq_ignore_ascii_case(pat)
    }
    let b = html.as_bytes();
    let mut out = String::with_capacity(b.len() / 2);
    let mut in_tag = false;
    let mut skip: Option<&[u8]> = None;
    let mut i = 0;
    while i < b.len() {
        if let Some(end) = skip {
            if starts_ci(b, i, end) {
                skip = None;
                i += end.len();
            } else {
                i += 1;
            }
            continue;
        }
        if starts_ci(b, i, b"<script") {
            skip = Some(b"</script>");
            i += 7;
            continue;
        }
        if starts_ci(b, i, b"<style") {
            skip = Some(b"</style>");
            i += 6;
            continue;
        }
        match b[i] {
            b'<' => in_tag = true,
            b'>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(if c.is_ascii() { c as char } else { ' ' }),
            _ => {}
        }
        i += 1;
    }
    out
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cohort_cap: usize = arg("--cohort").and_then(|v| v.parse().ok()).unwrap_or(25);
    let scan_cap = cohort_cap * 4; // hard bound — cannot walk the manifest
    let corpus = PathBuf::from("data/dailymed-pilot");
    let cache = PathBuf::from("data/dailymed-drift");
    std::fs::create_dir_all(&cache)?;
    let client = Client::builder()
        .user_agent("cognigraph-dailymed-drift/1.0")
        .timeout(Duration::from_secs(60))
        .build()?;

    let mut entries: Vec<ManifestEntry> = std::fs::read_to_string(corpus.join("manifest-rx.jsonl"))
        .context("read manifest")?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    entries.sort_by_key(|e| e.selection_rank);

    let (mut examined, mut scanned) = (0usize, 0usize);
    let (mut reformulation, mut admin) = (0usize, 0usize);
    let (mut struct_add, mut struct_rm) = (0usize, 0usize);
    let (mut narr_add, mut narr_rm, mut admin_stable) = (0usize, 0usize, 0usize);
    let mut rows: Vec<serde_json::Value> = Vec::new();

    for entry in &entries {
        if examined >= cohort_cap || scanned >= scan_cap {
            break;
        }
        scanned += 1;
        let sid = &entry.set_id;
        let Ok(hist_raw) = fetch(
            &client,
            &format!("{HISTORY_URL}/{sid}/history.json"),
            &cache.join(format!("{sid}-history.json")),
        )
        .await
        else {
            continue;
        };
        let Ok(hist) = serde_json::from_str::<History>(&hist_raw) else {
            continue;
        };
        let mut vs: Vec<u64> = hist.data.history.iter().map(|h| h.spl_version).collect();
        vs.sort_unstable();
        if vs.len() < 2 {
            continue;
        }
        let (ov, cv) = (vs[0], *vs.last().unwrap());
        let (Ok(old_html), Ok(cur_html)) = (
            fetch(
                &client,
                &format!("{LOOKUP_URL}?setid={sid}&version={ov}"),
                &cache.join(format!("{sid}-v{ov}.html")),
            )
            .await,
            fetch(
                &client,
                &format!("{LOOKUP_URL}?setid={sid}&version={cv}"),
                &cache.join(format!("{sid}-v{cv}.html")),
            )
            .await,
        ) else {
            continue;
        };
        let old_ing = parse_active(&old_html);
        let cur_ing = parse_active(&cur_html);
        if old_ing.is_empty() && cur_ing.is_empty() {
            continue; // unusable parse — skip WITHOUT counting or decrementing
        }
        examined += 1;

        let product = entry
            .title
            .rfind('[')
            .map(|i| entry.title[..i].trim())
            .unwrap_or(&entry.title)
            .to_string();
        let added: BTreeSet<String> = cur_ing.difference(&old_ing).cloned().collect();
        let removed: BTreeSet<String> = old_ing.difference(&cur_ing).cloned().collect();
        let vocab: BTreeSet<String> = old_ing.union(&cur_ing).cloned().collect();
        let og = grounded(&product, &vocab, &strip_html(&old_html)).await?;
        let cg = grounded(&product, &vocab, &strip_html(&cur_html)).await?;

        let reform = !added.is_empty() || !removed.is_empty();
        if reform {
            reformulation += 1;
            struct_add += added.len();
            struct_rm += removed.len();
            narr_add += added.iter().filter(|n| cg.contains(&entity_key(n))).count();
            narr_rm += removed
                .iter()
                .filter(|n| og.contains(&entity_key(n)) && !cg.contains(&entity_key(n)))
                .count();
        } else {
            admin += 1;
            if og == cg {
                admin_stable += 1;
            }
        }
        rows.push(json!({"set_id": sid, "old": ov, "current": cv,
            "active": {"old": old_ing.len(), "current": cur_ing.len(),
                       "added": added.len(), "removed": removed.len()},
            "class": if reform { "reformulation" } else { "administrative" }}));
        println!(
            "{:8} v{ov}->v{cv}  active {}->{} (+{}/-{})  [{}]",
            &sid[..8],
            old_ing.len(),
            cur_ing.len(),
            added.len(),
            removed.len(),
            if reform { "reformulation" } else { "admin" }
        );
    }

    println!(
        "\n=== Real-revision drift over {examined} multi-version labels (scanned {scanned}) ==="
    );
    println!("  taxonomy: {reformulation} reformulation, {admin} administrative-only");
    println!("  structured (name) delta: +{struct_add} / -{struct_rm} active ingredients");
    println!(
        "  narrative tracked: {narr_add}/{struct_add} additions, {narr_rm}/{struct_rm} removals grounded on the right side"
    );
    println!("  restraint on administrative-only: {admin_stable}/{admin} narrative-stable");
    println!("  NOTE: name-based HTML oracle — lower rigor than the XML gates (Method A caveat)");

    let out = PathBuf::from("data/dailymed/drift-real-results.json");
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&json!({
            "decision": "decision_dailymed_drift.md",
            "method": "A (real revisions; HTML ingredient-table name oracle; lower rigor)",
            "examined": examined, "scanned": scanned,
            "taxonomy": {"reformulation": reformulation, "administrative": admin},
            "structured_delta": {"added": struct_add, "removed": struct_rm},
            "narrative_tracking": {"added": narr_add, "removed": narr_rm},
            "administrative_stable": admin_stable,
            "rows": rows,
        }))?,
    )?;
    println!("saved       {}", out.display());
    Ok(())
}
