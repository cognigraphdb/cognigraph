use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use cognigraph_construct::{
    Chunk, DerivationOptions, DirectedRelation, MaterializationOptions, SpaceType,
    derive_fact_rows, derive_materialized_graph, directed_ingest, ground_chunk, ingest_chunks,
};
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::{NativeBackend, StorageMode, VectorMode};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

const CASES: &[(&str, &str, &str)] = &[
    ("Cafe\u{301}. Alpha supplies Beta.", "Alpha", "supplies"),
    ("Cafe\u{301} de\u{301}ploie Beta.", "Café", "déploie"),
    (
        "👩‍🔬 \u{1100}\u{1161}. Alpha supplies Beta.",
        "Alpha",
        "supplies",
    ),
];

fn config() -> SpaceType {
    serde_json::from_value(json!({
        "id":"evidence", "entities":[
            {"name":"Alpha"}, {"name":"Café"}, {"name":"Beta"}
        ], "relation_rules":[
            {"source":"Alpha", "relation":"SUPPLIES", "target":"Beta", "when_any":["supplies"]},
            {"source":"Café", "relation":"SUPPLIES", "target":"Beta", "when_any":["déploie"]}
        ]
    }))
    .unwrap()
}

fn chunks() -> Vec<Chunk> {
    CASES
        .iter()
        .enumerate()
        .map(|(i, (text, _, _))| Chunk {
            id: format!("c{i}"),
            title: "Evidence".into(),
            text: (*text).into(),
        })
        .collect()
}

fn canonical(chunks: &[Chunk]) -> Vec<Chunk> {
    chunks
        .iter()
        .map(|chunk| Chunk {
            text: chunk.text.nfc().collect(),
            ..chunk.clone()
        })
        .collect()
}

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "cognigraph-cg5-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn open(&self, name: &str, vectors: VectorMode, storage: StorageMode) -> NativeBackend {
        NativeBackend::open_with_modes(self.0.join(name), vectors, storage, 1024 * 1024).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn modes() -> [(VectorMode, StorageMode); 3] {
    [
        (VectorMode::Embedded, StorageMode::Resident),
        (VectorMode::Sidecar, StorageMode::Resident),
        (VectorMode::Sidecar, StorageMode::Paged),
    ]
}

fn logical(mut rows: Vec<Value>) -> Vec<Value> {
    for row in &mut rows {
        let row = row.as_object_mut().unwrap();
        for key in ["created_at", "updated_at"] {
            row.remove(key);
        }
    }
    rows.sort_by(|a, b| a["_key"].as_str().cmp(&b["_key"].as_str()));
    rows
}

async fn evidence(backend: &NativeBackend) -> Value {
    let chunks = backend.list_documents("chunks", None, None).await.unwrap();
    let facts = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(chunks.len(), CASES.len());
    assert_eq!(facts.len(), CASES.len());
    for chunk in &chunks {
        let text = chunk["text"].as_str().unwrap();
        assert!(unicode_normalization::is_nfc(text));
        let digest: String = Sha256::digest(text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(chunk["content_hash"], format!("sha256:{digest}"));
        let fact = facts
            .iter()
            .find(|fact| fact["evidence_chunk_id"] == chunk["chunk_id"])
            .unwrap();
        let start = fact["trigger_start"].as_u64().unwrap() as usize;
        let end = fact["trigger_end"].as_u64().unwrap() as usize;
        let slice = text
            .get(start..end)
            .expect("stored span must be a valid UTF-8 range");
        assert_eq!(
            slice.to_lowercase(),
            fact["trigger"].as_str().unwrap().to_lowercase()
        );
        assert_eq!(text.find(slice), Some(start));
    }
    json!({"chunks":logical(chunks), "facts":logical(facts),
        "mentions":logical(backend.list_documents("mentions",None,None).await.unwrap())})
}

async fn roundtrip(
    backend: NativeBackend,
    dir: &Directory,
    vectors: VectorMode,
    storage: StorageMode,
) {
    let before = evidence(&backend).await;
    let snapshot = backend.export_snapshot().await.unwrap();
    let restored = dir.open("restored.redb", vectors, storage);
    restored.import_snapshot(&snapshot).await.unwrap();
    assert_eq!(restored.export_snapshot().await.unwrap(), snapshot);
    assert_eq!(evidence(&restored).await, before);
    drop(restored);
    drop(backend);
    let reopened = dir.open("source.redb", vectors, storage);
    assert_eq!(evidence(&reopened).await, before);
    let restored = dir.open("restored.redb", vectors, storage);
    assert_eq!(evidence(&restored).await, before);
}

#[tokio::test]
async fn rule_ingest_hashes_spans_and_occurrences_survive_normalization_reingest_and_restart() {
    for (vectors, storage) in modes() {
        let dir = Directory::new();
        let backend = dir.open("source.redb", vectors, storage);
        let chunks = chunks();
        assert_eq!(
            ingest_chunks(&backend, "evidence", &config(), &chunks, &[])
                .await
                .unwrap(),
            3
        );
        let before = evidence(&backend).await;
        for revision in [&canonical(&chunks), &chunks, &canonical(&chunks)] {
            assert_eq!(
                ingest_chunks(&backend, "evidence", &config(), revision, &[])
                    .await
                    .unwrap(),
                3
            );
            assert_eq!(evidence(&backend).await, before);
        }
        roundtrip(backend, &dir, vectors, storage).await;
    }
}

struct Proposals;
#[async_trait]
impl CompletionProvider for Proposals {
    async fn complete_json(&self, _: &str, prompt: &str, _: &Value) -> anyhow::Result<Value> {
        assert!(
            unicode_normalization::is_nfc(prompt),
            "provider must see the canonical evidence"
        );
        Ok(
            json!({"facts":CASES.iter().enumerate().map(|(i,(text,source,_))| json!({
            "source":source.nfd().collect::<String>(), "source_type":"party",
            "target":"Beta", "target_type":"party", "relation":"SUPPLIES",
            "evidence":text, "chunk_id":format!("c{i}")
        })).collect::<Vec<_>>()}),
        )
    }
    fn model_name(&self) -> &str {
        "synthetic-nfc"
    }
}

#[tokio::test]
async fn directed_ingest_grounds_canonical_quotes_before_writing_and_reconciles_identically() {
    let taxonomy = [DirectedRelation {
        relation: "SUPPLIES".into(),
        description: "supplies".into(),
        require_in_sentence: vec!["supplies".into(), "de\u{301}ploie".into()],
    }];
    for (vectors, storage) in modes() {
        let dir = Directory::new();
        let backend = dir.open("source.redb", vectors, storage);
        let chunks = chunks();
        let result = directed_ingest(&backend, "evidence", &taxonomy, &chunks, &Proposals)
            .await
            .unwrap();
        assert_eq!(result.facts_grounded, 3, "{:?}", result.skips);
        let before = evidence(&backend).await;
        for revision in [&canonical(&chunks), &chunks] {
            let result = directed_ingest(&backend, "evidence", &taxonomy, revision, &Proposals)
                .await
                .unwrap();
            assert_eq!(result.facts_grounded, 3, "{:?}", result.skips);
            assert_eq!(evidence(&backend).await, before);
        }
        roundtrip(backend, &dir, vectors, storage).await;
    }
}

#[tokio::test]
async fn canonical_materialization_matches_ingestion_and_semantic_derivation() {
    let chunks = chunks();
    let config = config();
    let options = MaterializationOptions {
        max_entity_count: 10,
        max_chunk_count: 10,
        max_mention_count: 20,
        max_fact_occurrence_count: 20,
        max_semantic_fact_count: 20,
        yield_every_chunks: 1,
    };
    let projection = derive_materialized_graph("evidence", &chunks, &config, &[], options)
        .await
        .unwrap();
    projection.validate().unwrap();
    let normalized =
        derive_materialized_graph("evidence", &canonical(&chunks), &config, &[], options)
            .await
            .unwrap();
    assert_eq!(projection, normalized);
    assert_eq!(projection.facts.len(), 3);
    let mut noncanonical = projection.clone();
    noncanonical.chunks[0].text = noncanonical.chunks[0].text.nfd().collect();
    assert!(
        noncanonical
            .validate()
            .unwrap_err()
            .to_string()
            .contains("canonical NFC")
    );
    assert_eq!(
        projection.semantic_facts().unwrap(),
        derive_fact_rows(
            &chunks,
            &config,
            &[],
            DerivationOptions {
                max_fact_count: 20,
                yield_every_chunks: 1
            }
        )
        .await
        .unwrap()
    );
    let backend = NativeBackend::new();
    ingest_chunks(&backend, "evidence", &config, &chunks, &[])
        .await
        .unwrap();
    for chunk in &projection.chunks {
        let stored = backend
            .get_document("chunks", &chunk.key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored["text"], chunk.text);
        assert_eq!(stored["content_hash"], chunk.content_hash);
    }
    for fact in &projection.facts {
        let stored = backend
            .get_document("facts", &fact.key)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored["trigger_start"], fact.trigger_start);
        assert_eq!(stored["trigger_end"], fact.trigger_end);
        assert_eq!(stored["trigger"], fact.trigger);
    }
}

#[tokio::test]
async fn stale_custom_grounder_spans_fail_without_replacing_valid_evidence() {
    let backend = NativeBackend::new();
    let chunks = chunks();
    let config = config();
    ingest_chunks(&backend, "evidence", &config, &canonical(&chunks), &[])
        .await
        .unwrap();
    let before = backend.export_snapshot().await.unwrap();
    let stale = ground_chunk(&chunks[0].id, &chunks[0].text, &config, &[]);
    let error = cognigraph_construct::ingest::ingest_chunks_grounded_by(
        &backend,
        "evidence",
        &config,
        &chunks[..1],
        &|chunk| {
            assert!(unicode_normalization::is_nfc(&chunk.text));
            stale.clone()
        },
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("evidence span"), "{error}");
    assert_eq!(backend.export_snapshot().await.unwrap(), before);
}

#[tokio::test]
async fn explicit_reingestion_repairs_legacy_hashes_and_spans_without_stale_occurrences() {
    let original = NativeBackend::new();
    let chunks = chunks();
    let config = config();
    ingest_chunks(&original, "evidence", &config, &canonical(&chunks), &[])
        .await
        .unwrap();
    let expected = evidence(&original).await;
    let mut legacy = original.export_snapshot().await.unwrap();
    let chunk = &mut legacy["collections"]["chunks"]["documents"]["evidence-c0"];
    let digest: String = Sha256::digest(chunks[0].text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    chunk["content_hash"] = json!(format!("sha256:{digest}"));
    let facts = legacy["collections"]["facts"]["documents"]
        .as_object_mut()
        .unwrap();
    let key = facts
        .iter()
        .find(|(_, f)| f["evidence_chunk_id"] == "c0")
        .unwrap()
        .0
        .clone();
    let mut stale = facts.remove(&key).unwrap();
    stale["_key"] = json!("legacy-raw-offsets");
    stale["_id"] = json!("facts/legacy-raw-offsets");
    for field in ["trigger_start", "trigger_end"] {
        stale[field] = json!(stale[field].as_u64().unwrap() + 1);
    }
    facts.insert("legacy-raw-offsets".into(), stale);
    let restored = NativeBackend::new();
    restored.import_snapshot(&legacy).await.unwrap();
    assert_eq!(
        ingest_chunks(&restored, "evidence", &config, &chunks[..1], &[])
            .await
            .unwrap(),
        1
    );
    assert_eq!(evidence(&restored).await, expected);
    assert!(
        restored
            .get_document("facts", "legacy-raw-offsets")
            .await
            .unwrap()
            .is_none()
    );
}
