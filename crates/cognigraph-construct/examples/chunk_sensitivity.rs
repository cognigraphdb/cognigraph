//! Design-session diagnostic for cross-chunk grounding (deterministic,
//! no LLM, no backend): what does CHUNK LOCALITY actually cost, and
//! what would each candidate remedy recover — at what restraint price?
//!
//! Regimes, per kit (space as authored, gates included):
//!   original    — the kit's chunking (authors keep evidence in-chunk)
//!   sent-split  — every chunk re-split into single SENTENCES (the
//!                 adversarial chunking floor: no evidence may span)
//!   window2/3   — sentence-chunks re-joined into sliding windows of
//!                 2/3 (stride 1): does context concatenation recover
//!                 what splitting broke, and does it leak?
//!   lookback1   — original chunks, but require_in_sentence gates may
//!                 satisfy the endpoint from the PREVIOUS sentence too
//!                 (simulated externally from ungated trigger spans,
//!                 the endpoint_gate_sim methodology; lookback0 is the
//!                 sanity row and must equal original)
//!
//! Run: cargo run --release -p cognigraph-construct --example chunk_sensitivity -- DIR...

use std::collections::HashSet;
use std::path::Path;

use cognigraph_construct::*;

/// Sentence split with the production boundary rules ('.', '!', '?',
/// '\n'; a dot between digits is a decimal point).
fn sentences(text: &str) -> Vec<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        let boundary = matches!(c, '!' | '?' | '\n')
            || (c == '.' && (i + 1 >= bytes.len() || bytes[i + 1].is_whitespace()));
        if boundary {
            let sentence: String = bytes[start..=i].iter().collect();
            if !sentence.trim().is_empty() {
                out.push(sentence.trim().to_string());
            }
            start = i + 1;
        }
        i += 1;
    }
    let tail: String = bytes[start..].iter().collect();
    if !tail.trim().is_empty() {
        out.push(tail.trim().to_string());
    }
    out
}

fn distinct(facts: &[String]) -> Vec<Fact> {
    let mut seen = HashSet::new();
    facts
        .iter()
        .filter_map(|raw| Fact::parse(raw))
        .filter(|fact| seen.insert(fact.clone()))
        .collect()
}

fn ground_all(chunks: &[Chunk], space: &SpaceType) -> HashSet<Fact> {
    let mut facts = HashSet::new();
    for chunk in chunks {
        for g in ground_chunk(&chunk.id, &chunk.text, space, &[]) {
            facts.insert(g.fact);
        }
    }
    facts
}

fn main() -> anyhow::Result<()> {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        anyhow::bail!("usage: chunk_sensitivity DIR...");
    }
    println!(
        "{:<44} {:>4} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12}",
        "kit", "", "original", "sent-split", "window2", "window3", "lookback0", "lookback1"
    );

    for dir in &dirs {
        let dir = Path::new(dir);
        let space: SpaceType =
            serde_json::from_str(&std::fs::read_to_string(dir.join("space_type.json"))?)?;
        let spec: EvalSpec =
            serde_json::from_str(&std::fs::read_to_string(dir.join("eval.json"))?)?;
        let chunks: Vec<Chunk> = std::fs::read_to_string(dir.join("chunks.jsonl"))?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

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
        let score = |facts: &HashSet<Fact>| -> (usize, usize) {
            (
                expected.iter().filter(|f| facts.contains(f)).count(),
                forbidden.iter().filter(|f| facts.contains(f)).count(),
            )
        };

        // Sentence-granularity re-chunking (documents stay separate:
        // sentences never cross the original chunk's boundary, which is
        // itself conservative — real chunkers split WITHIN documents).
        let mut split: Vec<Chunk> = Vec::new();
        for chunk in &chunks {
            for (i, sentence) in sentences(&chunk.text).into_iter().enumerate() {
                split.push(Chunk {
                    id: format!("{}-s{}", chunk.id, i),
                    title: chunk.title.clone(),
                    text: sentence,
                });
            }
        }
        // Sliding windows of K sentence-chunks, stride 1, within the
        // original chunk (document) only.
        let windows = |k: usize| -> Vec<Chunk> {
            let mut out = Vec::new();
            // group split chunks by original id prefix
            let mut by_doc: Vec<(String, Vec<&Chunk>)> = Vec::new();
            for chunk in &split {
                let doc = chunk.id.rsplit_once("-s").map(|(d, _)| d).unwrap_or("");
                match by_doc.last_mut() {
                    Some((last, list)) if last == doc => list.push(chunk),
                    _ => by_doc.push((doc.to_string(), vec![chunk])),
                }
            }
            for (doc, list) in &by_doc {
                if list.len() < k {
                    out.push(Chunk {
                        id: format!("{doc}-w0"),
                        title: String::new(),
                        text: list
                            .iter()
                            .map(|c| c.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                    });
                    continue;
                }
                for start in 0..=(list.len() - k) {
                    out.push(Chunk {
                        id: format!("{doc}-w{start}"),
                        title: String::new(),
                        text: list[start..start + k]
                            .iter()
                            .map(|c| c.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                    });
                }
            }
            out
        };

        // Gate lookback simulation on ORIGINAL chunks: ground with gates
        // stripped to get spans, then re-apply each gated rule's gate
        // externally with an N-sentence lookback. lookback0 must equal
        // the authored behavior (sanity).
        let mut ungated = space.clone();
        for rule in &mut ungated.relation_rules {
            rule.require_in_sentence.clear();
        }
        let surfaces = |name: &str| -> Vec<String> {
            space
                .entities
                .iter()
                .filter(|e| e.name == name)
                .flat_map(|e| std::iter::once(&e.name).chain(e.aliases.iter()))
                .map(|s| s.to_lowercase())
                .collect()
        };
        let lookback = |n: usize| -> HashSet<Fact> {
            let mut facts = HashSet::new();
            for chunk in &chunks {
                let sents = sentences(&chunk.text);
                for g in ground_chunk(&chunk.id, &chunk.text, &ungated, &[]) {
                    let rule = space.relation_rules.iter().find(|r| {
                        r.source == g.fact.source
                            && r.relation == g.fact.relation
                            && r.target == g.fact.target
                    });
                    let Some(rule) = rule else { continue };
                    if rule.require_in_sentence.is_empty() {
                        facts.insert(g.fact);
                        continue;
                    }
                    // Sentence index of the trigger span start.
                    let mut offset = 0usize;
                    let mut index = 0usize;
                    for (i, s) in sents.iter().enumerate() {
                        let at = chunk.text[offset..].find(s.as_str()).map(|p| offset + p);
                        if let Some(at) = at {
                            if g.trigger_span.0 >= at && g.trigger_span.0 < at + s.len() {
                                index = i;
                                break;
                            }
                            offset = at + s.len();
                        }
                    }
                    let window_cf: String = sents[index.saturating_sub(n)..=index]
                        .join(" ")
                        .to_lowercase();
                    let passes = rule.require_in_sentence.iter().all(|endpoint| {
                        let name = match endpoint {
                            EndpointRef::Source => &rule.source,
                            EndpointRef::Target => &rule.target,
                        };
                        surfaces(name)
                            .iter()
                            .any(|s| window_cf.contains(s.as_str()))
                    });
                    if passes {
                        facts.insert(g.fact);
                    }
                }
            }
            facts
        };

        if std::env::var("CHUNK_SENSITIVITY_VERBOSE").is_ok() {
            let split_facts = ground_all(&split, &space);
            for fact in expected.iter().filter(|f| !split_facts.contains(f)) {
                println!(
                    "   LOST under sent-split: {} --{}--> {}",
                    fact.source, fact.relation, fact.target
                );
            }
        }
        let cell =
            |(r, v): (usize, usize)| format!("{r}/{} v{v}/{}", expected.len(), forbidden.len());
        println!(
            "{:<44} {:>4} {:>12} {:>12} {:>12} {:>12} {:>12} {:>12}",
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            "R/V",
            cell(score(&ground_all(&chunks, &space))),
            cell(score(&ground_all(&split, &space))),
            cell(score(&ground_all(&windows(2), &space))),
            cell(score(&ground_all(&windows(3), &space))),
            cell(score(&lookback(0))),
            cell(score(&lookback(1))),
        );
    }
    println!(
        "\nR = distinct expected facts grounded, v = distinct forbidden facts grounded.\n\
         sent-split = single-sentence chunks (adversarial floor); windows re-join within the\n\
         original chunk; lookback1 = authored gates satisfied from the previous sentence too."
    );
    Ok(())
}
