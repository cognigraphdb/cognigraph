//! DailyMed pilot pass-2 — clinical relations (decision_dailymed_clinical.md).
//! Clinical relations (`TREATS`, `CONTRAINDICATED_IN`) have NO structured
//! oracle, so this builds only the label-independent parts:
//!
//! 1. A mechanism-level RESTRAINT PROBE SUITE — author-constructed clinical
//!    sentences with known should/should-not-ground answers (unit tests for
//!    clinical negation, modality, and differential co-mention; not clinical
//!    fact labelling). This is the load-bearing "no labels" measurement.
//! 2. Real-corpus construction with a fixed public condition vocabulary:
//!    grounding volume and the structural no-leak guarantee (subject
//!    scoping). Recall is NOT scored — it waits on a domain-expert reference.
//!
//! Deterministic, offline, no LLM, no embeddings.
//!
//! Run: cargo run --release -p cognigraph-construct --example dailymed_clinical -- [--cohort 200]

use std::path::PathBuf;

use anyhow::{Context, Result};
use cognigraph_construct::clinical_reference::ClinicalVocabulary;
use cognigraph_construct::ingest::entity_key;
use cognigraph_construct::*;
use serde::Deserialize;
use serde_json::json;

const TREATS: &str = "TREATS";
const CONTRAINDICATED_IN: &str = "CONTRAINDICATED_IN";

fn triggers(relation: &str) -> Vec<&'static str> {
    match relation {
        TREATS => vec![
            "indicated for the treatment of {target}",
            "indicated in the treatment of {target}",
            "indicated for {target}",
        ],
        CONTRAINDICATED_IN => vec![
            "contraindicated in patients with {target}",
            "contraindicated in {target}",
        ],
        _ => vec![],
    }
}

/// The clinical condition vocabulary is CORPUS-DERIVED, not hand-picked.
/// Calibration proved a 30-term generic list cannot express a
/// concept-preserving clinical gold (only 3 of 24 of its candidates were TRUE;
/// ~20 required concepts were never surfaced). The frozen vocabulary is built
/// from the corpus's own indication/contraindication wording — see
/// `clinical_reference::build_vocabulary` and
/// `fixtures/semantic-neurons/dailymed/clinical-vocabulary-v1.json`.
const VOCABULARY_PATH: &str = "fixtures/semantic-neurons/dailymed/clinical-vocabulary-v1.json";

fn load_conditions() -> Result<Vec<String>> {
    let vocabulary = ClinicalVocabulary::load(std::path::Path::new(VOCABULARY_PATH))
        .context("load the frozen corpus-derived vocabulary; run `dailymed_clinical_reference vocabulary` first")?;
    let mut terms: Vec<String> = vocabulary
        .treats
        .iter()
        .chain(vocabulary.contraindicated_in.iter())
        .cloned()
        .collect();
    terms.sort();
    terms.dedup();
    Ok(terms)
}

/// Build a single-rule clinical space; `gate` toggles the sentence gate
/// (require the product surface in the licensing sentence).
fn clinical_space(product: &str, relation: &str, targets: &[String], gate: bool) -> SpaceType {
    let rules: Vec<serde_json::Value> = targets
        .iter()
        .map(|t| {
            json!({
                "source": product, "relation": relation, "target": t,
                "when_any": triggers(relation),
                "require_in_sentence": if gate { json!(["source"]) } else { json!([]) },
            })
        })
        .collect();
    let mut entities = vec![json!({"name": product, "type": "product", "aliases": []})];
    for t in targets {
        entities.push(json!({"name": t, "type": "condition", "aliases": []}));
    }
    serde_json::from_value(json!({
        "id": "clinical", "entities": entities, "relation_rules": rules,
    }))
    .unwrap()
}

fn grounds(sentence: &str, product: &str, relation: &str, target: &str, gate: bool) -> bool {
    let space = clinical_space(product, relation, &[target.to_string()], gate);
    let facts = ground_chunk("probe", sentence, &space, &[]);
    let (sk, tk) = (entity_key(product), entity_key(target));
    facts.iter().any(|g| {
        entity_key(&g.fact.source) == sk
            && g.fact.relation == relation
            && entity_key(&g.fact.target) == tk
    })
}

/// (sentence, product, relation, condition, should_ground, class)
type Probe = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
    &'static str,
);

fn probes() -> Vec<Probe> {
    vec![
        // Affirmative — should ground.
        (
            "Betamethasone cream is indicated for the treatment of psoriasis.",
            "Betamethasone cream",
            TREATS,
            "psoriasis",
            true,
            "affirmative",
        ),
        (
            "Amitriptyline tablets are indicated for depression.",
            "Amitriptyline tablets",
            TREATS,
            "depression",
            true,
            "affirmative",
        ),
        (
            "This product is contraindicated in patients with glaucoma.",
            "This product",
            CONTRAINDICATED_IN,
            "glaucoma",
            true,
            "affirmative",
        ),
        (
            "Timolol solution is contraindicated in bradycardia.",
            "Timolol solution",
            CONTRAINDICATED_IN,
            "bradycardia",
            true,
            "affirmative",
        ),
        // Negation — should NOT ground.
        (
            "This product is not contraindicated in pregnancy.",
            "This product",
            CONTRAINDICATED_IN,
            "pregnancy",
            false,
            "negation",
        ),
        (
            "Sertraline is not indicated for the treatment of insomnia.",
            "Sertraline",
            TREATS,
            "insomnia",
            false,
            "negation",
        ),
        (
            "It has not been shown to be contraindicated in renal impairment.",
            "It",
            CONTRAINDICATED_IN,
            "renal impairment",
            false,
            "negation",
        ),
        // Modality / hypothetical — no firm indication trigger.
        (
            "The drug may be considered in patients with mild hypertension.",
            "The drug",
            TREATS,
            "hypertension",
            false,
            "modality",
        ),
        (
            "Use in arthritis has been reported but is not an approved indication.",
            "Use",
            TREATS,
            "arthritis",
            false,
            "modality",
        ),
        // Differential / co-mention — the relation belongs to another subject;
        // the product is not named in the licensing sentence (gate refuses).
        (
            "Unlike corticosteroids indicated for eczema, this class acts differently.",
            "Fluticasone cream",
            TREATS,
            "eczema",
            false,
            "co-mention",
        ),
        (
            "Agents contraindicated in asthma include nonselective beta-blockers.",
            "Metoprolol tablets",
            CONTRAINDICATED_IN,
            "asthma",
            false,
            "co-mention",
        ),
        // Bare mention — no relation trigger at all.
        (
            "Patients with diabetes should be monitored during therapy.",
            "This product",
            CONTRAINDICATED_IN,
            "diabetes",
            false,
            "bare-mention",
        ),
    ]
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

fn arg(flag: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter()
        .position(|x| x == flag)
        .and_then(|i| a.get(i + 1).cloned())
}

fn main() -> Result<()> {
    // --- 1. Restraint probe suite (the load-bearing no-labels result). ---
    // Every clinical rule carries the sentence gate (the realistic config).
    println!("=== Clinical restraint probe suite (mechanism-level, sentence-gated) ===");
    let (mut recall_ok, mut recall_n) = (0usize, 0usize);
    let (mut restraint_ok, mut restraint_n) = (0usize, 0usize);
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for (sentence, product, relation, target, should, class) in probes() {
        let got = grounds(sentence, product, relation, target, true);
        let correct = got == should;
        if should {
            recall_n += 1;
            if correct {
                recall_ok += 1;
            }
        } else {
            restraint_n += 1;
            if correct {
                restraint_ok += 1;
            }
        }
        println!(
            "  [{:11}] {} expected {}, got {} {}",
            class,
            if correct { "PASS" } else { "FAIL" },
            should,
            got,
            if correct { "" } else { "  <-- MISS" }
        );
        rows.push(
            json!({"class": class, "relation": relation, "target": target,
            "should_ground": should, "grounded": got, "correct": correct}),
        );
    }
    println!(
        "  recall (affirmative grounded): {recall_ok}/{recall_n} | restraint (forbidden refused): {restraint_ok}/{restraint_n}"
    );

    // --- 2. Real-corpus construction: volume + structural no-leak. ---
    let n: usize = arg("--cohort").and_then(|v| v.parse().ok()).unwrap_or(200);
    let corpus = PathBuf::from("data/dailymed-pilot");
    let mut entries: Vec<ManifestEntry> = std::fs::read_to_string(corpus.join("manifest-rx.jsonl"))
        .context("read manifest")?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    entries.sort_by_key(|e| e.selection_rank);
    entries.truncate(n);

    let conditions = load_conditions()?;
    println!(
        "\nvocabulary   {} corpus-derived condition terms (was a 30-term generic list)",
        conditions.len()
    );
    let (mut treats, mut contra, mut leaks) = (0usize, 0usize, 0usize);
    let mut docs_with = 0usize;
    for e in &entries {
        let doc: NormalizedDoc =
            serde_json::from_str(&std::fs::read_to_string(corpus.join(&e.document_path))?)?;
        let product = e
            .title
            .rfind('[')
            .map(|i| e.title[..i].trim())
            .unwrap_or(&e.title)
            .to_string();
        let pk = entity_key(&product);
        let mut here = 0usize;
        for (relation, counter) in [(TREATS, &mut treats), (CONTRAINDICATED_IN, &mut contra)] {
            let space = clinical_space(&product, relation, &conditions, true);
            for g in ground_chunk(&e.set_id, &doc.text, &space, &[]) {
                // Structural restraint: every grounded fact's source is the
                // document's own product (subject scoping).
                if entity_key(&g.fact.source) != pk {
                    leaks += 1;
                }
                *counter += 1;
                here += 1;
            }
        }
        if here > 0 {
            docs_with += 1;
        }
    }
    let _ = &mut treats; // (bindings mutated through the tuple above)
    println!(
        "\n=== Real-corpus clinical construction ({} docs, gated) ===",
        entries.len()
    );
    println!(
        "  grounded: {treats} TREATS + {contra} CONTRAINDICATED_IN across {docs_with} docs; \
         cross-product leaks: {leaks}"
    );
    println!(
        "  NOTE: recall is NOT scored here — clinical recall ground truth is a domain-expert \
         reference (decision_dailymed_clinical.md D2), deferred."
    );

    let restraint_pass = restraint_ok == restraint_n && leaks == 0;
    println!(
        "\nacceptance  mechanism restraint {restraint_ok}/{restraint_n} + zero structural leaks: {}",
        if restraint_pass { "PASS" } else { "FAIL" }
    );

    let out = PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-results.json");
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&json!({
            "decision": "decision_dailymed_clinical.md",
            "probe_suite": {"recall_ok": recall_ok, "recall_n": recall_n,
                "restraint_ok": restraint_ok, "restraint_n": restraint_n, "rows": rows},
            "corpus": {"docs": entries.len(), "treats": treats,
                "contraindicated_in": contra, "docs_with_facts": docs_with,
                "cross_product_leaks": leaks},
            "recall_scored": false,
        }))?,
    )?;
    println!("saved       {}", out.display());
    Ok(())
}
