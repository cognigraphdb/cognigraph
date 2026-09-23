//! Exact construction output of the public Semantic Neurons kits (CG-95).
//!
//! `parity.rs` asserts recall and restraint; this test pins everything the
//! grounding path writes (entities, mentions, facts, their evidence spans and
//! semantics signals) plus the full evaluation outcome for every kit. Any
//! change to grounding behavior shows up as a diff against
//! `tests/snapshots/grounding-kits.json`. Regenerate deliberately with
//! `CG_UPDATE_SNAPSHOT=1 cargo test -p cognigraph-construct --test grounding_snapshot`.

use cognigraph_construct::*;
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

const KITS: [(&str, &str, &str, Option<&str>); 4] = [
    (
        "demo_policy_speech",
        "demo_sotu_2024",
        "state_of_the_union_2024",
        Some("demo_policy_speech"),
    ),
    (
        "digital_forensics",
        "forensics_case_alpha",
        "digital_forensics_case_alpha",
        None,
    ),
    (
        "digital_forensics",
        "forensics_case_bravo",
        "digital_forensics_case_bravo",
        None,
    ),
    (
        "pharma_research",
        "pharma_study_px101",
        "pharma_study_px101",
        None,
    ),
];

/// Write timestamps differ on every run; everything else must not.
const VOLATILE: [&str; 2] = ["created_at", "updated_at"];

fn fixtures() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/semantic-neurons"
    ))
}

fn snapshot_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/grounding-kits.json")
}

fn read<T: serde::de::DeserializeOwned>(relative: &str) -> T {
    serde_json::from_str(&std::fs::read_to_string(fixtures().join(relative)).unwrap()).unwrap()
}

fn chunks(name: &str) -> Vec<Chunk> {
    std::fs::read_to_string(fixtures().join(format!("chunks/{name}.chunks.jsonl")))
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

async fn construct(space: &str, eval: &str, chunk_set: &str, neurons: Option<&str>) -> Value {
    let space: SpaceType = read(&format!("space_types/{space}.json"));
    let (config, vetoes) = match neurons {
        Some(name) => {
            let set: NeuronSet = read(&format!("neurons/{name}.neurons.json"));
            validate_neurons(&set, &space).unwrap();
            (
                effective_config(&space, &set.neurons),
                effective_vetoes(&set.neurons),
            )
        }
        None => (space.clone(), Vec::new()),
    };
    let spec: EvalSpec = read(&format!("evaluations/{eval}.json"));
    let backend = NativeBackend::new();
    ingest_chunks(
        &backend,
        &spec.space_id,
        &config,
        &chunks(chunk_set),
        &vetoes,
    )
    .await
    .unwrap();
    let outcome = evaluate(&backend, &spec.space_id, &spec).await.unwrap();
    let mut collections = Map::new();
    for info in backend.list_collections().await.unwrap() {
        let mut documents = backend
            .list_documents(&info.name, None, None)
            .await
            .unwrap();
        for document in &mut documents {
            if let Some(object) = document.as_object_mut() {
                for field in VOLATILE {
                    object.remove(field);
                }
            }
        }
        documents.sort_by_key(|d| d["_key"].as_str().unwrap_or_default().to_string());
        collections.insert(info.name, Value::Array(documents));
    }
    let mut missing: Vec<String> = outcome.missing.iter().map(|f| format!("{f:?}")).collect();
    let mut violations: Vec<String> = outcome
        .violations
        .iter()
        .map(|f| format!("{f:?}"))
        .collect();
    missing.sort();
    violations.sort();
    let outcome = json!({
        "space_id": outcome.space_id,
        "expected_total": outcome.expected_total,
        "expected_found": outcome.expected_found,
        "forbidden_total": outcome.forbidden_total,
        "forbidden_triggered": outcome.forbidden_triggered,
        "missing": missing,
        "violations": violations,
    });
    json!({ "outcome": outcome, "collections": collections })
}

#[tokio::test]
async fn construction_output_of_every_public_kit_matches_the_snapshot() {
    let mut actual = Map::new();
    for (space, eval, chunk_set, neurons) in KITS {
        actual.insert(
            eval.to_string(),
            construct(space, eval, chunk_set, neurons).await,
        );
    }
    let actual = Value::Object(actual);
    if std::env::var("CG_UPDATE_SNAPSHOT").as_deref() == Ok("1") {
        std::fs::create_dir_all(snapshot_path().parent().unwrap()).unwrap();
        std::fs::write(
            snapshot_path(),
            serde_json::to_string_pretty(&actual).unwrap() + "\n",
        )
        .unwrap();
    }
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(snapshot_path()).expect("snapshot")).unwrap();
    for (kit, value) in expected.as_object().unwrap() {
        assert_eq!(
            &actual[kit], value,
            "{kit}: construction output differs from the snapshot"
        );
    }
    assert_eq!(actual, expected);
}
