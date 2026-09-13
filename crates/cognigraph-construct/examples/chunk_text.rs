//! Blind-eval helper: split a plain .txt/.md document into chunks.jsonl.
//! Mechanical paragraph packing to a char budget — no content judgment, no
//! grounding, nothing that could leak an answer. Hand-edit the output
//! afterwards if a split lands awkwardly.
//!
//! Run: cargo run -p cognigraph-construct --example chunk_text -- DOC.md [CHARS]
//!   writes JSONL to stdout; redirect into chunks.jsonl.

use std::io::Write;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: chunk_text DOC[.md|.txt] [char_budget]"))?;
    let budget: usize = args.next().map(|s| s.parse()).transpose()?.unwrap_or(1000);
    let raw = std::fs::read_to_string(&path)?;

    // Paragraphs = blank-line-separated blocks, whitespace collapsed.
    let paragraphs: Vec<String> = raw
        .split("\n\n")
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect();

    // Greedy-pack paragraphs up to the budget; split any single paragraph
    // that exceeds 2x the budget on sentence boundaries.
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for para in paragraphs {
        for piece in split_long(&para, budget * 2) {
            if !current.is_empty() && current.len() + piece.len() + 1 > budget {
                chunks.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&piece);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for (i, text) in chunks.iter().enumerate() {
        let line = serde_json::json!({ "id": format!("c-{i:03}"), "text": text });
        writeln!(out, "{}", serde_json::to_string(&line)?)?;
    }
    eprintln!(
        "{} chunks (budget {budget} chars) from {path}",
        chunks.len()
    );
    Ok(())
}

/// Split an over-long paragraph on sentence boundaries so no piece exceeds
/// ~`max`; short paragraphs pass through unchanged.
fn split_long(para: &str, max: usize) -> Vec<String> {
    if para.len() <= max {
        return vec![para.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for sentence in para.split_inclusive(['.', '!', '?']) {
        if !current.is_empty() && current.len() + sentence.len() > max {
            out.push(std::mem::take(&mut current).trim().to_string());
        }
        current.push_str(sentence);
    }
    let last = current.trim();
    if !last.is_empty() {
        out.push(last.to_string());
    }
    out
}
