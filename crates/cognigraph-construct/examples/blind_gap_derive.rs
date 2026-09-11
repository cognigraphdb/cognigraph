//! Derive a GAP-BEARING variant of a blind-eval kit, mechanically.
//!
//! Copies a kit directory, stripping every relation rule's `when_any`
//! triggers: the ontology keeps its full vocabulary (entities, aliases,
//! relation triples) but has ZERO grounding pathways, so every expected
//! fact starts as a gap and the repair loop must wire each pathway from
//! document evidence. The derivation makes no content decisions — no
//! judgment about which facts break — which is what keeps the gap variant
//! as blind as its source kit. chunks.jsonl and eval.json are copied
//! byte-identical.
//!
//! Run: cargo run -p cognigraph-construct --example blind_gap_derive -- SRC_DIR DST_DIR

use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(src), Some(dst)) = (args.next(), args.next()) else {
        anyhow::bail!("usage: blind_gap_derive SRC_DIR DST_DIR");
    };
    let (src, dst) = (Path::new(&src), Path::new(&dst));
    std::fs::create_dir_all(dst)?;

    let mut space: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("space_type.json"))?)?;
    let rules = space["relation_rules"]
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("space_type.json has no relation_rules array"))?;
    let total = rules.len();
    for rule in rules.iter_mut() {
        rule["when_any"] = serde_json::json!([]);
    }
    std::fs::write(
        dst.join("space_type.json"),
        serde_json::to_string_pretty(&space)?,
    )?;

    for file in ["chunks.jsonl", "eval.json"] {
        std::fs::copy(src.join(file), dst.join(file))?;
    }

    std::fs::write(
        dst.join("README.md"),
        format!(
            "# Gap-bearing variant (mechanically derived)\n\n\
             Derived from `{}` by `blind_gap_derive`: all {total} relation\n\
             rules kept, every `when_any` trigger list emptied. Vocabulary\n\
             intact, zero grounding pathways — the repair loop must wire\n\
             every expected fact from document evidence. chunks.jsonl and\n\
             eval.json are byte-identical to the source kit. No content\n\
             judgment was involved in the derivation.\n",
            src.display()
        ),
    )?;

    println!(
        "derived {} → {} ({total} rules, all triggers stripped)",
        src.display(),
        dst.display()
    );
    Ok(())
}
