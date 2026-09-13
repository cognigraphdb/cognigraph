//! Design-discussion diagnostic (deterministic, no LLM, no backend): what
//! would an ENDPOINT-PRESENCE gate on grounding do to a kit's recall and
//! restraint? For each candidate gate policy, a grounded fact additionally
//! requires the chunk to MENTION the rule's endpoint entities (by name or
//! alias, the existing `mentions` machinery):
//!
//!   none    — today's behavior (trigger affirmation alone)
//!   source  — the chunk must mention the fact's source entity
//!   target  — the chunk must mention the fact's target entity
//!   both    — the chunk must mention both endpoints
//!
//! Counts are DISTINCT facts (the eval's per-mention counting is reported
//! separately by blind_eval). This simulates; it changes nothing.
//!
//! Run: cargo run --release -p cognigraph-construct --example endpoint_gate_sim -- DIR...

use std::collections::HashSet;
use std::path::Path;

use cognigraph_construct::*;

fn distinct(facts: &[String]) -> Vec<Fact> {
    let mut seen = HashSet::new();
    facts
        .iter()
        .filter_map(|raw| Fact::parse(raw))
        .filter(|fact| seen.insert(fact.clone()))
        .collect()
}

fn main() -> anyhow::Result<()> {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    if dirs.is_empty() {
        anyhow::bail!("usage: endpoint_gate_sim DIR...");
    }

    println!(
        "{:<46} {:>6} {:>12} {:>12} {:>12} {:>12}",
        "kit", "", "none", "chunk-both", "sent-source", "sent-both"
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

        // Surfaces (name + aliases, casefolded) per entity name.
        let surfaces = |name: &str| -> Vec<String> {
            space
                .entities
                .iter()
                .filter(|e| e.name == name)
                .flat_map(|e| std::iter::once(&e.name).chain(e.aliases.iter()))
                .map(|s| s.to_lowercase())
                .collect()
        };
        // The sentence (casefolded) containing a byte offset of the
        // original text — QW5's trigger spans make sentence-scoped gating
        // simulable.
        let sentence_at = |text: &str, offset: usize| -> String {
            let bounds: &[char] = &['.', '!', '?', '\n'];
            let start = text[..offset.min(text.len())]
                .rfind(bounds)
                .map_or(0, |b| b + 1);
            let end = text[offset.min(text.len())..]
                .find(bounds)
                .map_or(text.len(), |b| offset + b + 1);
            text[start..end].to_lowercase()
        };

        // Ground every chunk once; record each grounded fact under each
        // candidate gate.
        #[derive(Default)]
        struct Grounded {
            none: HashSet<Fact>,
            chunk_both: HashSet<Fact>,
            sent_source: HashSet<Fact>,
            sent_both: HashSet<Fact>,
        }
        let mut grounded = Grounded::default();
        for chunk in &chunks {
            let mentioned: HashSet<&str> = mentions(&chunk.text, &space.entities)
                .into_iter()
                .map(|e| e.name.as_str())
                .collect();
            for g in ground_chunk(&chunk.id, &chunk.text, &space, &[]) {
                grounded.none.insert(g.fact.clone());
                if mentioned.contains(g.fact.source.as_str())
                    && mentioned.contains(g.fact.target.as_str())
                {
                    grounded.chunk_both.insert(g.fact.clone());
                }
                let sentence = sentence_at(&chunk.text, g.trigger_span.0);
                let in_sentence =
                    |name: &str| surfaces(name).iter().any(|s| sentence.contains(s.as_str()));
                if in_sentence(&g.fact.source) {
                    grounded.sent_source.insert(g.fact.clone());
                }
                if in_sentence(&g.fact.source) && in_sentence(&g.fact.target) {
                    grounded.sent_both.insert(g.fact.clone());
                }
            }
        }

        let score = |set: &HashSet<Fact>| -> (usize, usize) {
            (
                expected.iter().filter(|f| set.contains(f)).count(),
                forbidden.iter().filter(|f| set.contains(f)).count(),
            )
        };
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let cell =
            |(r, v): (usize, usize)| format!("{r}/{} v{v}/{}", expected.len(), forbidden.len());
        println!(
            "{:<46} {:>6} {:>12} {:>12} {:>12} {:>12}",
            name,
            "R/V",
            cell(score(&grounded.none)),
            cell(score(&grounded.chunk_both)),
            cell(score(&grounded.sent_source)),
            cell(score(&grounded.sent_both)),
        );
    }
    println!(
        "\nR = distinct expected facts grounded (recall), v = distinct forbidden facts grounded (violations)."
    );
    Ok(())
}
