//! D5 measurement gate for the ontology drafter
//! (decision_ontology_drafter.md): draft a space type from each kit's
//! chunks with the AUTHOR ONTOLOGY WITHHELD, ground the corpus with the
//! draft, and score against the kit's eval spec. Scoring is
//! label-lenient and direction-respecting: drafted names and relation
//! labels won't string-match the author's, so an expected fact counts
//! as covered when a drafted edge connects surface-matching endpoints
//! in the right direction under ANY label; violations are counted the
//! same way, which is CONSERVATIVE for restraint. No pass threshold —
//! this measures how good a STARTING POINT the draft is; the honest
//! number ships whatever it is.
//!
//! Run: cargo run --release -p cognigraph-construct --example ontology_draft_eval -- DIR...

use std::collections::HashSet;
use std::path::Path;

use cognigraph_construct::*;
use cognigraph_embeddings::completion::{CompletionProvider, OpenAiCompletion};

fn surfaces(space: &SpaceType, name: &str) -> Vec<String> {
    space
        .entities
        .iter()
        .filter(|e| e.name == name)
        .flat_map(|e| std::iter::once(&e.name).chain(e.aliases.iter()))
        .map(|s| s.to_lowercase())
        .collect()
}

/// Label-lenient endpoint match: any drafted surface of the endpoint
/// contains, or is contained by, the reference endpoint string.
fn matches(surfaces: &[String], reference: &str) -> bool {
    let reference = reference.to_lowercase();
    surfaces
        .iter()
        .any(|s| s.contains(&reference) || reference.contains(s.as_str()))
}

fn distinct(facts: &[String]) -> Vec<Fact> {
    let mut seen = HashSet::new();
    facts
        .iter()
        .filter_map(|raw| Fact::parse(raw))
        .filter(|f| seen.insert(f.clone()))
        .collect()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        anyhow::bail!("usage: ontology_draft_eval DIR...");
    }
    let key =
        std::env::var("OPENAI_API_KEY").map_err(|_| anyhow::anyhow!("OPENAI_API_KEY required"))?;
    let provider = OpenAiCompletion::new(key, None, None)?;
    println!(
        "completion model: {} @ {DRAFT_REV}\n",
        provider.model_name()
    );

    let (mut cov_all, mut exp_all, mut vio_all, mut forb_all) = (0usize, 0usize, 0usize, 0usize);
    for dir in &dirs {
        let dir = Path::new(dir);
        let author: SpaceType =
            serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
        let spec: EvalSpec =
            serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
        let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        // The drafter sees chunks only — the author ontology is used
        // strictly for the reference row below.
        let report = draft_space_type(&provider, "draft-eval", &chunks, 40).await?;
        let draft = &report.space;

        // Ground the corpus with the draft; collect distinct fact triples.
        let mut grounded: HashSet<(String, String, String)> = HashSet::new();
        for chunk in &chunks {
            for g in ground_chunk(&chunk.id, &chunk.text, draft, &[]) {
                grounded.insert((g.fact.source, g.fact.relation, g.fact.target));
            }
        }

        let expected = distinct(
            &spec
                .questions
                .iter()
                .flat_map(|q| q.expected_facts.clone())
                .collect::<Vec<_>>(),
        );
        let forbidden = distinct(
            &spec
                .questions
                .iter()
                .flat_map(|q| q.forbidden_facts.clone())
                .collect::<Vec<_>>(),
        );
        let connects = |fact: &Fact| -> bool {
            grounded.iter().any(|(s, _, t)| {
                matches(&surfaces(draft, s), &fact.source)
                    && matches(&surfaces(draft, t), &fact.target)
            })
        };
        let covered: Vec<&Fact> = expected.iter().filter(|f| connects(f)).collect();
        let violated: Vec<&Fact> = forbidden.iter().filter(|f| connects(f)).collect();

        // Entity coverage: how much of the author's vocabulary did the
        // draft discover? (Drafted surfaces vs author entity names,
        // same bidirectional-containment leniency.)
        let drafted_surfaces: Vec<String> = draft
            .entities
            .iter()
            .flat_map(|e| std::iter::once(&e.name).chain(e.aliases.iter()))
            .map(|s| s.to_lowercase())
            .collect();
        let entities_covered = author
            .entities
            .iter()
            .filter(|e| matches(&drafted_surfaces, &e.name))
            .count();

        println!(
            "== {}  ({} chunks, {} sampled)",
            dir.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default(),
            chunks.len(),
            report.sampled_chunks
        );
        println!(
            "   draft: {} entities, {} rules, {} skips  (author reference: {} entities, {} rules)",
            draft.entities.len(),
            draft.relation_rules.len(),
            report.skips.len(),
            author.entities.len(),
            author.relation_rules.len()
        );
        println!(
            "   author-entity coverage {}/{}",
            entities_covered,
            author.entities.len()
        );
        println!(
            "   grounded {} distinct facts | expected covered {}/{} (label-lenient) | \
             forbidden connected {}/{} | advisor: {} suggestion(s), {} review flag(s)",
            grounded.len(),
            covered.len(),
            expected.len(),
            violated.len(),
            forbidden.len(),
            report
                .advisor
                .rules
                .iter()
                .filter(|a| a.suggestion.is_some())
                .count(),
            report
                .advisor
                .rules
                .iter()
                .filter(|a| a.suggestion.is_none() && !a.never_in_sentence.is_empty())
                .count(),
        );
        if std::env::var("DRAFT_EVAL_VERBOSE").is_ok() {
            for rule in &draft.relation_rules {
                println!(
                    "     drafted: {} --{}--> {}  when_any={:?}",
                    rule.source, rule.relation, rule.target, rule.when_any
                );
            }
            for entity in &draft.entities {
                println!("     entity: {}  aliases={:?}", entity.name, entity.aliases);
            }
        }
        for fact in expected.iter().filter(|f| !connects(f)) {
            println!(
                "     MISSED  {} --{}--> {}",
                fact.source, fact.relation, fact.target
            );
        }
        for fact in &violated {
            println!(
                "     CONNECTED-FORBIDDEN  {} --{}--> {}",
                fact.source, fact.relation, fact.target
            );
        }
        cov_all += covered.len();
        exp_all += expected.len();
        vio_all += violated.len();
        forb_all += forbidden.len();
        println!();
    }
    println!(
        "COMBINED  expected covered {cov_all}/{exp_all} (label-lenient) | \
         forbidden connected {vio_all}/{forb_all}"
    );
    Ok(())
}
