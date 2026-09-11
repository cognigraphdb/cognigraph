//! Blinded domain-expert reference workflow for DailyMed clinical relations.
//!
//! Run with `--help` for commands. Gold labels are always human-authored;
//! this tool prepares, validates, compares, compiles, and scores them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use cognigraph_construct::clinical_reference::*;
use cognigraph_construct::judge_neuron;
use cognigraph_embeddings::completion::completion_from_env;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let Some(command) = args.get(1).map(String::as_str) else {
        print_help();
        return Ok(());
    };
    if command == "--help" || command == "help" {
        print_help();
        return Ok(());
    }
    let options = Options::parse(&args[2..])?;
    match command {
        "prepare" => prepare(&options),
        "handoff" => handoff(&options),
        "validate" => validate(&options),
        "compare" => compare(&options),
        "compile" => compile(&options),
        "vocabulary" => vocabulary(&options),
        "lexicon" => lexicon(&options),
        "matcher" => matcher(&options),
        "losses" => losses(&options),
        "score" => score(&options),
        "judge" => judge(&options).await,
        other => bail!("unknown command {other}; run with --help"),
    }
}

fn prepare(options: &Options) -> Result<()> {
    let corpus = options.path("--corpus", "data/dailymed-pilot");
    let output = options.path("--output", "data/dailymed-clinical-reference");
    let documents = options.usize("--documents", 50)?;
    let calibration = options.usize("--calibration", 10)?;
    let seed = options.value("--seed", "cognigraph-clinical-reference-v1");
    let packets = prepare_reference(&corpus, &output, documents, calibration, seed)?;
    let candidate_count: usize = packets.iter().map(|packet| packet.candidates.len()).sum();
    // State the two counts distinctly. They are easy to conflate: each candidate
    // is copied to BOTH reviewer files, so the number of annotation ROWS is twice
    // the number of CANDIDATES. Reporting only the row count would double-count
    // the reviewer copies and overstate the size of the reference.
    const REVIEWERS: usize = 2;
    println!(
        "prepared {} blinded packets ({} calibration, {} evaluation)",
        packets.len(),
        calibration,
        packets.len() - calibration,
    );
    println!(
        "candidates   {candidate_count} unique mechanical candidates -> \
         {} unreviewed annotation rows ({REVIEWERS} independent reviewer copies each)",
        candidate_count * REVIEWERS
    );
    println!("workspace    {}", output.display());
    println!("next         label annotations/expert-a.jsonl and expert-b.jsonl independently");
    Ok(())
}

/// Generate the two blinded reviewer worksheets and a coordinator README.
///
/// This is the HUMAN hand-off. It surfaces every candidate with its exact
/// section evidence and the frozen R1–R7 rules, and lets a clinician record
/// verdicts and add missed facts; the worksheet exports a validator-compatible
/// `<annotator>.jsonl`. It writes NO verdicts — an LLM labelling these would be
/// circular and, for safety-sensitive clinical claims, inappropriate (D2).
fn handoff(options: &Options) -> Result<()> {
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let out = options.path("--output", root.join("handoff"));
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))
        .context("read packets.jsonl — run `prepare` first")?;
    assert_workspace_fresh(&packets)?;
    fs::create_dir_all(&out)?;

    // Only the fields the worksheet needs; the reviewer never sees tool output.
    let payload_packets: Vec<serde_json::Value> = packets
        .iter()
        .map(|p| {
            serde_json::json!({
                "set_id": p.set_id,
                "product": p.product,
                "split": match p.split { ReviewSplit::Calibration => "calibration", _ => "evaluation" },
                "sections": p.sections.iter().map(|s| serde_json::json!({
                    "code": s.code, "title": s.title, "text": s.text })).collect::<Vec<_>>(),
                "candidates": p.candidates.iter().map(|c| serde_json::json!({
                    "relation": c.relation, "condition": c.condition,
                    "section_code": c.section_code, "evidence": c.evidence })).collect::<Vec<_>>(),
            })
        })
        .collect();

    let template = include_str!("worksheet.html");
    let (cal, eval) = packets.iter().fold((0, 0), |(c, e), p| match p.split {
        ReviewSplit::Calibration => (c + 1, e),
        _ => (c, e + 1),
    });

    for annotator in ["expert-a", "expert-b"] {
        let payload = serde_json::to_string(&serde_json::json!({
            "schema_version": "dailymed-clinical-reference-v1",
            "annotator": annotator,
            "packets": payload_packets,
        }))?;
        // The payload is embedded in a <script type="application/json"> block;
        // only `</` needs escaping to avoid closing the tag early.
        let html = template.replace("__PAYLOAD__", &payload.replace("</", "<\\/"));
        fs::write(out.join(format!("{annotator}.html")), html)?;
    }
    fs::write(out.join("REVIEWERS.md"), reviewers_readme(cal, eval))?;

    println!("handoff      {cal} calibration + {eval} held-out documents");
    println!("wrote        {}/expert-a.html", out.display());
    println!("             {}/expert-b.html", out.display());
    println!("             {}/REVIEWERS.md", out.display());
    println!(
        "next         two clinicians open their worksheet, label independently, download their\n\
         \x20            .jsonl; then `compare` -> adjudicate -> `score`. No verdicts are pre-filled."
    );
    Ok(())
}

fn reviewers_readme(cal: usize, eval: usize) -> String {
    format!(
        "# Clinical reference — reviewer instructions\n\n\
        Two clinically qualified reviewers label **independently and blind to each other**\n\
        and to any tool output. This produces the held-out gold standard; nothing is\n\
        machine-labelled.\n\n\
        ## Do this\n\n\
        1. Reviewer A opens `expert-a.html`; reviewer B opens `expert-b.html` (any modern\n\
           browser, offline). Work autosaves to that browser.\n\
        2. For every candidate, mark **true / false / uncertain** under rules R1–R7 (shown\n\
           at the top of the worksheet, frozen at calibration).\n\
        3. **Read each full section** and **add any real fact the candidates missed** — the\n\
           reference must be exhaustive for TREATS and CONTRAINDICATED_IN. Added-fact\n\
           evidence must be copied exactly from the section (the worksheet checks this).\n\
        4. When all {total} documents are complete, click **Download my labels** and send\n\
           the `.jsonl` to the coordinator.\n\n\
        ## Scope\n\n\
        - **{eval} held-out documents** are the scoring set.\n\
        - **{cal} calibration documents** are included too (rules were frozen from these);\n\
          labelling them independently also measures inter-rater agreement. They are\n\
          **excluded from the reported recall score** by the toolkit.\n\n\
        ## Coordinator, once both files are in (`data/dailymed-clinical-reference/`)\n\n\
        ```\n\
        cargo run --release -p cognigraph-construct --example dailymed_clinical_reference -- \\\n\
          compare --a expert-a.jsonl --b expert-b.jsonl      # writes adjudication.jsonl\n\
        # resolve every ADJUDICATE row, mark each record complete, then:\n\
        cargo run --release -p cognigraph-construct --example dailymed_clinical_reference -- score\n\
        ```\n\n\
        Until both files exist and adjudication is complete, `score` and `judge` refuse to\n\
        run. That refusal is deliberate: there is no clinical recall number until this\n\
        human reference is done.\n",
        total = cal + eval
    )
}

fn validate(options: &Options) -> Result<()> {
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))?;
    let annotations = required_path(options, "--annotations")?;
    let records: Vec<AnnotationRecord> = read_jsonl(&annotations)?;
    validate_annotations(&packets, &records)?;
    println!(
        "valid        {} complete documents in {}",
        records.len(),
        annotations.display()
    );
    Ok(())
}

fn compare(options: &Options) -> Result<()> {
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))?;
    let a_path = options.path("--a", root.join("annotations/expert-a.jsonl"));
    let b_path = options.path("--b", root.join("annotations/expert-b.jsonl"));
    let output = options.path("--output", root.join("adjudication.jsonl"));
    let a: Vec<AnnotationRecord> = read_jsonl(&a_path)?;
    let b: Vec<AnnotationRecord> = read_jsonl(&b_path)?;
    let (agreements, disagreements) = compare_annotations(&packets, &a, &b, &output)?;
    println!("agreements   {agreements}");
    println!("disagreements {disagreements}");
    println!("adjudication {}", output.display());
    if disagreements > 0 {
        println!("next         resolve ADJUDICATE rows and mark each record complete");
    }
    Ok(())
}

fn compile(options: &Options) -> Result<()> {
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))?;
    let annotations = options.path("--annotations", root.join("adjudication.jsonl"));
    let records: Vec<AnnotationRecord> = read_jsonl(&annotations)?;
    validate_annotations(&packets, &records)?;
    let (packets, records) =
        evaluation_subset(packets, records, options.has("--include-calibration"));
    let spec = compile_eval_spec(&packets, &records)?;
    let output = options.path("--output", root.join("clinical-eval.json"));
    fs::write(&output, serde_json::to_string_pretty(&spec)?)?;
    println!(
        "compiled     {} evaluation questions to {}",
        spec.questions.len(),
        output.display()
    );
    Ok(())
}

/// Build and freeze the corpus-derived clinical vocabulary. Derived from label
/// TEXT only (never gold labels), with the reference documents EXCLUDED so the
/// held-out evaluation is not scored on concepts lifted from its own documents.
fn vocabulary(options: &Options) -> Result<()> {
    let corpus = options.path("--corpus", "data/dailymed-pilot");
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let output = options.path(
        "--output",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-vocabulary-v1.json"),
    );
    let min_df = options.usize("--min-df", DEFAULT_MIN_DOCUMENT_FREQUENCY)?;

    // Exclude every reference document, calibration and held-out alike.
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))
        .context("read packets.jsonl — run `prepare` first so the exclusion list exists")?;
    let excluded: std::collections::BTreeSet<String> =
        packets.iter().map(|p| p.set_id.clone()).collect();

    let drugs = DrugLexicon::load(std::path::Path::new(
        "fixtures/semantic-neurons/dailymed/clinical-drug-lexicon-v1.json",
    ))
    .context("load the drug lexicon; run `lexicon` first")?
    .set();
    let built = build_vocabulary(&corpus, &excluded, min_df, &drugs)?;
    built.save(&output)?;
    // Two DIFFERENT counts, easily conflated: the vocabulary is relation-specific
    // (a term may be legitimately valid for both relations), so the number of
    // ENTRIES exceeds the number of unique condition STRINGS.
    let unique: std::collections::BTreeSet<&String> = built
        .treats
        .iter()
        .chain(built.contraindicated_in.iter())
        .collect();
    println!(
        "vocabulary   {} relation-specific entries ({} TREATS + {} CONTRAINDICATED_IN) = {} \
         unique condition strings, {} of which are valid for BOTH relations",
        built.len(),
        built.treats.len(),
        built.contraindicated_in.len(),
        unique.len(),
        built.len() - unique.len(),
    );
    println!(
        "sources      {} documents; {} reference documents excluded; min document frequency {}",
        built.source_documents, built.excluded_documents, built.min_document_frequency
    );
    println!("digest       {}", built.digest);
    println!("frozen       {}", output.display());
    Ok(())
}

/// PHASE 2: build the drug lexicon from the corpus's STRUCTURED ingredient data
/// (the UNII-anchored block gates 1-3 trusted as ground truth). A bare-list term
/// that is a drug is then refused as a condition because it IS a drug, not
/// because it was deny-listed.
fn lexicon(options: &Options) -> Result<()> {
    let corpus = options.path("--corpus", "data/dailymed-pilot");
    let output = options.path(
        "--output",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-drug-lexicon-v1.json"),
    );
    let built = build_drug_lexicon(&corpus)?;
    built.save(&output)?;
    println!(
        "drug lexicon {} active substances from {} labels (structured UNII data)",
        built.substances.len(),
        built.source_documents
    );
    println!("digest       {}", built.digest);
    println!("frozen       {}", output.display());
    Ok(())
}
/// PHASE 1 VALIDATION + FREEZE. Validate against the accepted development-showcase
/// calibration and every synthetic R1-R7 negative/broad probe.
fn matcher(options: &Options) -> Result<()> {
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))?;
    assert_workspace_fresh(&packets)?;
    let corpus = ClinicalVocabulary::load(&options.path(
        "--vocabulary",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-vocabulary-v1.json"),
    ))?;
    let treats: BTreeSet<String> = corpus.treats.iter().cloned().collect();
    let contra: BTreeSet<String> = corpus.contraindicated_in.iter().cloned().collect();
    let drugs = DrugLexicon::load(std::path::Path::new(
        "fixtures/semantic-neurons/dailymed/clinical-drug-lexicon-v1.json",
    ))
    .context("load the drug lexicon; run `lexicon` first")?
    .set();
    let calibration_path = options.path(
        "--calibration",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-calibration-v1.json"),
    );
    let accepted = load_calibration(&calibration_path, &packets)?;

    // --- (a) ACCEPTED CALIBRATION. Held-out annotations stay sealed. ---
    let calibration_packets: Vec<&ReviewPacket> = packets
        .iter()
        .filter(|p| p.split == ReviewSplit::Calibration)
        .collect();
    // Semantic validation offers every accepted concept so a refusal cannot be
    // explained by a missing vocabulary term. End-to-end coverage is measured
    // separately with the genuinely frozen corpus vocabulary.
    let mut semantic_treats = treats.clone();
    let mut semantic_contra = contra.clone();
    for case in &accepted.cases {
        if case.relation == TREATS {
            semantic_treats.insert(case.concept.clone());
        } else {
            semantic_contra.insert(case.concept.clone());
        }
    }
    let mut asserted_semantic: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut asserted_end_to_end: BTreeSet<(String, String, String)> = BTreeSet::new();
    for packet in &calibration_packets {
        for assertion in match_packet(packet, &semantic_treats, &semantic_contra, &drugs) {
            asserted_semantic.insert((
                packet.set_id.clone(),
                assertion.relation,
                assertion.concept,
            ));
        }
        for assertion in match_packet(packet, &treats, &contra, &drugs) {
            asserted_end_to_end.insert((
                packet.set_id.clone(),
                assertion.relation,
                assertion.concept,
            ));
        }
    }
    let true_cases: Vec<_> = accepted
        .cases
        .iter()
        .filter(|case| case.verdict == ClinicalVerdict::True)
        .collect();
    let false_cases: Vec<_> = accepted
        .cases
        .iter()
        .filter(|case| case.verdict == ClinicalVerdict::False)
        .collect();
    let (mut in_vocab, mut semantic_hit, mut end_to_end_hit) = (0usize, 0usize, 0usize);
    let mut missed = Vec::new();
    for case in &true_cases {
        let vocabulary = if case.relation == TREATS {
            &treats
        } else {
            &contra
        };
        if vocabulary.contains(&case.concept) {
            in_vocab += 1;
        }
        let key = (
            case.set_id.clone(),
            case.relation.clone(),
            case.concept.clone(),
        );
        if asserted_semantic.contains(&key) {
            semantic_hit += 1;
        } else {
            missed.push(format!("{} {} [{}]", case.relation, case.concept, case.id));
        }
        if asserted_end_to_end.contains(&key) {
            end_to_end_hit += 1;
        }
    }
    let calibration_leaks: Vec<_> = false_cases
        .iter()
        .filter_map(|case| {
            let key = (
                case.set_id.clone(),
                case.relation.clone(),
                case.concept.clone(),
            );
            asserted_semantic
                .contains(&key)
                .then(|| format!("{} {} [{}]", case.relation, case.concept, case.id))
        })
        .collect();
    println!(
        "MATCHER {MATCHER_VERSION} — validation against the accepted calibration (calibration split only)\n"
    );
    println!(
        "  accepted artifact              {}",
        calibration_path.display()
    );
    println!("  cases digest                   {}", accepted.cases_digest);
    println!(
        "  accepted TRUE cases asserted    {semantic_hit}/{} (all concepts offered)",
        true_cases.len()
    );
    println!(
        "  accepted FALSE cases refused    {}/{} (all concepts offered)",
        false_cases.len() - calibration_leaks.len(),
        false_cases.len()
    );
    println!(
        "  corpus vocabulary coverage      {in_vocab}/{}",
        true_cases.len()
    );
    println!(
        "  end-to-end accepted TRUE asserted {end_to_end_hit}/{}",
        true_cases.len()
    );
    if !missed.is_empty() {
        println!("  missed:");
        for m in &missed {
            println!("    - {m}");
        }
    }
    for leak in &calibration_leaks {
        println!("    SIGNED FALSE LEAK  {leak}");
    }

    // --- (b) R1-R7 negative / broad probes. Every one must be REFUSED. ---
    let probes: &[(&str, &str, &str, &str)] = &[
        // (rule, section code, text, concept that must NOT be asserted)
        (
            "R1 broad",
            "34067-9",
            "Indicated for the treatment of partial-onset seizures.",
            "seizures",
        ),
        (
            "R1 broad",
            "34070-3",
            "Contraindicated in systemic fungal infections.",
            "infection",
        ),
        (
            "R1 broad",
            "34067-9",
            "Indicated for the management of neuropathic pain associated with diabetic peripheral neuropathy.",
            "pain",
        ),
        (
            "negation",
            "34070-3",
            "This product is not contraindicated in pregnancy.",
            "pregnancy",
        ),
        (
            "negation",
            "34067-9",
            "Sertraline is not indicated for the treatment of insomnia.",
            "insomnia",
        ),
        (
            "negation",
            "34070-3",
            "It has not been shown to be contraindicated in renal impairment.",
            "renal impairment",
        ),
        (
            "modality",
            "34067-9",
            "The drug may be considered in patients with mild hypertension.",
            "hypertension",
        ),
        (
            "modality",
            "34067-9",
            "Use in arthritis has been reported but is not an approved indication.",
            "arthritis",
        ),
        (
            "co-mention",
            "34067-9",
            "Unlike corticosteroids indicated for eczema, this class acts differently.",
            "eczema",
        ),
        (
            "co-mention",
            "34070-3",
            "Agents contraindicated in asthma include nonselective beta-blockers.",
            "asthma",
        ),
        (
            "bare mention",
            "34070-3",
            "Patients with diabetes should be monitored during therapy.",
            "diabetes",
        ),
        (
            "R4 prose/type noise",
            "34070-3",
            "Severe, rarely fatal, anaphylactic-like reactions to NSAIDs have been reported in such patients.",
            "severe",
        ),
        (
            "R4 prose/type noise",
            "34070-3",
            "Severe, rarely fatal, anaphylactic-like reactions to NSAIDs have been reported in such patients.",
            "doxazosin",
        ),
        (
            "R5 combination",
            "34070-3",
            "Do not co-administer aliskiren with olmesartan medoxomil in patients with diabetes.",
            "diabetes",
        ),
    ];
    let mut refused = 0usize;
    let mut probe_leaks = Vec::new();
    for (rule, code, text, forbidden) in probes {
        let section = ClinicalSection {
            code: code.to_string(),
            title: String::new(),
            text: text.to_string(),
        };
        // The forbidden concept is offered IN the vocabulary — the refusal must
        // come from the matcher's semantics, not from a missing term.
        let vocabulary: BTreeSet<String> = std::iter::once(forbidden.to_string()).collect();
        let got = match_section(&section, &vocabulary, &drugs);
        if got.is_empty() {
            refused += 1;
        } else {
            probe_leaks.push(format!("{rule}: asserted {forbidden:?} from {text:?}"));
        }
    }
    println!(
        "\n  R1-R7 negative/broad probes refused  {refused}/{}",
        probes.len()
    );
    for l in &probe_leaks {
        println!("    LEAK  {l}");
    }

    // --- (c) FREEZE. ---
    let frozen = serde_json::json!({
        "matcher_version": MATCHER_VERSION,
        "frozen_at": "after project-owner development-showcase calibration acceptance; before held-out expert labels",
        "vocabulary_digest": corpus.digest,
        "calibration_artifact": calibration_path,
        "calibration_cases_digest": accepted.cases_digest,
        "calibration_acceptance_limitation": accepted.limitation,
        "calibration_true_asserted_semantic": semantic_hit,
        "calibration_true_asserted_end_to_end": end_to_end_hit,
        "calibration_true_total": true_cases.len(),
        "calibration_true_in_corpus_vocabulary": in_vocab,
        "calibration_true_missed": missed,
        "calibration_false_refused": false_cases.len() - calibration_leaks.len(),
        "calibration_false_total": false_cases.len(),
        "calibration_false_leaks": calibration_leaks,
        "negative_probes_refused": refused,
        "negative_probes_total": probes.len(),
        "probe_leaks": probe_leaks,
        "caveat": "Validation against the project-owner-accepted calibration. NOT a recall result. No number here may be described as a \
                   recall improvement until the held-out expert reference is complete.",
    });
    let output = options.path(
        "--output",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-matcher-v1.json"),
    );
    fs::write(&output, serde_json::to_string_pretty(&frozen)?)?;
    println!("\nfrozen       {}", output.display());
    if !calibration_leaks.is_empty() || !probe_leaks.is_empty() {
        bail!(
            "matcher leaked on {} accepted or synthetic negative case(s); refusing to freeze",
            calibration_leaks.len() + probe_leaks.len()
        );
    }
    Ok(())
}

/// The three losses, reported SEPARATELY (constraint 6), at fact-instance
/// granularity with relation-specific vocabularies (constraint 1). Requires the
/// frozen matcher.
fn losses(options: &Options) -> Result<()> {
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))?;
    assert_workspace_fresh(&packets)?;
    let corpus = ClinicalVocabulary::load(&options.path(
        "--vocabulary",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-vocabulary-v1.json"),
    ))?;
    let frozen = PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-matcher-v1.json");
    if !frozen.exists() {
        bail!("the matcher must be validated and frozen first: run `matcher`");
    }
    let treats: BTreeSet<String> = corpus.treats.iter().cloned().collect();
    let contra: BTreeSet<String> = corpus.contraindicated_in.iter().cloned().collect();
    let drugs = DrugLexicon::load(std::path::Path::new(
        "fixtures/semantic-neurons/dailymed/clinical-drug-lexicon-v1.json",
    ))
    .context("load the drug lexicon; run `lexicon` first")?
    .set();

    let held: Vec<&ReviewPacket> = packets
        .iter()
        .filter(|p| p.split == ReviewSplit::Evaluation)
        .collect();
    // Fact INSTANCES: (set_id, relation, concept) — never deduplicated globally.
    let mut candidates: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut asserted: BTreeSet<(String, String, String)> = BTreeSet::new();
    for packet in &held {
        for c in &packet.candidates {
            candidates.insert((
                packet.set_id.clone(),
                c.relation.clone(),
                c.condition.clone(),
            ));
        }
        for a in match_packet(packet, &treats, &contra, &drugs) {
            asserted.insert((packet.set_id.clone(), a.relation, a.concept));
        }
    }
    // Re-derive the calibration validation numbers from the frozen artifact.
    let frozen_json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&frozen)?)?;
    // Fail loudly on a missing key. The previous `unwrap_or(0)` silently wrote
    // ZEROS into the published fixture when the key names drifted — exactly the
    // class of bug that ships a wrong number without anyone noticing.
    let as_usize = |k: &str| -> Result<usize> {
        frozen_json
            .get(k)
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .with_context(|| format!("frozen matcher artifact is missing the key `{k}`"))
    };
    let report = compute_losses(
        &candidates,
        &asserted,
        &corpus,
        &drugs,
        as_usize("calibration_true_total")?,
        as_usize("calibration_true_in_corpus_vocabulary")?,
        as_usize("calibration_true_asserted_end_to_end")?,
        frozen_json["calibration_true_missed"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    );
    println!("THREE LOSSES — fact instances (set_id, relation, concept), NOT recall\n");
    println!(
        "  candidate instances (extractor; NOT gold)   {}",
        report.candidate_instances
    );
    println!(
        "  1. vocabulary loss                          {}",
        report.vocabulary_loss
    );
    println!(
        "  2. trigger/structure loss                   {}",
        report.trigger_structure_loss
    );
    println!(
        "  3. extractor/type noise                     {}",
        report.type_noise_instances
    );
    println!(
        "  0. asserted candidates                      {}",
        report.asserted_candidates
    );
    println!(
        "     partition check: {} + {} + {} + {} = {}",
        report.asserted_candidates,
        report.vocabulary_loss,
        report.trigger_structure_loss,
        report.type_noise_instances,
        report.candidate_instances
    );
    println!(
        "\n  {} asserts {} facts IN TOTAL — {} of them found outside the candidate\n  list, because the matcher and the reference extractor do not share a parser.\n  That total is NOT a partition member and must never be summed with the losses.",
        MATCHER_VERSION, report.asserted_by_matcher, report.asserted_outside_candidates
    );
    println!("\n  {}", report.caveat);
    let output = options.path(
        "--output",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-losses.json"),
    );
    fs::write(&output, serde_json::to_string_pretty(&report)?)?;
    println!("\nsaved        {}", output.display());
    Ok(())
}

fn score(options: &Options) -> Result<()> {
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))?;
    let annotations = options.path("--annotations", root.join("adjudication.jsonl"));
    let records: Vec<AnnotationRecord> = read_jsonl(&annotations)?;
    validate_annotations(&packets, &records)?;
    let (packets, records) =
        evaluation_subset(packets, records, options.has("--include-calibration"));

    // The corpus-derived vocabulary is optional: without it the improved-system
    // lane is skipped rather than silently falling back to the gold concepts.
    let vocabulary_path = options.path(
        "--vocabulary",
        PathBuf::from("fixtures/semantic-neurons/dailymed/clinical-vocabulary-v1.json"),
    );
    let corpus_vocabulary = ClinicalVocabulary::load(&vocabulary_path).ok();

    // Three lanes, reported separately and never conflated. The FIRST is the
    // headline end-to-end result; the LAST is a diagnostic that hands the
    // grounder the gold concepts and must never be read as system performance.
    let mut lanes = vec![(VocabularyLane::FrozenGeneric, None)];
    if let Some(vocabulary) = corpus_vocabulary.as_ref() {
        lanes.push((VocabularyLane::CorpusDerived, Some(vocabulary)));
    } else {
        println!(
            "note         corpus-derived lane SKIPPED (no frozen vocabulary at {}); \
             run `vocabulary` first",
            vocabulary_path.display()
        );
    }
    lanes.push((VocabularyLane::OracleDiagnostic, None));

    let mut all = Vec::new();
    for (lane, vocabulary) in lanes {
        let result = score_grounding(&packets, &records, lane, vocabulary)?;
        println!("\nlane         {}", result.lane);
        println!(
            "grounding    TP={} FN={} TN={} FP={} uncertain_excluded={}",
            result.true_positive,
            result.false_negative,
            result.true_negative,
            result.false_positive,
            result.uncertain_excluded
        );
        println!(
            "metrics      recall={} restraint={} precision={}",
            metric(result.recall),
            metric(result.restraint),
            metric(result.precision)
        );
        if !result.end_to_end {
            println!("             ^ DIAGNOSTIC ONLY — concept discovery removed from the problem");
        }
        all.push(result);
    }
    let output = options.path("--output", root.join("grounding-score.json"));
    fs::write(&output, serde_json::to_string_pretty(&all)?)?;
    println!("\nsaved        {}", output.display());
    Ok(())
}

async fn judge(options: &Options) -> Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let root = options.path("--root", "data/dailymed-clinical-reference");
    let packets: Vec<ReviewPacket> = read_jsonl(&root.join("packets.jsonl"))?;
    let annotations = options.path("--annotations", root.join("adjudication.jsonl"));
    let records: Vec<AnnotationRecord> = read_jsonl(&annotations)?;
    validate_annotations(&packets, &records)?;
    let (packets, records) =
        evaluation_subset(packets, records, options.has("--include-calibration"));
    let record_map: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.set_id.as_str(), record))
        .collect();
    let limit = options.optional_usize("--limit")?;
    let provider = completion_from_env()?;
    let mut results = Vec::new();
    'documents: for packet in &packets {
        for label in &record_map[&packet.set_id.as_str()].facts {
            if !matches!(
                label.verdict,
                ClinicalVerdict::True | ClinicalVerdict::False
            ) {
                continue;
            }
            if limit.is_some_and(|cap| results.len() >= cap) {
                break 'documents;
            }
            let neuron = neuron_for_case(packet, label);
            let verdict =
                judge_neuron(provider.as_ref(), &neuron, &chunks_for_packet(packet)).await?;
            let accepted =
                verdict.verdict == "accept" && verdict.confidence >= JUDGE_POLICY_THRESHOLD;
            println!(
                "{} {} {} gold={:?} judge={} {:.2}",
                packet.set_id,
                label.relation,
                label.condition,
                label.verdict,
                verdict.verdict,
                verdict.confidence
            );
            results.push(JudgeCaseResult {
                set_id: packet.set_id.clone(),
                relation: label.relation.clone(),
                condition: label.condition.clone(),
                gold: label.verdict,
                verdict: verdict.verdict,
                confidence: verdict.confidence,
                accepted,
                reasoning: verdict.reasoning,
            });
        }
    }
    let score = finalize_judge_score(results);
    let output = options.path("--output", root.join("judge-score.json"));
    fs::write(&output, serde_json::to_string_pretty(&score)?)?;
    println!(
        "judge        true accepted/rejected={}/{}; false rejected/accepted={}/{}; needs_human={}",
        score.true_accepted,
        score.true_rejected,
        score.false_rejected,
        score.false_accepted,
        score.needs_human
    );
    println!("precision    {}", metric(score.accepted_precision));
    println!("saved        {}", output.display());
    Ok(())
}

fn evaluation_subset(
    packets: Vec<ReviewPacket>,
    records: Vec<AnnotationRecord>,
    include_calibration: bool,
) -> (Vec<ReviewPacket>, Vec<AnnotationRecord>) {
    if include_calibration {
        return (packets, records);
    }
    let keep: std::collections::BTreeSet<_> = packets
        .iter()
        .filter(|packet| packet.split == ReviewSplit::Evaluation)
        .map(|packet| packet.set_id.clone())
        .collect();
    (
        packets
            .into_iter()
            .filter(|packet| keep.contains(&packet.set_id))
            .collect(),
        records
            .into_iter()
            .filter(|record| keep.contains(&record.set_id))
            .collect(),
    )
}

fn required_path(options: &Options, name: &str) -> Result<PathBuf> {
    options
        .values
        .get(name)
        .map(PathBuf::from)
        .with_context(|| format!("{name} is required"))
}

fn metric(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_string(), |value| format!("{value:.3}"))
}

#[derive(Default)]
struct Options {
    values: BTreeMap<String, String>,
    flags: std::collections::BTreeSet<String>,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self::default();
        let mut index = 0;
        while index < args.len() {
            let name = &args[index];
            if !name.starts_with("--") {
                bail!("unexpected argument {name}");
            }
            if name == "--include-calibration" {
                options.flags.insert(name.clone());
                index += 1;
                continue;
            }
            let value = args
                .get(index + 1)
                .with_context(|| format!("{name} requires a value"))?;
            options.values.insert(name.clone(), value.clone());
            index += 2;
        }
        Ok(options)
    }

    fn value<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.values.get(name).map_or(default, String::as_str)
    }

    fn path(&self, name: &str, default: impl AsRef<Path>) -> PathBuf {
        self.values
            .get(name)
            .map_or_else(|| default.as_ref().to_path_buf(), PathBuf::from)
    }

    fn usize(&self, name: &str, default: usize) -> Result<usize> {
        self.optional_usize(name)
            .map(|value| value.unwrap_or(default))
    }

    fn optional_usize(&self, name: &str) -> Result<Option<usize>> {
        self.values
            .get(name)
            .map(|value| {
                value
                    .parse::<usize>()
                    .with_context(|| format!("{name} must be an unsigned integer"))
            })
            .transpose()
    }

    fn has(&self, name: &str) -> bool {
        self.flags.contains(name)
    }
}

fn print_help() {
    println!(
        r#"DailyMed clinical domain-expert reference toolkit

prepare  [--corpus PATH] [--output PATH] [--documents 50] [--calibration 10] [--seed TEXT]
validate --annotations PATH [--root PATH]
compare  [--root PATH] [--a PATH] [--b PATH] [--output PATH]
compile  [--root PATH] [--annotations PATH] [--output PATH] [--include-calibration]
score    [--root PATH] [--annotations PATH] [--output PATH] [--include-calibration]
judge    [--root PATH] [--annotations PATH] [--output PATH] [--limit N] [--include-calibration]

Scoring and judge runs use only the evaluation split unless --include-calibration is explicit.
The judge command runs only after complete human labels validate; it never creates gold labels."#
    );
}
