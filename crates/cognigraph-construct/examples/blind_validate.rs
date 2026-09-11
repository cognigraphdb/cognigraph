//! Blind-eval STRUCTURAL validator. Checks that the three authored files
//! parse and cross-reference — every fact names a declared entity and an
//! existing relation, every rule endpoint is declared. It NEVER ingests,
//! grounds, or evaluates: it cannot and must not reveal whether any fact
//! would actually ground, because that would defeat the blind protocol.
//! A typo-catcher, not an oracle.
//!
//! Run: cargo run -p cognigraph-construct --example blind_validate -- DIR
//! Exit code 1 if any errors; warnings do not fail.

use std::collections::BTreeSet;
use std::path::Path;

use cognigraph_construct::{Chunk, EvalSpec, Fact, SpaceType};

fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let dir = Path::new(&dir);
    let space: SpaceType =
        serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
    let spec: EvalSpec = serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
    let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            serde_json::from_str(l).map_err(|e| anyhow::anyhow!("chunks.jsonl line {}: {e}", i + 1))
        })
        .collect::<Result<_, _>>()?;

    let entities: BTreeSet<String> = space.entities.iter().map(|e| e.name.clone()).collect();
    let relations: BTreeSet<String> = space
        .relation_rules
        .iter()
        .map(|r| r.relation.clone())
        .collect();

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let mut seen = BTreeSet::new();
    for e in &space.entities {
        if !seen.insert(e.name.as_str()) {
            warnings.push(format!("entity `{}` declared more than once", e.name));
        }
    }
    for (i, r) in space.relation_rules.iter().enumerate() {
        if r.when_any.iter().all(|t| t.trim().is_empty()) {
            warnings.push(format!(
                "relation_rule[{i}] {} --{}--> {} has no triggers (can never ground)",
                r.source, r.relation, r.target
            ));
        }
        if !entities.contains(r.source.as_str()) {
            errors.push(format!(
                "relation_rule[{i}] source `{}` is not a declared entity",
                r.source
            ));
        }
        if !entities.contains(r.target.as_str()) {
            errors.push(format!(
                "relation_rule[{i}] target `{}` is not a declared entity",
                r.target
            ));
        }
    }

    for q in &spec.questions {
        for raw in &q.expected_facts {
            if let Some((is_error, msg)) = classify(raw, false, &q.id, &entities, &relations) {
                if is_error {
                    errors.push(msg);
                } else {
                    warnings.push(msg);
                }
            }
        }
        for raw in &q.forbidden_facts {
            if let Some((is_error, msg)) = classify(raw, true, &q.id, &entities, &relations) {
                if is_error {
                    errors.push(msg);
                } else {
                    warnings.push(msg);
                }
            }
        }
    }

    let distinct_relations = relations.len();
    println!(
        "space_type `{}`: {} entities, {} relations, {} rules",
        space.id,
        entities.len(),
        distinct_relations,
        space.relation_rules.len()
    );
    println!("chunks: {}", chunks.len());
    let expected: usize = spec.questions.iter().map(|q| q.expected_facts.len()).sum();
    let forbidden: usize = spec.questions.iter().map(|q| q.forbidden_facts.len()).sum();
    println!(
        "eval `{}`: {} questions, {expected} expected facts, {forbidden} forbidden facts\n",
        spec.space_id,
        spec.questions.len()
    );

    for w in &warnings {
        println!("WARN  {w}");
    }
    for e in &errors {
        println!("ERROR {e}");
    }
    println!(
        "\n{} — {} error(s), {} warning(s)",
        if errors.is_empty() { "PASS" } else { "FAIL" },
        errors.len(),
        warnings.len()
    );
    if errors.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

/// Returns `Some((is_error, message))` if a fact has a structural problem.
/// Expected-fact problems are errors (the fact can never ground);
/// forbidden-fact problems are warnings (the trap is trivially safe, so it
/// tests nothing) — never a grounding check.
fn classify(
    raw: &str,
    forbidden: bool,
    qid: &str,
    entities: &BTreeSet<String>,
    relations: &BTreeSet<String>,
) -> Option<(bool, String)> {
    let Some(fact) = Fact::parse(raw) else {
        return Some((true, format!("[{qid}] fact `{raw}` is not `A --REL--> B`")));
    };
    let mut problems = Vec::new();
    if !entities.contains(fact.source.as_str()) {
        problems.push(format!("source `{}` undeclared", fact.source));
    }
    if !entities.contains(fact.target.as_str()) {
        problems.push(format!("target `{}` undeclared", fact.target));
    }
    if !relations.contains(fact.relation.as_str()) {
        problems.push(format!("relation `{}` not in ontology", fact.relation));
    }
    if problems.is_empty() {
        return None;
    }
    let kind = if forbidden { "forbidden" } else { "expected" };
    let base = format!("[{qid}] {kind} fact `{raw}`: {}", problems.join(", "));
    if forbidden {
        Some((
            false,
            format!("{base} — can never ground, so it tests nothing"),
        ))
    } else {
        Some((true, base))
    }
}
