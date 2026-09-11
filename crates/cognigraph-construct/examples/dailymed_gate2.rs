//! DailyMed pilot, gate 2 (docs/dailymed-pilot.md; decision_dailymed_ontology.md
//! D6): calibrate the governed loop on 1,000 documents AND run the first
//! ground-truth-verified measurement of judge quality in the programme.
//!
//! Two oracle-grounded experiments, both scored against the SPL
//! structured data (active + inactive ingredients, routes):
//!
//! A. Judge as a false-positive filter (the crown jewel). Deterministic
//!    construction grounds facts, some true and some false per the
//!    oracle. Each sampled grounded fact becomes a relation_hint carrying
//!    its grounding trigger; the two-stage judge accepts or rejects; the
//!    decision is scored against oracle truth. Yields the judge confusion
//!    matrix and a confidence-threshold sweep (the reviewer-threshold
//!    calibration).
//! B. Propose -> judge on recall gaps. A bounded sample of oracle facts
//!    the templates missed is run through the real proposal loop
//!    (`propose_neurons_via_backend`) and judged; measures proposer
//!    recall, judge accept rate, and net gap closure.
//!
//! Cost is bounded by the sample caps, never the corpus size. `--dry`
//! does all construction, labelling, and sampling and prints the LLM
//! plan and call budget WITHOUT spending anything — run it first.
//!
//! Run: cargo run --release -p cognigraph-construct --example dailymed_gate2 -- \
//!        [--rx 900] [--otc 100] [--judge-sample 40] [--gap-sample 30] [--dry]

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

const SPACE_ID: &str = "dailymed-gate2";
const MAX_CHUNK_CHARS: usize = 2_800;
const TARGET_CHUNK_CHARS: usize = 2_000;
const MIN_SECTION_CHARS: usize = 40;
const CONTAINS_INGREDIENT: &str = "CONTAINS_INGREDIENT";
const ADMINISTERED_VIA: &str = "ADMINISTERED_VIA";

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

/// Per-document mechanical truth. `ingredients` holds ACTIVE and INACTIVE
/// names both — the clean truth for judging (an excipient is a real
/// ingredient); `active` is the subset used for the recall table so gate 2
/// stays comparable with gate 1.5's active-only numbers.
#[derive(Debug, Default)]
struct OracleDoc {
    ingredients: BTreeSet<String>,
    active: BTreeSet<String>,
    routes: BTreeSet<String>,
}

fn parse_oracle(xml: &str) -> Result<OracleDoc> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut oracle = OracleDoc::default();
    let mut ingredient_class: Option<String> = None;
    let mut ingredient_depth = 0usize;
    let mut in_substance = false;
    let mut capture_ingredient = false;
    let mut depth = 0usize;

    loop {
        match reader.read_event() {
            Err(e) => anyhow::bail!("xml error: {e}"),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                depth += 1;
                match e.name().as_ref() {
                    b"ingredient" => {
                        let class = e
                            .try_get_attribute("classCode")?
                            .map(|a| {
                                a.normalized_value(XmlVersion::default())
                                    .unwrap_or_default()
                                    .to_string()
                            })
                            .unwrap_or_default();
                        // ACTIB/ACTIM = active, IACT = inactive; both are
                        // ingredients for truth.
                        if class.starts_with("ACTI") || class == "IACT" {
                            ingredient_class = Some(class);
                            ingredient_depth = depth;
                        }
                    }
                    b"ingredientSubstance" if ingredient_class.is_some() => in_substance = true,
                    b"name" if in_substance && !capture_ingredient => capture_ingredient = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"routeCode"
                    && let Some(display) = e.try_get_attribute("displayName")?
                {
                    oracle.routes.insert(
                        display
                            .normalized_value(XmlVersion::default())
                            .unwrap_or_default()
                            .to_string(),
                    );
                }
            }
            Ok(Event::Text(t)) => {
                if capture_ingredient {
                    let name = t.decode().unwrap_or_default().trim().to_string();
                    if !name.is_empty() {
                        let active = ingredient_class
                            .as_deref()
                            .is_some_and(|c| c.starts_with("ACTI"));
                        if active {
                            oracle.active.insert(name.clone());
                        }
                        oracle.ingredients.insert(name);
                    }
                    capture_ingredient = false;
                }
            }
            Ok(Event::End(e)) => {
                match e.name().as_ref() {
                    b"ingredient" if ingredient_class.is_some() && depth == ingredient_depth => {
                        ingredient_class = None;
                        in_substance = false;
                    }
                    b"ingredientSubstance" => in_substance = false,
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

fn triggers_for(relation: &str) -> Vec<String> {
    let raw: &[&str] = match relation {
        CONTAINS_INGREDIENT => &["contains {target}", "of {target}", "{target}, usp"],
        ADMINISTERED_VIA => &[
            "for {target} use",
            "{target} use only",
            "{target} administration",
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

fn has_flag(flag: &str) -> bool {
    std::env::args().any(|a| a == flag)
}

/// A grounded fact with its oracle label and the product it belongs to.
struct Labelled {
    source: String,
    relation: String,
    target: String,
    trigger: String,
    truth: bool,
}

/// Deterministic balanced sample: sort by a stable key, then interleave
/// so the head isn't dominated by one product.
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
    let gap_sample: usize = arg("--gap-sample")
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let dry = has_flag("--dry");
    let out_dir = PathBuf::from("fixtures/semantic-neurons/dailymed");

    // 1. Cohort, chunks, oracle.
    let mut cohort = manifest_cohort(&corpus, "manifest-rx.jsonl", n_rx)?;
    cohort.extend(manifest_cohort(&corpus, "manifest-otc.jsonl", n_otc)?);
    let mut taken = HashSet::new();
    let mut docs: Vec<(String, OracleDoc, Vec<Chunk>)> = Vec::new();
    let t = Instant::now();
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
        docs.push((product, oracle, chunks));
    }
    println!(
        "cohort      {} docs ({n_rx} rx + {n_otc} otc), oracle parsed in {} ms",
        docs.len(),
        t.elapsed().as_millis()
    );

    // 2. Typed vocabulary (public; not answers).
    let mut ingredients: BTreeSet<String> = BTreeSet::new();
    let mut routes: BTreeSet<String> = BTreeSet::new();
    for (_, oracle, _) in &docs {
        ingredients.extend(oracle.active.iter().cloned()); // rules target ACTIVE names
        routes.extend(oracle.routes.iter().cloned());
    }
    let mut entities: Vec<serde_json::Value> = Vec::new();
    let entity =
        |name: &str, ty: &str| json!({"name": name, "type": ty, "aliases": Vec::<String>::new()});
    for (product, oracle, _) in &docs {
        // Products are titled with a dosage-form suffix ("… CAPSULE") that
        // rarely appears verbatim in compositional prose; the active
        // ingredient name (public, in the product's own title) is how the
        // label refers to the drug, so it rides as an alias to let
        // evidence retrieval find the product in prose (Experiment B).
        let aliases: Vec<String> = oracle.active.iter().cloned().collect();
        entities.push(json!({"name": product, "type": "product", "aliases": aliases}));
    }
    for name in &ingredients {
        entities.push(entity(name, "ingredient"));
    }
    for name in &routes {
        entities.push(entity(name, "route"));
    }
    let entities: Vec<EntityDef> = serde_json::from_value(json!(entities))?;

    // 3. Subject-scoped deterministic construction (D5). Free.
    let backend = NativeBackend::new();
    let targets: Vec<(&str, Vec<String>)> = vec![
        (CONTAINS_INGREDIENT, ingredients.iter().cloned().collect()),
        (ADMINISTERED_VIA, routes.iter().cloned().collect()),
    ];
    let t = Instant::now();
    for (product, _, chunks) in &docs {
        let mut rules: Vec<serde_json::Value> = Vec::new();
        for (relation, names) in &targets {
            for target in names {
                rules.push(json!({
                    "source": product, "relation": relation, "target": target,
                    "when_any": triggers_for(relation),
                }));
            }
        }
        let config: SpaceType = serde_json::from_value(json!({
            "id": SPACE_ID, "entities": entities, "relation_rules": rules,
        }))?;
        ingest_chunks(&backend, SPACE_ID, &config, chunks, &[]).await?;
    }
    let construct_ms = t.elapsed().as_millis();

    // 4. Label every grounded fact against the FULL-ingredient oracle.
    //    Product key -> its truth sets, for O(1) labelling.
    let mut truth: HashMap<String, (HashSet<String>, HashSet<String>)> = HashMap::new(); // key -> (ingredient keys, route keys)
    for (product, oracle, _) in &docs {
        truth.insert(
            entity_key(product),
            (
                oracle.ingredients.iter().map(|s| entity_key(s)).collect(),
                oracle.routes.iter().map(|s| entity_key(s)).collect(),
            ),
        );
    }
    let facts = backend.list_documents("facts", None, None).await?;
    let mut labelled: Vec<Labelled> = Vec::new();
    let (mut n_true, mut n_false) = (0usize, 0usize);
    for fact in &facts {
        let source = fact["_from"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("entities/")
            .to_string();
        let target = fact["_to"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("entities/")
            .to_string();
        let relation = fact["relation_type"].as_str().unwrap_or("?").to_string();
        let is_true = truth.get(&source).is_some_and(|(ings, rts)| {
            if relation == CONTAINS_INGREDIENT {
                ings.contains(&target)
            } else {
                rts.contains(&target)
            }
        });
        if is_true {
            n_true += 1;
        } else {
            n_false += 1;
        }
        labelled.push(Labelled {
            source,
            relation,
            target,
            trigger: fact["trigger"].as_str().unwrap_or("").to_string(),
            truth: is_true,
        });
    }

    // Recall gaps (Experiment B pool): active-oracle facts NOT grounded.
    let grounded: HashSet<(String, String, String)> = labelled
        .iter()
        .map(|l| (l.source.clone(), l.relation.clone(), l.target.clone()))
        .collect();
    let mut gaps: Vec<(String, String, String)> = Vec::new();
    for (product, oracle, _) in &docs {
        let pk = entity_key(product);
        for ing in &oracle.active {
            let key = (pk.clone(), CONTAINS_INGREDIENT.to_string(), entity_key(ing));
            if !grounded.contains(&key) {
                gaps.push((
                    product.clone(),
                    CONTAINS_INGREDIENT.to_string(),
                    ing.clone(),
                ));
            }
        }
    }

    let total_gaps = gaps.len();
    println!(
        "construct   {} grounded facts in {construct_ms} ms ({} true / {} false vs full oracle); {} active-ingredient recall gaps",
        facts.len(),
        n_true,
        n_false,
        total_gaps
    );

    // Balanced judge sample (Experiment A): up to judge_sample of each class.
    let key = |l: &Labelled| format!("{}|{}|{}", l.source, l.relation, l.target);
    let trues: Vec<Labelled> = labelled
        .iter()
        .filter(|l| l.truth)
        .map(clone_labelled)
        .collect();
    let falses: Vec<Labelled> = labelled
        .iter()
        .filter(|l| !l.truth)
        .map(clone_labelled)
        .collect();
    let sampled_true = sample(trues, judge_sample, key);
    let sampled_false = sample(falses, judge_sample, key);
    let sampled_gaps = sample(gaps, gap_sample, |g| format!("{}|{}|{}", g.0, g.1, g.2));

    let judge_calls_est = 2 * (sampled_true.len() + sampled_false.len());
    let propose_calls_est = 3 * sampled_gaps.len();
    println!(
        "plan        Experiment A: judge {} true + {} false grounded facts (~{judge_calls_est} calls)",
        sampled_true.len(),
        sampled_false.len()
    );
    println!(
        "plan        Experiment B: propose+judge {} recall gaps (~{propose_calls_est} calls)",
        sampled_gaps.len()
    );
    println!(
        "plan        estimated LLM budget: ~{} calls total",
        judge_calls_est + propose_calls_est
    );

    if dry {
        println!("dry         --dry set: no LLM calls made. Re-run without --dry to spend.");
        return Ok(());
    }

    // Product -> its own chunks (subject-scoped evidence view for judging).
    let mut product_chunks: HashMap<String, Vec<Chunk>> = HashMap::new();
    for (product, _, chunks) in &docs {
        product_chunks
            .entry(entity_key(product))
            .or_default()
            .extend(chunks.iter().cloned());
    }

    let provider = completion_from_env()?;
    let run_start = Instant::now();
    let mut llm_calls = 0usize;

    // --- Experiment A: judge as an oracle-scored FP filter. ---
    #[derive(Default)]
    struct Confusion {
        accept_true: usize,
        accept_false: usize,
        reject_true: usize,
        reject_false: usize,
        human_true: usize,
        human_false: usize,
    }
    let mut cm = Confusion::default();
    let mut judge_rows: Vec<serde_json::Value> = Vec::new();
    for l in sampled_true.iter().chain(sampled_false.iter()) {
        let neuron: Neuron = serde_json::from_value(json!({
            "id": format!("gate2-{}-{}-{}", l.source, l.relation, l.target),
            "type": "relation_hint",
            "status": "proposed",
            "source": l.source, "relation": l.relation, "target": l.target,
            "triggers": [l.trigger],
            "rationale": "Candidate relation grounded from product label prose. \
                          Judge whether the matched evidence states this relation for this product.",
            "evidence": [l.trigger.clone()],
        }))?;
        let chunks = product_chunks.get(&l.source).cloned().unwrap_or_default();
        let verdict = judge_neuron(provider.as_ref(), &neuron, &chunks).await?;
        llm_calls += if verdict.verdict == "needs_human" && verdict.raw.get("screen").is_some() {
            1
        } else {
            2
        };
        // The judge_schema verdict enum is accept | reject | needs_human.
        match (verdict.verdict.as_str(), l.truth) {
            ("accept", true) => cm.accept_true += 1,
            ("accept", false) => cm.accept_false += 1,
            ("reject", true) => cm.reject_true += 1,
            ("reject", false) => cm.reject_false += 1,
            (_, true) => cm.human_true += 1,
            (_, false) => cm.human_false += 1,
        }
        judge_rows.push(json!({
            "fact": format!("{} --{}--> {}", l.source, l.relation, l.target),
            "trigger": l.trigger,
            "oracle_true": l.truth,
            "verdict": verdict.verdict,
            "confidence": verdict.confidence,
            "reasoning": verdict.reasoning,
        }));
    }
    let judged = sampled_true.len() + sampled_false.len();
    let correct = cm.accept_true + cm.reject_false;
    let filter_precision = if cm.accept_true + cm.accept_false > 0 {
        100.0 * cm.accept_true as f64 / (cm.accept_true + cm.accept_false) as f64
    } else {
        0.0
    };
    let filter_recall = if cm.accept_true + cm.reject_true + cm.human_true > 0 {
        100.0 * cm.accept_true as f64 / (cm.accept_true + cm.reject_true + cm.human_true) as f64
    } else {
        0.0
    };
    println!("\n=== Experiment A: judge as false-positive filter (vs full oracle) ===");
    println!(
        "  judged {judged}; agreement with oracle {correct}/{judged} ({:.0}%)",
        100.0 * correct as f64 / judged.max(1) as f64
    );
    println!(
        "  accept: {} true / {} false   reject: {} true / {} false   needs_human: {} true / {} false",
        cm.accept_true,
        cm.accept_false,
        cm.reject_true,
        cm.reject_false,
        cm.human_true,
        cm.human_false
    );
    println!(
        "  as a filter: precision {filter_precision:.0}% (accepted facts that are true), \
         recall {filter_recall:.0}% (true facts kept)"
    );

    // --- Experiment B: propose -> judge on recall gaps. ---
    let missing: Vec<Fact> = sampled_gaps
        .iter()
        .map(|(s, r, t)| Fact {
            source: s.clone(),
            relation: r.clone(),
            target: t.clone(),
        })
        .collect();
    // The proposal validator learns the legal relation vocabulary from
    // the space's relation_rules; pass one declaring rule per relation so
    // proposed hints validate (without it, every proposal is rejected as
    // an "unknown relation").
    let propose_space: SpaceType = serde_json::from_value(json!({
        "id": SPACE_ID,
        "entities": entities,
        "relation_rules": [
            {"source": "", "relation": CONTAINS_INGREDIENT, "target": "", "when_any": []},
            {"source": "", "relation": ADMINISTERED_VIA, "target": "", "when_any": []},
        ],
    }))?;
    let proposal = propose_neurons_via_backend(
        provider.as_ref(),
        &propose_space,
        &missing,
        &backend,
        SPACE_ID,
    )
    .await?;
    // Proposer skips on empty evidence happen BEFORE any LLM call ($0);
    // only proposals that reached the model cost a call. Count precisely.
    let proposed = proposal.set.neurons.len();
    llm_calls += proposed;
    let mut gap_accept = 0usize;
    let mut gap_rows: Vec<serde_json::Value> = Vec::new();
    for neuron in &proposal.set.neurons {
        let chunks = product_chunks
            .get(&entity_key(&neuron.source))
            .cloned()
            .unwrap_or_default();
        let verdict = judge_neuron(provider.as_ref(), neuron, &chunks).await?;
        llm_calls += 2;
        if verdict.verdict == "accept" {
            gap_accept += 1;
        }
        gap_rows.push(json!({
            "fact": format!("{} --{}--> {}", neuron.source, neuron.relation, neuron.target),
            "triggers": neuron.triggers,
            "verdict": verdict.verdict,
            "confidence": verdict.confidence,
        }));
    }
    // Skip reasons are the diagnostic for a low proposal rate.
    let mut skip_reasons: BTreeMap<String, usize> = BTreeMap::new();
    for skip in &proposal.skipped {
        *skip_reasons.entry(skip.reason.clone()).or_default() += 1;
    }
    println!("\n=== Experiment B: propose -> judge on recall gaps ===");
    println!(
        "  {} gaps sampled -> {proposed} proposed ({} skipped) -> {gap_accept} judge-accepted",
        sampled_gaps.len(),
        proposal.skipped.len()
    );
    for (reason, count) in &skip_reasons {
        println!("  skip x{count}  {reason}");
    }
    println!(
        "  net recall closure on the sample: {gap_accept}/{} ({:.0}%)",
        sampled_gaps.len(),
        100.0 * gap_accept as f64 / sampled_gaps.len().max(1) as f64
    );

    println!(
        "\ncalibration LLM calls {llm_calls}, wall-clock {:.1}s ({:.2}s/judgment)",
        run_start.elapsed().as_secs_f64(),
        run_start.elapsed().as_secs_f64() / (judged + proposed).max(1) as f64
    );

    std::fs::create_dir_all(&out_dir)?;
    std::fs::write(
        out_dir.join("gate2-results.json"),
        serde_json::to_string_pretty(&json!({
            "cohort": {"rx": n_rx, "otc": n_otc, "products": docs.len()},
            "construction": {
                "grounded_facts": facts.len(), "true": n_true, "false": n_false,
                "recall_gaps": total_gaps, "construct_ms": construct_ms,
            },
            "experiment_a_judge_filter": {
                "judged": judged,
                "accept_true": cm.accept_true, "accept_false": cm.accept_false,
                "reject_true": cm.reject_true, "reject_false": cm.reject_false,
                "needs_human_true": cm.human_true, "needs_human_false": cm.human_false,
                "oracle_agreement": correct,
                "filter_precision_pct": filter_precision,
                "filter_recall_pct": filter_recall,
                "rows": judge_rows,
            },
            "experiment_b_gap_closure": {
                "gaps_sampled": sampled_gaps.len(), "proposed": proposed,
                "proposer_skipped": proposal.skipped.len(), "judge_accepted": gap_accept,
                "skip_reasons": skip_reasons,
                "rows": gap_rows,
            },
            "calibration": {"llm_calls": llm_calls},
        }))?,
    )?;
    println!(
        "saved       {}",
        out_dir.join("gate2-results.json").display()
    );
    Ok(())
}

fn clone_labelled(l: &Labelled) -> Labelled {
    Labelled {
        source: l.source.clone(),
        relation: l.relation.clone(),
        target: l.target.clone(),
        trigger: l.trigger.clone(),
        truth: l.truth,
    }
}
