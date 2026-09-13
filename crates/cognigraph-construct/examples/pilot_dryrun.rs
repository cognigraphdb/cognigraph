//! Pilot-readiness dry run (deterministic, no LLM, no network): exercise
//! the construction loop's OPERATIONAL surfaces at 10k-document scale
//! before the real pilot spends credibility on them — ingest wall-clock
//! (grounding + entities + mentions + facts), idempotent re-ingest,
//! evaluate at scale, and proposal-queue listing with pagination at
//! thousands of pending neurons. The LLM-bound stages (propose/review)
//! are covered by the route tests' slice semantics; this measures
//! everything around them.
//!
//! Run: cargo run --release -p cognigraph-construct --example pilot_dryrun -- [CHUNKS] [PROPOSALS]

use cognigraph_construct::*;
use cognigraph_core::{FieldPredicate, GraphBackend, PredicateOp};
use cognigraph_native::NativeBackend;
use serde_json::json;
use std::time::Instant;

fn space() -> SpaceType {
    let entities: Vec<serde_json::Value> = (0..30)
        .map(|i| json!({"name": format!("Vendor{i:02}"), "type": "org", "aliases": [format!("V{i:02} Inc")]}))
        .chain((0..10).map(|i| json!({"name": format!("Platform{i}"), "type": "platform", "aliases": []})))
        .collect();
    let rules: Vec<serde_json::Value> = (0..40)
        .map(|i| {
            json!({
                "source": format!("Vendor{:02}", i % 30),
                "relation": "SUPPLIES",
                "target": format!("Platform{}", i % 10),
                "when_any": [format!("vendor{:02} supplies platform{}", i % 30, i % 10)],
                "require_in_sentence": if i % 4 == 0 { json!(["source"]) } else { json!([]) }
            })
        })
        .collect();
    serde_json::from_value(json!({
        "id": "dryrun", "entities": entities, "relation_rules": rules
    }))
    .unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let n_chunks: usize = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);
    let n_proposals: usize = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(2_000);
    let space = space();

    // Corpus mix: 30% grounding chunks (trigger affirmed, subject in
    // sentence), 20% near-misses (negated trigger), 50% filler.
    let chunks: Vec<Chunk> = (0..n_chunks)
        .map(|i| {
            // Decade-stride rule selection: every rule receives grounding
            // chunks (i%40 would leave most rules uncovered because
            // grounding chunks land only on i%10 in 0..=2).
            let rule = (i / 10) % 40;
            let (vendor, platform) = (rule % 30, rule % 10);
            let text = match i % 10 {
                0..=2 => format!(
                    "Quarterly note {i}. Vendor{vendor:02} supplies Platform{platform} under the \
                     renewed agreement. Deployment continues across regions."
                ),
                3..=4 => format!(
                    "Briefing {i}. It is not true that Vendor{vendor:02} supplies \
                     Platform{platform}; the deal was retracted last quarter."
                ),
                _ => format!(
                    "Filler document {i}: market commentary, hiring notes, and unrelated \
                     product updates spanning several paragraphs of prose."
                ),
            };
            Chunk {
                id: format!("c{i:05}"),
                title: String::new(),
                text,
            }
        })
        .collect();

    let backend = NativeBackend::new();
    let t = Instant::now();
    let grounded = ingest_chunks(&backend, "dryrun", &space, &chunks, &[]).await?;
    let ingest_ms = t.elapsed().as_millis();
    println!(
        "ingest      {n_chunks} chunks -> {grounded} fact edges in {ingest_ms} ms \
         ({:.1} chunks/ms)",
        n_chunks as f64 / ingest_ms.max(1) as f64
    );

    let t = Instant::now();
    let regrounded = ingest_chunks(&backend, "dryrun", &space, &chunks, &[]).await?;
    println!(
        "re-ingest   idempotent pass in {} ms ({} fact edges re-upserted)",
        t.elapsed().as_millis(),
        regrounded
    );

    // Evaluate at scale: every rule's fact expected; 40 forbidden traps.
    let spec: EvalSpec = serde_json::from_value(json!({
        "space_id": "dryrun",
        "questions": [{
            "id": "q1", "question": "coverage",
            "expected_facts": (0..40).map(|i| format!(
                "Vendor{:02} --SUPPLIES--> Platform{}", i % 30, i % 10
            )).collect::<Vec<_>>(),
            "forbidden_facts": (0..40).map(|i| format!(
                "Vendor{:02} --ACQUIRED--> Platform{}", i % 30, i % 10
            )).collect::<Vec<_>>(),
        }]
    }))?;
    let t = Instant::now();
    let outcome = evaluate(&backend, "dryrun", &spec).await?;
    println!(
        "evaluate    recall {}/{} restraint v{}/{} in {} ms",
        outcome.expected_found,
        outcome.expected_total,
        outcome.forbidden_triggered,
        outcome.forbidden_total,
        t.elapsed().as_millis()
    );

    // Proposal queue at pilot scale: create N proposed neurons, list
    // paginated the way `neuron pending`/review slicing will.
    let t = Instant::now();
    for i in 0..n_proposals {
        backend
            .create_document(
                "neurons",
                json!({
                    "_key": format!("p{i:05}"), "id": format!("p{i:05}"),
                    "type": "relation_hint", "status": "proposed",
                    "space_type": "dryrun", "confidence": 0.8,
                    "source": format!("Vendor{:02}", i % 30), "relation": "SUPPLIES",
                    "target": format!("Platform{}", i % 10),
                    "triggers": [format!("alt phrasing {i}")],
                    "evidence": ["…"], "rationale": "dry run",
                }),
            )
            .await?;
    }
    println!(
        "queue       {n_proposals} proposals stored in {} ms",
        t.elapsed().as_millis()
    );
    let predicates = [
        FieldPredicate {
            path: vec!["space_type".into()],
            op: PredicateOp::Eq,
            value: json!("dryrun"),
        },
        FieldPredicate {
            path: vec!["status".into()],
            op: PredicateOp::Eq,
            value: json!("proposed"),
        },
    ];
    let t = Instant::now();
    let mut seen = 0usize;
    let page = 500;
    let mut offset = 0;
    loop {
        let batch = backend
            .list_documents_filtered("neurons", &predicates, None, Some(page), Some(offset))
            .await?;
        if batch.is_empty() {
            break;
        }
        seen += batch.len();
        offset += page;
    }
    println!(
        "pagination  {seen} proposals in pages of {page} in {} ms",
        t.elapsed().as_millis()
    );
    Ok(())
}
