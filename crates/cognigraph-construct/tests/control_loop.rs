//! The paper's central claim, executed: knowledge construction as a closed
//! control loop. Measure a gap → propose a governed repair → validate →
//! (human) accept → re-construct → prove the gap closed with restraint held.

use cognigraph_construct::*;
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use std::path::Path;

fn fixtures() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/semantic-neurons"
    ))
}

fn load<T: serde::de::DeserializeOwned>(rel: &str) -> T {
    serde_json::from_str(&std::fs::read_to_string(fixtures().join(rel)).unwrap()).unwrap()
}

fn load_chunks() -> Vec<Chunk> {
    std::fs::read_to_string(fixtures().join("chunks/state_of_the_union_2024.chunks.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// Deterministic stand-in for the LLM proposer: returns a valid
/// relation_hint for the SOTU Ukraine gap.
struct ScriptedProposer;

#[async_trait::async_trait]
impl CompletionProvider for ScriptedProposer {
    fn model_name(&self) -> &str {
        "scripted"
    }
    async fn complete_json(&self, _s: &str, _u: &str, _schema: &Value) -> anyhow::Result<Value> {
        Ok(json!({
            "id": "us-supports-ukraine-loop",
            "type": "relation_hint",
            "confidence": 0.9,
            "rationale": "The speech commits to standing with Ukraine.",
            "source": "United States",
            "relation": "SUPPORTS",
            "target": "Ukraine",
            "triggers": ["stand with Ukraine"],
            "evidence": ["Ukraine can stop Putin if we stand with Ukraine and provide the weapons that it needs."]
        }))
    }
}

async fn run_loop(provider: &dyn CompletionProvider) {
    // 1. Manufacture a gap: strip the Ukraine rule from the base ontology.
    let mut space: SpaceType = load("space_types/demo_policy_speech.json");
    space
        .relation_rules
        .retain(|r| !(r.source == "United States" && r.relation == "SUPPORTS"));
    let spec: EvalSpec = load("evaluations/demo_sotu_2024.json");
    let chunks = load_chunks();

    // 2. Measure: the gap is visible.
    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &space, &chunks, &[])
        .await
        .unwrap();
    let before = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    assert!(!before.recall_ok(), "the manufactured gap must be measured");
    assert!(before.missing.iter().any(|f| f.relation == "SUPPORTS"));

    // 3. Propose (gap-directed) — proposals arrive as `proposed`, inert.
    // Validation needs the relation label in the ontology (neurons cannot
    // introduce vocabulary), so validate against the full base ontology.
    let full_space: SpaceType = load("space_types/demo_policy_speech.json");
    let proposals = propose_neurons(provider, &full_space, &before.missing, &chunks)
        .await
        .unwrap();
    assert!(!proposals.neurons.is_empty(), "proposer produced nothing");
    assert!(
        proposals
            .neurons
            .iter()
            .all(|n| n.status == NeuronStatus::Proposed)
    );

    // Proposed neurons are ignored by construction (governance holds).
    let ignored = effective_config(&space, &proposals.neurons);
    assert_eq!(ignored.relation_rules.len(), space.relation_rules.len());

    // 4. Human review: accept.
    let mut accepted = proposals.neurons.clone();
    for neuron in &mut accepted {
        neuron.status = NeuronStatus::Accepted;
    }

    // 5. Re-construct with the accepted repair; 6. Re-measure.
    let repaired = effective_config(&space, &accepted);
    let backend = NativeBackend::new();
    ingest_chunks(&backend, &spec.space_id, &repaired, &chunks, &[])
        .await
        .unwrap();
    let after = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    assert!(
        after.recall_ok(),
        "accepted repair must close the gap; still missing: {:?}",
        after.missing
    );
    assert!(after.restraint_ok(), "repair must not open violations");
}

#[tokio::test]
async fn closed_loop_with_scripted_proposer() {
    run_loop(&ScriptedProposer).await;
}

/// Live-LLM tests are opt-in (COGNIGRAPH_LIVE_LLM=1): they are conformance
/// probes, not commit gates — model nondeterminism must not block commits.
fn live_llm_enabled() -> bool {
    if std::env::var("COGNIGRAPH_LIVE_LLM").as_deref() == Ok("1") {
        return true;
    }
    eprintln!("Skipping: set COGNIGRAPH_LIVE_LLM=1 to run live-LLM loop tests");
    false
}

#[tokio::test]
async fn closed_loop_with_live_openai() {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    if !live_llm_enabled() {
        return;
    }
    let Ok(key) = std::env::var("OPENAI_API_KEY") else {
        eprintln!("Skipping: OPENAI_API_KEY not set");
        return;
    };
    let provider =
        cognigraph_embeddings::completion::OpenAiCompletion::new(key, None, None).unwrap();
    run_loop(&provider).await;
}

#[tokio::test]
async fn closed_loop_with_live_gemini() {
    dotenvy::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env")).ok();
    if !live_llm_enabled() {
        return;
    }
    let Ok(key) = std::env::var("GEMINI_API_KEY") else {
        eprintln!("Skipping: GEMINI_API_KEY not set");
        return;
    };
    let provider =
        cognigraph_embeddings::completion::GeminiCompletion::new(key, None, None).unwrap();
    run_loop(&provider).await;
}
