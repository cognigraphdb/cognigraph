//! Pass-2 clinical self-healing demo (decision_dailymed_clinical.md D5):
//! the capstone where the judge and self-healing loop are load-bearing
//! because there is NO oracle to re-gate against.
//!
//! A label revision rewords an indication ("indicated for the treatment of
//! psoriasis" -> "used in the management of psoriasis"). The literal
//! trigger dies, so the TREATS fact loses its grounding pathway even though
//! the fact is still true. The loop must: detect the dead pathway
//! (`degradation_report`), re-propose a trigger from the REVISED prose
//! (gap-directed proposing), gate it through the judge, and restore the
//! pathway — while a co-mentioned forbidden condition (eczema) is never
//! revived (restraint holds through the drift).
//!
//! Uses the propose->judge LLM loop (a couple of calls). Run:
//!   cargo run --release -p cognigraph-construct --example dailymed_clinical_selfheal

use anyhow::Result;
use cognigraph_construct::clinical_reference::JUDGE_POLICY_THRESHOLD;
use cognigraph_construct::*;
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::completion::completion_from_env;
use cognigraph_native::NativeBackend;
use serde_json::json;

const SPACE_ID: &str = "clinical-selfheal";

fn chunk(id: &str, text: &str) -> Chunk {
    Chunk {
        id: id.to_string(),
        title: String::new(),
        text: text.to_string(),
    }
}

async fn grounds(backend: &NativeBackend, source: &str, target: &str) -> Result<bool> {
    let facts = backend.list_documents("facts", None, None).await?;
    let (sk, tk) = (ingest::entity_key(source), ingest::entity_key(target));
    Ok(facts.iter().any(|f| {
        f["_from"]
            .as_str()
            .unwrap_or("")
            .ends_with(&format!("/{sk}"))
            && f["_to"].as_str().unwrap_or("").ends_with(&format!("/{tk}"))
            && f["relation_type"].as_str() == Some("TREATS")
    }))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();

    let product = "This product";
    // Base ontology: TREATS with the literal indication trigger, plus a
    // forbidden co-mention target (eczema) that must never ground.
    let space: SpaceType = serde_json::from_value(json!({
        "id": SPACE_ID,
        "entities": [
            {"name": product, "type": "product", "aliases": []},
            {"name": "psoriasis", "type": "condition", "aliases": []},
            {"name": "eczema", "type": "condition", "aliases": []},
        ],
        "relation_rules": [{
            "source": product, "relation": "TREATS", "target": "psoriasis",
            "when_any": ["indicated for the treatment of {target}"],
        }],
    }))?;

    // v_old: the literal trigger fires; eczema is only a co-mention.
    let v_old = vec![chunk(
        "s0",
        "This product is indicated for the treatment of psoriasis. \
         It should not be confused with agents used for eczema.",
    )];
    // v_new: the indication is REWORDED; the literal trigger no longer fires.
    let v_new = vec![chunk(
        "s0",
        "This product is used in the management of psoriasis. \
         It should not be confused with agents used for eczema.",
    )];

    let backend = NativeBackend::new();
    ingest_chunks(&backend, SPACE_ID, &space, &v_old, &[]).await?;
    println!(
        "v_old        TREATS(product, psoriasis) grounded: {} | forbidden eczema grounded: {}",
        grounds(&backend, product, "psoriasis").await?,
        grounds(&backend, product, "eczema").await?
    );

    // The revision lands. Detect degradation against the revised prose.
    let dead = degradation_report(&space, &[], &v_new);
    println!(
        "revision     {} dead pathway(s): {}",
        dead.len(),
        dead.iter()
            .map(|d| format!(
                "{} --{}--> {}",
                d.fact.source, d.fact.relation, d.fact.target
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if dead.is_empty() {
        anyhow::bail!("expected the reworded indication to dead-end the literal trigger");
    }

    // Re-ground the revised corpus and confirm the fact is now missing.
    let backend = NativeBackend::new();
    ingest_chunks(&backend, SPACE_ID, &space, &v_new, &[]).await?;
    println!(
        "v_new(base)  TREATS(product, psoriasis) grounded: {} (pathway lost)",
        grounds(&backend, product, "psoriasis").await?
    );

    // Gap-directed re-proposal from the REVISED prose (LLM). The propose
    // space declares the relation vocabulary so the hint validates.
    let provider = completion_from_env()?;
    let missing = [Fact {
        source: product.to_string(),
        relation: "TREATS".to_string(),
        target: "psoriasis".to_string(),
    }];
    let report =
        propose_neurons_via_backend(provider.as_ref(), &space, &missing, &backend, SPACE_ID)
            .await?;
    let Some(hint) = report.set.neurons.into_iter().next() else {
        println!("propose      no trigger recovered from the revised prose (governed refusal)");
        return Ok(());
    };
    println!(
        "propose      recovered trigger(s) {:?} from the revised prose",
        hint.triggers
    );

    // Judge gates the recovered hint (no oracle — the judge IS the gate).
    let verdict = judge_neuron(provider.as_ref(), &hint, &v_new).await?;
    println!(
        "judge        verdict: {} (confidence {:.2} — LIVE MODEL OUTPUT, observed this run; \
         it varies between runs and is NOT a deterministic property of the pipeline. \
         What IS deterministic and now ENFORCED: the repair is applied only if the verdict \
         clears the {:.2} policy threshold.)",
        verdict.verdict, verdict.confidence, JUDGE_POLICY_THRESHOLD
    );

    // ENFORCE the policy threshold. Previously this applied the repair on any
    // "accept" verdict regardless of confidence, while printing that the verdict
    // must clear the threshold — a governance demonstration that did not enforce
    // its own gate. The observed runs happened to pass (0.98, 0.99), so the bug
    // never showed in the output; it was still a hole in exactly the guarantee
    // this example exists to demonstrate.
    let clears_policy = verdict.verdict == "accept" && verdict.confidence >= JUDGE_POLICY_THRESHOLD;
    if verdict.verdict == "accept" && !clears_policy {
        println!(
            "policy       WITHHELD — accepted at {:.2} but below the {JUDGE_POLICY_THRESHOLD:.2} \
             threshold; the repair is NOT applied and escalates to a human",
            verdict.confidence
        );
    }

    // Apply the accepted hint and re-ground: pathway restored, forbidden
    // still refused.
    let restored = if clears_policy {
        let mut accepted = hint.clone();
        accepted.status = NeuronStatus::Accepted;
        let healed = effective_config(&space, std::slice::from_ref(&accepted));
        let backend = NativeBackend::new();
        ingest_chunks(&backend, SPACE_ID, &healed, &v_new, &[]).await?;
        let psoriasis = grounds(&backend, product, "psoriasis").await?;
        let eczema = grounds(&backend, product, "eczema").await?;
        println!(
            "healed       TREATS(product, psoriasis) grounded: {psoriasis} | forbidden eczema grounded: {eczema}"
        );
        psoriasis && !eczema
    } else {
        false
    };

    println!(
        "\nself-healing {} — dead clinical pathway {} without an oracle; restraint held",
        if restored { "PASS" } else { "INCOMPLETE" },
        if restored {
            "detected, re-proposed, judge-gated, and restored"
        } else {
            "not restored (see verdict above)"
        }
    );
    Ok(())
}
