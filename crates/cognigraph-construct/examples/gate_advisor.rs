//! The gate advisor over fixture kits (deterministic, no LLM, no
//! backend): for each rule, where should the author spend the
//! sentence-gate precision budget? Reads `space_type.json` +
//! `chunks.jsonl` per directory; add `--strip-gates` to analyze a
//! space AS IF ungated (validation mode: does the advisor rediscover
//! gates an author already applied?).
//!
//! Run: cargo run --release -p cognigraph-construct --example gate_advisor -- [--strip-gates] DIR...

use std::path::Path;

use cognigraph_construct::*;

fn main() -> anyhow::Result<()> {
    let mut strip_gates = false;
    let dirs: Vec<String> = std::env::args()
        .skip(1)
        .filter(|arg| {
            if arg == "--strip-gates" {
                strip_gates = true;
                false
            } else {
                true
            }
        })
        .collect();
    if dirs.is_empty() {
        anyhow::bail!("usage: gate_advisor [--strip-gates] DIR...");
    }

    for dir in &dirs {
        let dir = Path::new(dir);
        let mut space: SpaceType =
            serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
        let authored_gates: Vec<(String, Vec<String>)> = space
            .relation_rules
            .iter()
            .filter(|r| !r.require_in_sentence.is_empty())
            .map(|r| {
                (
                    format!("{} --{}--> {}", r.source, r.relation, r.target),
                    r.require_in_sentence
                        .iter()
                        .map(|e| format!("{e:?}").to_lowercase())
                        .collect(),
                )
            })
            .collect();
        if strip_gates {
            for rule in &mut space.relation_rules {
                rule.require_in_sentence.clear();
            }
        }
        let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        let report = advise_gates(&space, &[], &chunks);
        let suggested = report
            .rules
            .iter()
            .filter(|a| a.suggestion.is_some())
            .count();
        println!(
            "== {} ({} rules, {} chunks; {} suggestion(s){})",
            dir.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default(),
            report.rules.len(),
            chunks.len(),
            suggested,
            if strip_gates {
                format!("; {} authored gate(s) stripped", authored_gates.len())
            } else {
                String::new()
            }
        );
        for advice in &report.rules {
            let cells = format!(
                "ungated {} | src {}/{} | tgt {}/{} | both {}/{}",
                advice.grounds_ungated,
                advice.source_gate.grounds,
                advice.source_gate.blocked,
                advice.target_gate.grounds,
                advice.target_gate.blocked,
                advice.both_gate.grounds,
                advice.both_gate.blocked,
            );
            match &advice.suggestion {
                Some(gate) => {
                    println!("  GATE {:?} {:<58} {}", gate, advice.fact, cells);
                    for sample in &advice.samples {
                        let sentence: String = sample.sentence.chars().take(110).collect();
                        println!("       blocked [{}]: {}", sample.chunk_id, sentence);
                    }
                }
                None if advice.grounds_ungated == 0 => {
                    println!("  dead {:<58} (no grounding anywhere)", advice.fact);
                }
                None if !advice.never_in_sentence.is_empty() => {
                    println!(
                        "  REVIEW {:?} {:<52} {}",
                        advice.never_in_sentence, advice.fact, cells
                    );
                    for sample in advice.samples.iter().take(1) {
                        let sentence: String = sample.sentence.chars().take(110).collect();
                        println!("       e.g. [{}]: {}", sample.chunk_id, sentence);
                    }
                }
                None => {
                    println!("  ok   {:<58} {}", advice.fact, cells);
                }
            }
        }
        if strip_gates && !authored_gates.is_empty() {
            println!("  -- authored gates (ground truth for validation):");
            let mut rediscovered = 0usize;
            for (fact, gates) in &authored_gates {
                let advice = report.rules.iter().find(|a| &a.fact == fact);
                let advised = advice
                    .map(|a| match (&a.suggestion, &a.never_in_sentence) {
                        (Some(g), _) => format!("advisor suggests {g:?}"),
                        (None, flags) if !flags.is_empty() => {
                            format!("advisor flags REVIEW {flags:?}")
                        }
                        _ => "advisor: nothing flagged".to_string(),
                    })
                    .unwrap_or_else(|| "rule not analyzed".to_string());
                let hit = advice.is_some_and(|a| {
                    let named: Vec<String> = a
                        .suggestion
                        .clone()
                        .unwrap_or_else(|| a.never_in_sentence.clone())
                        .iter()
                        .map(|e| format!("{e:?}").to_lowercase())
                        .collect();
                    gates.iter().all(|g| named.contains(g))
                });
                if hit {
                    rediscovered += 1;
                }
                println!(
                    "     {} {fact} authored={gates:?} — {advised}",
                    if hit { "HIT " } else { "MISS" }
                );
            }
            println!(
                "  rediscovered {rediscovered}/{} authored gates",
                authored_gates.len()
            );
        }
        println!();
    }
    Ok(())
}
