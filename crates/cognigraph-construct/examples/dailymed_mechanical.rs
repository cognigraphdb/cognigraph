//! DailyMed mechanical pass (decision_dailymed_ontology.md, D1–D6):
//! construct the mechanically-checkable relations from narrative prose
//! and score them against the SPL structured-data oracle.
//!
//! Pipeline:
//! 1. Cohort + section-aware chunking (same discipline as dailymed_gate1).
//! 2. Oracle extraction from the cached raw SPL XML (no network): active
//!    ingredients (UNII), routes, dosage form, labeler — per document.
//! 3. Typed vocabulary (D3) from the oracle across the WHOLE cohort —
//!    vocabulary is public; per-document pairs are the answers and never
//!    reach rule generation (D4).
//! 4. Candidate rules: typed cross-product with authored template
//!    triggers, every trigger referencing `{target}` (D4). Grounding is
//!    subject-scoped: each document is ingested with only its own
//!    product's rule subset (D5 — structural restraint).
//! 5. Structured facts imported into `facts_structured` with
//!    `provenance: "structured"` (D2 — never sharing an edge with
//!    narrative facts, whose upsert key ignores provenance).
//! 6. Score narrative facts against the oracle: per-relation
//!    precision/recall + the restraint inspection list (D6).
//!
//! Deterministic, offline, zero LLM calls, zero embeddings.
//!
//! Run: cargo run --release -p cognigraph-construct --example dailymed_mechanical -- \
//!        [--rx 90] [--otc 10] [--corpus data/dailymed-pilot]

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use cognigraph_construct::ingest::entity_key;
use cognigraph_construct::*;
use cognigraph_core::{CollectionType, GraphBackend};
use cognigraph_native::NativeBackend;
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};
use serde::Deserialize;
use serde_json::json;

const SPACE_ID: &str = "dailymed-mechanical";
const MAX_CHUNK_CHARS: usize = 2_800;
const TARGET_CHUNK_CHARS: usize = 2_000;
const MIN_SECTION_CHARS: usize = 40;

const CONTAINS_INGREDIENT: &str = "CONTAINS_INGREDIENT";
const ADMINISTERED_VIA: &str = "ADMINISTERED_VIA";
const HAS_DOSAGE_FORM: &str = "HAS_DOSAGE_FORM";
const MARKETED_BY: &str = "MARKETED_BY";

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

/// Per-document mechanical truth, straight from the SPL structured data.
#[derive(Debug, Default)]
struct OracleDoc {
    /// Active ingredient name → UNII code.
    ingredients: BTreeMap<String, String>,
    routes: BTreeSet<String>,
    dosage_form: Option<String>,
    labeler: Option<String>,
}

/// Stream the cached SPL XML for the structured elements the mechanical
/// relations need. Element paths, not regexes: ingredients are the
/// `<ingredient classCode="ACTI*">` blocks' substance names, the dosage
/// form is the FIRST product `formCode`, the labeler is the first
/// `representedOrganization` name.
fn parse_oracle(xml: &str) -> Result<OracleDoc> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut oracle = OracleDoc::default();

    let mut in_active_ingredient = false;
    let mut ingredient_depth = 0usize;
    let mut pending_unii = String::new();
    let mut in_substance = false;
    let mut capture_text_for: Option<&'static str> = None;
    let mut in_labeler_org = false;
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
                        if class.starts_with("ACTI") {
                            in_active_ingredient = true;
                            ingredient_depth = depth;
                            pending_unii.clear();
                        }
                    }
                    "ingredientSubstance" if in_active_ingredient => in_substance = true,
                    "name" if in_substance && capture_text_for.is_none() => {
                        capture_text_for = Some("ingredient");
                    }
                    "representedOrganization" if oracle.labeler.is_none() => {
                        in_labeler_org = true;
                    }
                    "name" if in_labeler_org && capture_text_for.is_none() => {
                        capture_text_for = Some("labeler");
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => match e.name().as_ref() {
                "code" if in_substance && pending_unii.is_empty() => {
                    if let Some(code) = e.try_get_attribute("code")? {
                        pending_unii = code
                            .normalized_value(XmlVersion::default())
                            .unwrap_or_default()
                            .to_string();
                    }
                }
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
                if let Some(kind) = capture_text_for.take() {
                    let text = t.trim().to_string();
                    if !text.is_empty() {
                        match kind {
                            "ingredient" => {
                                oracle
                                    .ingredients
                                    .entry(text)
                                    .or_insert_with(|| pending_unii.clone());
                            }
                            "labeler" => {
                                if oracle.labeler.is_none() {
                                    oracle.labeler = Some(text);
                                }
                                in_labeler_org = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    "ingredient" if in_active_ingredient && depth == ingredient_depth => {
                        in_active_ingredient = false;
                        in_substance = false;
                    }
                    "ingredientSubstance" => in_substance = false,
                    "representedOrganization" => in_labeler_org = false,
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

/// Product display name: the manifest title without the trailing
/// `[LABELER]` bracket, uniquified on collision.
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

/// Authored template triggers (D4): every trigger references `{target}`
/// — the source is credited structurally by subject scoping (D5), so no
/// trigger may fire on target-free prose.
fn triggers_for(relation: &str) -> Vec<String> {
    let raw: &[&str] = match relation {
        CONTAINS_INGREDIENT => &["contains {target}", "of {target}", "{target}, usp"],
        ADMINISTERED_VIA => &[
            "for {target} use",
            "{target} use only",
            "{target} administration",
        ],
        HAS_DOSAGE_FORM => &["supplied as {target}", "available as {target}"],
        MARKETED_BY => &[
            "manufactured by {target}",
            "distributed by {target}",
            "manufactured for {target}",
            "marketed by {target}",
        ],
        _ => &[],
    };
    raw.iter().map(|s| s.to_string()).collect()
}

fn arg(flag: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let corpus = PathBuf::from(arg("--corpus").unwrap_or_else(|| "data/dailymed-pilot".into()));
    let n_rx: usize = arg("--rx").and_then(|v| v.parse().ok()).unwrap_or(90);
    let n_otc: usize = arg("--otc").and_then(|v| v.parse().ok()).unwrap_or(10);
    let out_dir = PathBuf::from("data/dailymed");

    // 1+2. Cohort, chunks, oracle.
    let mut cohort = manifest_cohort(&corpus, "manifest-rx.jsonl", n_rx)?;
    cohort.extend(manifest_cohort(&corpus, "manifest-otc.jsonl", n_otc)?);
    let mut taken = HashSet::new();
    let mut docs: Vec<(String, NormalizedDoc, OracleDoc, Vec<Chunk>)> = Vec::new();
    let t = Instant::now();
    for entry in &cohort {
        let doc: NormalizedDoc = serde_json::from_str(
            &std::fs::read_to_string(corpus.join(&entry.document_path))
                .with_context(|| format!("reading {}", entry.document_path))?,
        )?;
        let xml = std::fs::read_to_string(corpus.join("raw").join(format!("{}.xml", entry.set_id)))
            .with_context(|| format!("raw xml for {}", entry.set_id))?;
        let oracle = parse_oracle(&xml).with_context(|| format!("oracle for {}", entry.set_id))?;
        let chunks = chunk_document(&doc);
        let product = product_name(&entry.title, &mut taken, &entry.set_id);
        docs.push((product, doc, oracle, chunks));
    }
    println!(
        "oracle      {} docs parsed in {} ms",
        docs.len(),
        t.elapsed().as_millis()
    );

    // 3. Typed vocabulary across the WHOLE cohort (public; not answers).
    let mut ingredients: BTreeMap<String, String> = BTreeMap::new(); // name -> UNII
    let mut routes: BTreeSet<String> = BTreeSet::new();
    let mut forms: BTreeSet<String> = BTreeSet::new();
    let mut labelers: BTreeSet<String> = BTreeSet::new();
    for (_, _, oracle, _) in &docs {
        for (name, unii) in &oracle.ingredients {
            ingredients.entry(name.clone()).or_insert(unii.clone());
        }
        routes.extend(oracle.routes.iter().cloned());
        forms.extend(oracle.dosage_form.iter().cloned());
        labelers.extend(oracle.labeler.iter().cloned());
    }
    println!(
        "vocabulary  {} products, {} ingredients, {} routes, {} forms, {} labelers",
        docs.len(),
        ingredients.len(),
        routes.len(),
        forms.len(),
        labelers.len()
    );

    let entity =
        |name: &str, ty: &str| json!({"name": name, "type": ty, "aliases": Vec::<String>::new()});
    let mut entities: Vec<serde_json::Value> = Vec::new();
    for (product, _, _, _) in &docs {
        entities.push(entity(product, "product"));
    }
    for name in ingredients.keys() {
        entities.push(entity(name, "ingredient"));
    }
    for name in &routes {
        entities.push(entity(name, "route"));
    }
    for name in &forms {
        entities.push(entity(name, "dosage_form"));
    }
    for name in &labelers {
        entities.push(entity(name, "labeler"));
    }
    let entities: Vec<EntityDef> = serde_json::from_value(json!(entities))?;

    // 4. Subject-scoped ingest (D5): per document, only that product's
    //    rule subset is active — cross-product leakage is impossible by
    //    construction. The rules themselves are the typed cross-product
    //    (D4): candidates for EVERY vocabulary target, oracle-blind.
    let backend = NativeBackend::new();
    let targets: Vec<(&str, Vec<String>)> = vec![
        (CONTAINS_INGREDIENT, ingredients.keys().cloned().collect()),
        (ADMINISTERED_VIA, routes.iter().cloned().collect()),
        (HAS_DOSAGE_FORM, forms.iter().cloned().collect()),
        (MARKETED_BY, labelers.iter().cloned().collect()),
    ];
    let mut candidate_rules = 0usize;
    let mut grounding_events = 0usize;
    let t = Instant::now();
    for (product, _, _, chunks) in &docs {
        let mut rules: Vec<serde_json::Value> = Vec::new();
        for (relation, names) in &targets {
            for target in names {
                rules.push(json!({
                    "source": product,
                    "relation": relation,
                    "target": target,
                    "when_any": triggers_for(relation),
                }));
            }
        }
        candidate_rules += rules.len();
        let config: SpaceType = serde_json::from_value(json!({
            "id": SPACE_ID,
            "entities": entities,
            "relation_rules": rules,
        }))?;
        grounding_events += ingest_chunks(&backend, SPACE_ID, &config, chunks, &[]).await?;
    }
    let ingest_ms = t.elapsed().as_millis();

    // Pin UNII codes onto ingredient entities (D3: codes as properties,
    // never grounding surfaces).
    for (name, unii) in &ingredients {
        let _ = backend
            .update_document(
                "entities",
                &entity_key(name),
                json!({"codes": {"unii": unii}}),
            )
            .await;
    }

    // 5. Structured import (D2): own collection, never sharing an edge
    //    with narrative facts.
    backend
        .ensure_collection("facts_structured", CollectionType::Edge)
        .await?;
    let mut oracle_triples: HashSet<(String, String, String)> = HashSet::new();
    for (product, doc, oracle, _) in &docs {
        let mut import = Vec::new();
        for name in oracle.ingredients.keys() {
            import.push((CONTAINS_INGREDIENT, name.clone()));
        }
        for route in &oracle.routes {
            import.push((ADMINISTERED_VIA, route.clone()));
        }
        if let Some(form) = &oracle.dosage_form {
            import.push((HAS_DOSAGE_FORM, form.clone()));
        }
        if let Some(labeler) = &oracle.labeler {
            import.push((MARKETED_BY, labeler.clone()));
        }
        for (relation, target) in import {
            backend
                .upsert_edge(
                    "facts_structured",
                    &format!("entities/{}", entity_key(product)),
                    &format!("entities/{}", entity_key(&target)),
                    relation,
                    json!({
                        "space_id": SPACE_ID,
                        "provenance": "structured",
                        "source_set_id": doc.set_id,
                    }),
                )
                .await?;
            oracle_triples.insert((
                entity_key(product),
                relation.to_string(),
                entity_key(&target),
            ));
        }
    }

    // 6. Score narrative facts against the oracle (D6).
    let narrative = backend.list_documents("facts", None, None).await?;
    let mut per_relation: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // (tp, fp)
    let mut restraint: Vec<String> = Vec::new();
    let mut narrative_triples: HashSet<(String, String, String)> = HashSet::new();
    for fact in &narrative {
        let from = fact["_from"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("entities/");
        let to = fact["_to"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("entities/");
        let relation = fact["relation_type"].as_str().unwrap_or("?");
        let triple = (from.to_string(), relation.to_string(), to.to_string());
        narrative_triples.insert(triple.clone());
        let hit = oracle_triples.contains(&triple);
        let slot = per_relation.entry(relation.to_string()).or_default();
        if hit {
            slot.0 += 1;
        } else {
            slot.1 += 1;
            restraint.push(format!(
                "{from} --{relation}--> {to}  [chunk {} trigger `{}`]",
                fact["evidence_chunk_id"].as_str().unwrap_or("?"),
                fact["trigger"].as_str().unwrap_or("?"),
            ));
        }
    }
    let mut oracle_per_relation: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // (found, total)
    for (source, relation, target) in &oracle_triples {
        let slot = oracle_per_relation.entry(relation.clone()).or_default();
        slot.1 += 1;
        if narrative_triples.contains(&(source.clone(), relation.clone(), target.clone())) {
            slot.0 += 1;
        }
    }

    println!(
        "grounding   {candidate_rules} candidate rules, {grounding_events} grounding events -> {} distinct narrative facts in {ingest_ms} ms",
        narrative.len()
    );
    println!(
        "scoring     narrative vs oracle ({} oracle facts):",
        oracle_triples.len()
    );
    for (relation, (found, total)) in &oracle_per_relation {
        let (tp, fp) = per_relation.get(relation).copied().unwrap_or((0, 0));
        let precision = if tp + fp > 0 {
            format!("{:.0}%", 100.0 * tp as f64 / (tp + fp) as f64)
        } else {
            "n/a".into()
        };
        println!(
            "  {relation:20} recall {found}/{total}  precision {precision} ({tp} true, {fp} outside oracle)"
        );
    }
    println!(
        "restraint   {} narrative facts outside the oracle:",
        restraint.len()
    );
    for line in restraint.iter().take(20) {
        println!("  inspect   {line}");
    }
    if restraint.len() > 20 {
        println!("  … {} more (see results json)", restraint.len() - 20);
    }

    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(
        out_dir.join("mechanical-pass-results.json"),
        serde_json::to_string_pretty(&json!({
            "decision": "decision_dailymed_ontology.md",
            "cohort": {"rx": n_rx, "otc": n_otc},
            "vocabulary": {
                "products": docs.len(),
                "ingredients": ingredients.len(),
                "routes": routes.len(),
                "forms": forms.len(),
                "labelers": labelers.len(),
            },
            "candidate_rules": candidate_rules,
            "grounding_events": grounding_events,
            "narrative_facts": narrative.len(),
            "oracle_facts": oracle_triples.len(),
            "recall_by_relation": oracle_per_relation,
            "precision_by_relation": per_relation,
            "restraint_inspection": restraint,
        }))?,
    )?;
    println!(
        "saved       {}",
        out_dir.join("mechanical-pass-results.json").display()
    );
    Ok(())
}
