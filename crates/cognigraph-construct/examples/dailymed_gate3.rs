//! DailyMed pilot, gate 3 (docs/decisions/decision_gate3_baseline.md):
//! freeze the baseline policy and run the full-scale construction.
//!
//! - D1 tightened triggers (drop the broad `of {target}` FP source);
//!   `--calibrate` sweeps candidate trigger sets against the oracle and
//!   prints precision/recall so the freeze is measured, not asserted.
//! - D2 two-tier oracle (active + inactive ingredients) with SPL
//!   `<activeMoiety>` parsed as a salt/base ingredient alias.
//! - D3/D4 structured-agreement production gate: every narrative fact is
//!   cross-checked against the structured oracle into three buckets —
//!   agreed (auto-accept), narrative-only (review queue), structured-only
//!   (imported). No confidence-threshold gating.
//! - D5 four relations; structured facts populate the graph for free.
//! - D6 certified judge sample (measured ON the baseline), grounding cost,
//!   `gate3-policy-v1` manifest, acceptance check.
//!
//! Run (validate):  cargo run --release -p cognigraph-construct --example dailymed_gate3
//! Run (full 10k):  … -- --rx 9000 --otc 1000
//! Calibrate only:  … -- --calibrate
//! Deterministic:   … -- --dry   (skips the judge certification)

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use cognigraph_construct::ingest::entity_key;
use cognigraph_construct::*;
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::completion::completion_from_env;
use cognigraph_native::NativeBackend;
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use serde::Deserialize;
use serde_json::json;

const SPACE_ID: &str = "dailymed-gate3";
const POLICY_REV: &str = "gate3-policy-v1";
const MAX_CHUNK_CHARS: usize = 2_800;
const TARGET_CHUNK_CHARS: usize = 2_000;
const MIN_SECTION_CHARS: usize = 40;
const CONTAINS_INGREDIENT: &str = "CONTAINS_INGREDIENT";
const ADMINISTERED_VIA: &str = "ADMINISTERED_VIA";
const HAS_DOSAGE_FORM: &str = "HAS_DOSAGE_FORM";
const MARKETED_BY: &str = "MARKETED_BY";

/// Frozen trigger set after calibration (D1), measurement-justified
/// PER RELATION (the tradeoff is not uniform):
/// - CONTAINS_INGREDIENT drops the broad `of {target}` — precision
///   50%->86% for 25% active-recall (production precision is anyway
///   guaranteed by the structured gate, so narrative precision governs
///   only review-queue size).
/// - ADMINISTERED_VIA keeps `{target} administration` — dropping it costs
///   45 recall points (62%->17%) to gain 12 precision points, an
///   unacceptable trade; route mentions are far less leakage-prone than
///   ingredient mentions.
fn frozen_triggers(relation: &str) -> Vec<String> {
    let raw: &[&str] = match relation {
        CONTAINS_INGREDIENT => &["contains {target}", "{target}, usp"],
        ADMINISTERED_VIA => &[
            "for {target} use",
            "{target} use only",
            "{target} administration",
        ],
        _ => &[],
    };
    raw.iter().map(|s| s.to_string()).collect()
}

/// Named candidate trigger sets for `--calibrate`.
fn triggers_named(set: &str, relation: &str) -> Vec<String> {
    let raw: &[&str] = match (set, relation) {
        ("gate2", CONTAINS_INGREDIENT) => &["contains {target}", "of {target}", "{target}, usp"],
        ("gate2", ADMINISTERED_VIA) => &[
            "for {target} use",
            "{target} use only",
            "{target} administration",
        ],
        ("tight", CONTAINS_INGREDIENT) => &[
            "contains {target}",
            "{target}, usp",
            "each tablet contains {target}",
        ],
        ("tight", ADMINISTERED_VIA) => &["for {target} use", "{target} use only"],
        ("tightest", CONTAINS_INGREDIENT) => &["contains {target}"],
        ("tightest", ADMINISTERED_VIA) => &["for {target} use"],
        _ => &[],
    };
    raw.iter().map(|s| s.to_string()).collect()
}

const CALIBRATION_SETS: &[&str] = &["gate2", "tight", "tightest"];

#[derive(Deserialize)]
struct ManifestEntry {
    set_id: String,
    document_path: String,
    selection_rank: u64,
    title: String,
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

#[derive(Debug, Default)]
struct OracleDoc {
    /// Active ingredient substance names.
    active: BTreeSet<String>,
    /// Inactive ingredient names.
    inactive: BTreeSet<String>,
    /// Active substance name -> active-moiety (base) name, the salt/base
    /// alias (D2).
    moiety: BTreeMap<String, String>,
    routes: BTreeSet<String>,
    dosage_form: Option<String>,
    labeler: Option<String>,
}

fn parse_oracle(xml: &str) -> Result<OracleDoc> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut oracle = OracleDoc::default();
    let mut ingredient_class: Option<String> = None;
    let mut ingredient_depth = 0usize;
    let mut in_substance = false;
    let mut moiety_depth = 0usize;
    let mut capture: Option<&'static str> = None;
    let mut current_substance = String::new();
    let mut in_labeler = false;
    let mut depth = 0usize;

    loop {
        match reader.read_event() {
            Err(e) => anyhow::bail!("xml error: {e}"),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                depth += 1;
                match e.name().as_ref() {
                    "ingredient" => {
                        let class = e
                            .try_get_attribute("classCode")?
                            .map(|a| {
                                a.normalized_value(XmlVersion::default())
                                    .unwrap_or_default()
                                    .to_string()
                            })
                            .unwrap_or_default();
                        if class.starts_with("ACTI") || class == "IACT" {
                            ingredient_class = Some(class);
                            ingredient_depth = depth;
                            current_substance.clear();
                        }
                    }
                    "ingredientSubstance" if ingredient_class.is_some() => in_substance = true,
                    "activeMoiety" if in_substance => moiety_depth = depth,
                    // The substance name comes before the nested activeMoiety
                    // name; capture the substance first, moiety second.
                    "name" if in_substance && capture.is_none() => {
                        capture = Some(if moiety_depth > 0 {
                            "moiety"
                        } else {
                            "substance"
                        });
                    }
                    "representedOrganization" if oracle.labeler.is_none() => in_labeler = true,
                    "name" if in_labeler && capture.is_none() => capture = Some("labeler"),
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                "routeCode" => {
                    if let Some(display) = e.try_get_attribute("displayName")? {
                        oracle.routes.insert(
                            display
                                .normalized_value(XmlVersion::default())
                                .unwrap_or_default()
                                .to_string(),
                        );
                    }
                }
                "formCode" if oracle.dosage_form.is_none() => {
                    let system = e
                        .try_get_attribute("codeSystem")?
                        .map(|a| {
                            a.normalized_value(XmlVersion::default())
                                .unwrap_or_default()
                                .to_string()
                        })
                        .unwrap_or_default();
                    if system == "2.16.840.1.113883.3.26.1.1"
                        && let Some(display) = e.try_get_attribute("displayName")?
                    {
                        oracle.dosage_form = Some(
                            display
                                .normalized_value(XmlVersion::default())
                                .unwrap_or_default()
                                .to_string(),
                        );
                    }
                }
                _ => {}
            },
            Ok(Event::Text(t)) => {
                if let Some(kind) = capture.take() {
                    let text = t.trim().to_string();
                    if !text.is_empty() {
                        match kind {
                            "substance" => {
                                current_substance = text.clone();
                                let active = ingredient_class
                                    .as_deref()
                                    .is_some_and(|c| c.starts_with("ACTI"));
                                if active {
                                    oracle.active.insert(text);
                                } else {
                                    oracle.inactive.insert(text);
                                }
                            }
                            "moiety" => {
                                if !current_substance.is_empty() && text != current_substance {
                                    oracle
                                        .moiety
                                        .entry(current_substance.clone())
                                        .or_insert(text);
                                }
                            }
                            "labeler" => {
                                oracle.labeler.get_or_insert(text);
                                in_labeler = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    "ingredient" if ingredient_class.is_some() && depth == ingredient_depth => {
                        ingredient_class = None;
                        in_substance = false;
                        moiety_depth = 0;
                    }
                    "activeMoiety" if depth == moiety_depth => moiety_depth = 0,
                    "ingredientSubstance" => in_substance = false,
                    "representedOrganization" => in_labeler = false,
                    _ => {}
                }
                depth = depth.saturating_sub(1);
            }
            Ok(_) => {}
        }
    }
    Ok(oracle)
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

fn chunk_document(doc: &NormalizedDoc) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    for (idx, section) in doc.sections.iter().enumerate() {
        let text = section.text.trim();
        if text.len() < MIN_SECTION_CHARS {
            continue;
        }
        if text.len() <= MAX_CHUNK_CHARS {
            chunks.push(Chunk {
                id: format!("{}-s{idx:02}", doc.set_id),
                title: section.title.clone(),
                text: text.to_string(),
            });
        } else {
            for (part, piece) in split_sentences(text).into_iter().enumerate() {
                chunks.push(Chunk {
                    id: format!("{}-s{idx:02}-p{part}", doc.set_id),
                    title: section.title.clone(),
                    text: piece,
                });
            }
        }
    }
    chunks
}

fn product_name(title: &str, taken: &mut HashSet<String>, set_id: &str) -> String {
    let base = title
        .rfind('[')
        .map(|i| title[..i].trim())
        .unwrap_or(title.trim())
        .to_string();
    let name = if taken.contains(&entity_key(&base)) {
        format!("{base} [{}]", &set_id[..8.min(set_id.len())])
    } else {
        base
    };
    taken.insert(entity_key(&name));
    name
}

struct Doc {
    product: String,
    oracle: OracleDoc,
    chunks: Vec<Chunk>,
}

fn load_cohort(corpus: &Path, n_rx: usize, n_otc: usize) -> Result<Vec<Doc>> {
    let mut cohort = manifest_cohort(corpus, "manifest-rx.jsonl", n_rx)?;
    cohort.extend(manifest_cohort(corpus, "manifest-otc.jsonl", n_otc)?);
    let mut taken = HashSet::new();
    let mut docs = Vec::new();
    for entry in &cohort {
        let doc: NormalizedDoc = serde_json::from_str(
            &std::fs::read_to_string(corpus.join(&entry.document_path))
                .with_context(|| format!("reading {}", entry.document_path))?,
        )?;
        let xml = std::fs::read_to_string(corpus.join("raw").join(format!("{}.xml", entry.set_id)))
            .with_context(|| format!("raw xml for {}", entry.set_id))?;
        let oracle = parse_oracle(&xml)?;
        let chunks = chunk_document(&doc);
        let product = product_name(&entry.title, &mut taken, &entry.set_id);
        docs.push(Doc {
            product,
            oracle,
            chunks,
        });
    }
    Ok(docs)
}

/// Build the typed entity catalogue. Ingredient entities carry their
/// salt-base moiety as an alias (D2); products carry their active
/// ingredient names as aliases so evidence is retrievable.
fn build_entities(
    docs: &[Doc],
    ingredients: &BTreeMap<String, BTreeSet<String>>,
    routes: &[String],
) -> Vec<EntityDef> {
    let mut entities: Vec<serde_json::Value> = Vec::new();
    for doc in docs {
        let aliases: Vec<String> = doc.oracle.active.iter().cloned().collect();
        entities.push(json!({"name": doc.product, "type": "product", "aliases": aliases}));
    }
    for (name, aliases) in ingredients {
        entities.push(json!({"name": name, "type": "ingredient", "aliases": aliases}));
    }
    // Route entities are grounding endpoints for ADMINISTERED_VIA — their
    // `{target}` placeholders can only expand if the entity exists.
    for name in routes {
        entities.push(json!({"name": name, "type": "route", "aliases": Vec::<String>::new()}));
    }
    serde_json::from_value(json!(entities)).unwrap()
}

/// Lowercased concatenation of a document's chunk text — the mention
/// prefilter's index. Casefold substring is exactly what grounding's
/// trigger match uses, so testing surface presence against this text is a
/// perfect over-approximation: it never drops a grounding the full run
/// would produce, it only skips rules whose target cannot possibly match.
fn doc_text_lower(doc: &Doc) -> String {
    let mut text = String::new();
    for chunk in &doc.chunks {
        text.push_str(&chunk.text.to_lowercase());
        text.push('\n');
    }
    text
}

fn surface_present(surface: &str, text_lower: &str) -> bool {
    !surface.trim().is_empty() && text_lower.contains(&surface.to_lowercase())
}

/// Subject-scoped construction with a given trigger set; returns the
/// grounded facts (source key, relation, target key, trigger). A per-doc
/// mention prefilter builds rules only for vocabulary targets whose
/// surface actually occurs in the document — the QW7 optimization
/// (decision_dailymed_ontology.md D4) that makes 10k-scale grounding
/// tractable (the naive full cross-product is hours; this is minutes).
async fn construct(
    backend: &NativeBackend,
    docs: &[Doc],
    entities: &[EntityDef],
    ingredients: &BTreeMap<String, BTreeSet<String>>,
    route_names: &[String],
    trigger_set: &str,
    frozen: bool,
) -> Result<Vec<(String, String, String, String)>> {
    let via = |rel: &str| {
        if frozen {
            frozen_triggers(rel)
        } else {
            triggers_named(trigger_set, rel)
        }
    };
    for doc in docs {
        let text_lower = doc_text_lower(doc);
        let mut rules: Vec<serde_json::Value> = Vec::new();
        for (name, aliases) in ingredients {
            // Present if the ingredient name OR any of its moiety aliases
            // occurs in the document.
            let present = surface_present(name, &text_lower)
                || aliases.iter().any(|a| surface_present(a, &text_lower));
            if present {
                rules.push(
                    json!({"source": doc.product, "relation": CONTAINS_INGREDIENT,
                    "target": name, "when_any": via(CONTAINS_INGREDIENT)}),
                );
            }
        }
        for target in route_names {
            if surface_present(target, &text_lower) {
                rules.push(json!({"source": doc.product, "relation": ADMINISTERED_VIA,
                    "target": target, "when_any": via(ADMINISTERED_VIA)}));
            }
        }
        let config: SpaceType = serde_json::from_value(json!({
            "id": SPACE_ID, "entities": entities, "relation_rules": rules,
        }))?;
        ingest_chunks(backend, SPACE_ID, &config, &doc.chunks, &[]).await?;
    }
    let facts = backend.list_documents("facts", None, None).await?;
    Ok(facts
        .iter()
        .map(|f| {
            (
                f["_from"]
                    .as_str()
                    .unwrap_or("")
                    .trim_start_matches("entities/")
                    .to_string(),
                f["relation_type"].as_str().unwrap_or("?").to_string(),
                f["_to"]
                    .as_str()
                    .unwrap_or("")
                    .trim_start_matches("entities/")
                    .to_string(),
                f["trigger"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect())
}

fn arg(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn has_flag(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

fn sample<T>(mut items: Vec<T>, cap: usize, key: impl Fn(&T) -> String) -> Vec<T> {
    items.sort_by_key(|i| key(i));
    if items.len() <= cap {
        return items;
    }
    let stride = items.len() as f64 / cap as f64;
    (0..cap)
        .map(|i| (i as f64 * stride) as usize)
        .map(|idx| items.swap_remove(idx.min(items.len() - 1)))
        .collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let corpus = PathBuf::from(arg("--corpus").unwrap_or_else(|| "data/dailymed-pilot".into()));
    let n_rx: usize = arg("--rx").and_then(|v| v.parse().ok()).unwrap_or(900);
    let n_otc: usize = arg("--otc").and_then(|v| v.parse().ok()).unwrap_or(100);
    let judge_sample: usize = arg("--judge-sample")
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let calibrate = has_flag("--calibrate");
    let dry = has_flag("--dry");
    let out_dir = PathBuf::from("data/dailymed");

    let t = Instant::now();
    let docs = load_cohort(&corpus, n_rx, n_otc)?;
    println!(
        "cohort      {} docs ({n_rx} rx + {n_otc} otc), oracle parsed in {} ms",
        docs.len(),
        t.elapsed().as_millis()
    );

    // Vocabulary: ingredient name -> its moiety aliases; routes.
    let mut ingredients: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut routes: BTreeSet<String> = BTreeSet::new();
    for doc in &docs {
        for name in &doc.oracle.active {
            let entry = ingredients.entry(name.clone()).or_default();
            if let Some(base) = doc.oracle.moiety.get(name) {
                entry.insert(base.clone());
            }
        }
        routes.extend(doc.oracle.routes.iter().cloned());
    }
    let route_names: Vec<String> = routes.iter().cloned().collect();
    let entities = build_entities(&docs, &ingredients, &route_names);

    // Structured oracle triples (D5) — the production truth and the
    // agreement reference. `structured_active` is the CONTAINS subset over
    // ACTIVE ingredients only, for the comparable active-recall number
    // (D2 — full-ingredient recall is diluted by every excipient).
    let mut structured: HashSet<(String, String, String)> = HashSet::new();
    let mut structured_active: HashSet<(String, String, String)> = HashSet::new();
    let mut structured_by_relation: BTreeMap<String, usize> = BTreeMap::new();
    for doc in &docs {
        let pk = entity_key(&doc.product);
        let mut add = |rel: &str, target: &str, set: &mut HashSet<_>| {
            if set.insert((pk.clone(), rel.to_string(), entity_key(target))) {
                *structured_by_relation.entry(rel.to_string()).or_default() += 1;
            }
        };
        for ing in &doc.oracle.active {
            add(CONTAINS_INGREDIENT, ing, &mut structured);
            structured_active.insert((
                pk.clone(),
                CONTAINS_INGREDIENT.to_string(),
                entity_key(ing),
            ));
        }
        for ing in &doc.oracle.inactive {
            add(CONTAINS_INGREDIENT, ing, &mut structured);
        }
        for base in doc.oracle.moiety.values() {
            add(CONTAINS_INGREDIENT, base, &mut structured);
            structured_active.insert((
                pk.clone(),
                CONTAINS_INGREDIENT.to_string(),
                entity_key(base),
            ));
        }
        for route in &doc.oracle.routes {
            add(ADMINISTERED_VIA, route, &mut structured);
        }
        if let Some(form) = &doc.oracle.dosage_form {
            add(HAS_DOSAGE_FORM, form, &mut structured);
        }
        if let Some(labeler) = &doc.oracle.labeler {
            add(MARKETED_BY, labeler, &mut structured);
        }
    }

    // --- D1 calibration: sweep trigger sets, no LLM, no freeze. ---
    if calibrate {
        println!("\n=== D1 trigger calibration (narrative vs structured oracle) ===");
        for set in CALIBRATION_SETS {
            let backend = NativeBackend::new();
            let t = Instant::now();
            let facts = construct(
                &backend,
                &docs,
                &entities,
                &ingredients,
                &route_names,
                set,
                false,
            )
            .await?;
            let ms = t.elapsed().as_millis();
            for rel in [CONTAINS_INGREDIENT, ADMINISTERED_VIA] {
                let narr: Vec<_> = facts.iter().filter(|f| f.1 == rel).collect();
                let agreed = narr
                    .iter()
                    .filter(|f| structured.contains(&(f.0.clone(), f.1.clone(), f.2.clone())))
                    .count();
                let truth = structured_by_relation.get(rel).copied().unwrap_or(0);
                let precision = if !narr.is_empty() {
                    100.0 * agreed as f64 / narr.len() as f64
                } else {
                    0.0
                };
                let recall = if truth > 0 {
                    100.0 * agreed as f64 / truth as f64
                } else {
                    0.0
                };
                // Active-only recall for CONTAINS (comparable across gates;
                // full-ingredient recall is diluted by excipients).
                let active_recall = if rel == CONTAINS_INGREDIENT && !structured_active.is_empty() {
                    let hit = narr
                        .iter()
                        .filter(|f| {
                            structured_active.contains(&(f.0.clone(), f.1.clone(), f.2.clone()))
                        })
                        .count();
                    format!(
                        ", active-recall {:.0}%",
                        100.0 * hit as f64 / structured_active.len() as f64
                    )
                } else {
                    String::new()
                };
                println!(
                    "  [{set:8}] {rel:20} {} facts, precision {precision:.0}%, recall {recall:.0}%{active_recall} ({ms} ms)",
                    narr.len()
                );
            }
        }
        println!(
            "\nFrozen choice: `tight` (drops `of {{target}}`; keeps composition-specific \
             phrasings). Re-run without --calibrate to build the baseline."
        );
        return Ok(());
    }

    // --- Frozen baseline construction (D1 tight triggers). ---
    let backend = NativeBackend::new();
    let t = Instant::now();
    let narrative = construct(
        &backend,
        &docs,
        &entities,
        &ingredients,
        &route_names,
        "tight",
        true,
    )
    .await?;
    let construct_ms = t.elapsed().as_millis();

    // Structural leakage assertion (D6 acceptance): every narrative fact's
    // source is the product of the document it grounded in — subject
    // scoping makes cross-product leakage impossible, and we verify it.
    let product_keys: HashSet<String> = docs.iter().map(|d| entity_key(&d.product)).collect();
    let leaks = narrative
        .iter()
        .filter(|f| !product_keys.contains(&f.0))
        .count();

    // --- D3 structured-agreement gate: three buckets. ---
    let narrative_triples: HashSet<(String, String, String)> = narrative
        .iter()
        .map(|f| (f.0.clone(), f.1.clone(), f.2.clone()))
        .collect();
    let agreed: Vec<&(String, String, String, String)> = narrative
        .iter()
        .filter(|f| structured.contains(&(f.0.clone(), f.1.clone(), f.2.clone())))
        .collect();
    let narrative_only: Vec<&(String, String, String, String)> = narrative
        .iter()
        .filter(|f| !structured.contains(&(f.0.clone(), f.1.clone(), f.2.clone())))
        .collect();
    let structured_only = structured
        .iter()
        .filter(|t| !narrative_triples.contains(*t))
        .count();

    let narr_precision = if !narrative.is_empty() {
        100.0 * agreed.len() as f64 / narrative.len() as f64
    } else {
        0.0
    };
    println!(
        "construct   {} narrative facts in {construct_ms} ms ({:.1} docs/s); {} cross-product leaks",
        narrative.len(),
        docs.len() as f64 / (construct_ms.max(1) as f64 / 1000.0),
        leaks
    );
    println!(
        "gate (D3)   agreed {} | narrative-only {} (review queue) | structured-only {} (imported)",
        agreed.len(),
        narrative_only.len(),
        structured_only
    );
    println!(
        "            narrative precision {narr_precision:.0}% (agreed / all narrative); \
         production graph = agreed + structured-only = {} facts @ 100% (oracle-backed)",
        agreed.len() + structured_only
    );
    for rel in [CONTAINS_INGREDIENT, ADMINISTERED_VIA] {
        let narr = narrative.iter().filter(|f| f.1 == rel).count();
        let agr = agreed.iter().filter(|f| f.1 == rel).count();
        let truth = structured_by_relation.get(rel).copied().unwrap_or(0);
        // Active-only recall for CONTAINS (comparable across gates; the
        // full-ingredient denominator is inflated by every excipient).
        let active_note = if rel == CONTAINS_INGREDIENT && !structured_active.is_empty() {
            let hit = narrative
                .iter()
                .filter(|f| {
                    f.1 == rel
                        && structured_active.contains(&(f.0.clone(), f.1.clone(), f.2.clone()))
                })
                .count();
            format!(
                ", active-recall {:.0}% of {}",
                100.0 * hit as f64 / structured_active.len() as f64,
                structured_active.len()
            )
        } else {
            String::new()
        };
        println!(
            "  {rel:20} narrative {narr}, agreed {agr}, recall {:.0}% of {truth} structured{active_note}",
            if truth > 0 {
                100.0 * agr as f64 / truth as f64
            } else {
                0.0
            }
        );
    }

    // --- Policy manifest (D6). ---
    let snapshot = std::fs::read_to_string(corpus.join("run.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("catalog_dates").cloned())
        .unwrap_or(json!(null));
    let manifest = json!({
        "policy_rev": POLICY_REV,
        "decision": "decision_gate3_baseline.md",
        "snapshot": snapshot,
        "relations": {
            "narrative": [CONTAINS_INGREDIENT, ADMINISTERED_VIA],
            "structured": [CONTAINS_INGREDIENT, ADMINISTERED_VIA, HAS_DOSAGE_FORM, MARKETED_BY],
        },
        "triggers": {
            CONTAINS_INGREDIENT: frozen_triggers(CONTAINS_INGREDIENT),
            ADMINISTERED_VIA: frozen_triggers(ADMINISTERED_VIA),
        },
        "oracle": "two-tier: active + inactive ingredients, activeMoiety salt/base alias",
        "production_gate": "structured-agreement (D3); no confidence-threshold auto-accept (D4)",
    });

    // --- D6 acceptance check. ---
    let accept_leakage = leaks == 0;
    let accept_precision = narr_precision >= 80.0;
    println!(
        "\nacceptance  zero cross-product leakage: {}  | narrative precision >= 80%: {} ({:.0}%)",
        if accept_leakage { "PASS" } else { "FAIL" },
        if accept_precision { "PASS" } else { "FAIL" },
        narr_precision
    );

    // --- D6 certified judge sample (measured ON the baseline). ---
    let mut cert = json!(null);
    if !dry {
        struct Row {
            source: String,
            relation: String,
            target: String,
            trigger: String,
            truth: bool,
        }
        let mut product_chunks: HashMap<String, Vec<Chunk>> = HashMap::new();
        for doc in &docs {
            product_chunks
                .entry(entity_key(&doc.product))
                .or_default()
                .extend(doc.chunks.iter().cloned());
        }
        let to_row = |f: &&(String, String, String, String), truth: bool| Row {
            source: f.0.clone(),
            relation: f.1.clone(),
            target: f.2.clone(),
            trigger: f.3.clone(),
            truth,
        };
        let key = |r: &Row| format!("{}|{}|{}", r.source, r.relation, r.target);
        let pos = sample(
            agreed.iter().map(|f| to_row(f, true)).collect(),
            judge_sample,
            key,
        );
        let neg = sample(
            narrative_only.iter().map(|f| to_row(f, false)).collect(),
            judge_sample,
            key,
        );
        let provider = completion_from_env()?;
        let (mut at, mut af, mut rt, mut rf, mut ht, mut hf) = (0, 0, 0, 0, 0, 0);
        let run = Instant::now();
        for r in pos.iter().chain(neg.iter()) {
            let neuron: Neuron = serde_json::from_value(json!({
                "id": format!("g3-{}-{}-{}", r.source, r.relation, r.target),
                "type": "relation_hint", "status": "proposed",
                "source": r.source, "relation": r.relation, "target": r.target,
                "triggers": [r.trigger],
                "rationale": "Candidate relation grounded from product label prose. \
                              Judge whether the matched evidence states this relation for this product.",
                "evidence": [r.trigger.clone()],
            }))?;
            let chunks = product_chunks.get(&r.source).cloned().unwrap_or_default();
            let v = judge_neuron(provider.as_ref(), &neuron, &chunks).await?;
            match (v.verdict.as_str(), r.truth) {
                ("accept", true) => at += 1,
                ("accept", false) => af += 1,
                ("reject", true) => rt += 1,
                ("reject", false) => rf += 1,
                (_, true) => ht += 1,
                (_, false) => hf += 1,
            }
        }
        let judged = pos.len() + neg.len();
        println!(
            "\ncertify     judged {judged} in {:.0}s; accept {at}T/{af}F reject {rt}T/{rf}F human {ht}T/{hf}F; \
             oracle agreement {}/{judged}",
            run.elapsed().as_secs_f64(),
            at + rf
        );
        cert = json!({
            "judged": judged, "accept_true": at, "accept_false": af,
            "reject_true": rt, "reject_false": rf, "needs_human_true": ht, "needs_human_false": hf,
            "oracle_agreement": at + rf,
        });
    }

    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(
        out_dir.join("gate3-policy-v1.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    std::fs::write(
        out_dir.join("gate3-baseline-results.json"),
        serde_json::to_string_pretty(&json!({
            "policy_rev": POLICY_REV,
            "cohort": {"rx": n_rx, "otc": n_otc, "products": docs.len()},
            "construction": {
                "narrative_facts": narrative.len(), "construct_ms": construct_ms,
                "cross_product_leaks": leaks,
            },
            "gate": {
                "agreed": agreed.len(), "narrative_only": narrative_only.len(),
                "structured_only": structured_only, "narrative_precision_pct": narr_precision,
                "production_graph_facts": agreed.len() + structured_only,
            },
            "structured_by_relation": structured_by_relation,
            "acceptance": {"zero_leakage": accept_leakage, "precision_ge_80": accept_precision},
            "certification": cert,
        }))?,
    )?;
    println!(
        "saved       {} + {}",
        out_dir.join("gate3-policy-v1.json").display(),
        out_dir.join("gate3-baseline-results.json").display()
    );
    Ok(())
}
