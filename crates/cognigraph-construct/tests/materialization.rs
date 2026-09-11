use cognigraph_construct::{
    Chunk, DerivationOptions, MaterializationError, MaterializationOptions, SpaceType,
    derive_fact_rows, derive_materialized_graph, ingest_chunks,
};
use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

fn space(value: Value) -> SpaceType {
    serde_json::from_value(value).expect("test space type must deserialize")
}

fn config() -> SpaceType {
    space(json!({
        "id": "materialized",
        "entities": [
            {"name": "Beta", "type": "product", "aliases": ["Beta Cloud"]},
            {"name": "Alpha", "type": "company", "aliases": ["Alpha Corp"]}
        ],
        "relation_rules": [{
            "source": "Alpha",
            "relation": "CONNECTS",
            "target": "Beta",
            "when_any": ["connects"],
            "trigger_provenance": {
                "connects": {"neuron_id": "neuron-7", "reviewed_by": "alice@example.test"}
            }
        }]
    }))
}

fn chunks() -> Vec<Chunk> {
    vec![
        Chunk {
            id: "chunk z".into(),
            title: "Zed".into(),
            text: "Alpha Corp connects Beta Cloud.".into(),
        },
        Chunk {
            id: "chunk-a".into(),
            title: "Alpha".into(),
            text: "Alpha connects Beta.".into(),
        },
    ]
}

fn options() -> MaterializationOptions {
    MaterializationOptions {
        max_entity_count: 10,
        max_chunk_count: 10,
        max_mention_count: 20,
        max_fact_occurrence_count: 20,
        max_semantic_fact_count: 20,
        yield_every_chunks: 1,
    }
}

fn strip_backend_metadata(mut rows: Vec<Value>) -> Vec<Value> {
    for row in &mut rows {
        let object = row.as_object_mut().expect("stored row is an object");
        for field in ["_id", "created_at", "updated_at", "confidence"] {
            object.remove(field);
        }
    }
    rows.sort_by(|left, right| left["_key"].as_str().cmp(&right["_key"].as_str()));
    rows
}

fn serialized_rows<T: serde::Serialize>(rows: &[T]) -> Vec<Value> {
    rows.iter()
        .map(|row| serde_json::to_value(row).expect("projection row serializes"))
        .collect()
}

fn assert_invalid_projection(
    projection: &cognigraph_construct::MaterializedGraphProjection,
    case: &str,
) {
    assert!(
        projection.validate().is_err(),
        "forged projection case `{case}` passed closed validation"
    );
}

#[tokio::test]
async fn projection_is_sorted_deterministic_and_matches_fact_derivation() {
    let config = config();
    let chunks = chunks();
    let projection = derive_materialized_graph("target-space", &chunks, &config, &[], options())
        .await
        .unwrap();
    let expected_semantic = derive_fact_rows(
        &chunks,
        &config,
        &[],
        DerivationOptions {
            max_fact_count: 20,
            yield_every_chunks: 1,
        },
    )
    .await
    .unwrap();

    assert_eq!(projection.semantic_facts().unwrap(), expected_semantic);
    assert_eq!(projection.entities.len(), 2);
    assert_eq!(projection.chunks.len(), 2);
    assert_eq!(projection.mentions.len(), 4);
    assert_eq!(projection.facts.len(), 2);
    assert!(
        projection
            .entities
            .windows(2)
            .all(|rows| rows[0].key < rows[1].key)
    );
    assert!(
        projection
            .chunks
            .windows(2)
            .all(|rows| rows[0].key < rows[1].key)
    );
    assert!(
        projection
            .mentions
            .windows(2)
            .all(|rows| rows[0].key < rows[1].key)
    );
    assert!(
        projection
            .facts
            .windows(2)
            .all(|rows| rows[0].key < rows[1].key)
    );
    assert!(projection.facts.iter().all(|fact| {
        fact.neuron_id.as_deref() == Some("neuron-7")
            && fact.reviewed_by.as_deref() == Some("alice@example.test")
            && fact.provenance == "narrative"
            && fact.trigger == "connects"
            && fact.trigger_end > fact.trigger_start
    }));

    let mut reversed = chunks.clone();
    reversed.reverse();
    assert_eq!(
        derive_materialized_graph("target-space", &reversed, &config, &[], options())
            .await
            .unwrap(),
        projection,
        "input chunk order must not affect the complete projection"
    );
}

#[tokio::test]
async fn rows_serialize_as_the_existing_logical_ingest_documents() {
    let backend = NativeBackend::new();
    let config = config();
    let chunks = chunks();
    let projection = derive_materialized_graph("target-space", &chunks, &config, &[], options())
        .await
        .unwrap();

    ingest_chunks(&backend, "target-space", &config, &chunks, &[])
        .await
        .unwrap();

    for (collection, projected) in [
        ("entities", serialized_rows(&projection.entities)),
        ("chunks", serialized_rows(&projection.chunks)),
        ("mentions", serialized_rows(&projection.mentions)),
        ("facts", serialized_rows(&projection.facts)),
    ] {
        assert_eq!(
            projected,
            strip_backend_metadata(
                backend
                    .list_documents(collection, None, None)
                    .await
                    .unwrap()
            ),
            "{collection} projection must be the exact pre-stamp ingest payload"
        );
    }
}

#[tokio::test]
async fn closed_projection_validation_rejects_forged_rows_and_links() {
    let original = derive_materialized_graph("target-space", &chunks(), &config(), &[], options())
        .await
        .unwrap();
    original.validate().unwrap();

    let mut forged = original.clone();
    forged.entities[0].key.push_str("-forged");
    assert_invalid_projection(&forged, "entity key");

    let mut forged = original.clone();
    forged.chunks.swap(0, 1);
    assert_invalid_projection(&forged, "unsorted chunk keys");

    let mut forged = original.clone();
    forged.chunks[0].space_id = "other-space".into();
    assert_invalid_projection(&forged, "chunk space");

    let mut forged = original.clone();
    forged.chunks[0].content_hash = "sha256:forged".into();
    assert_invalid_projection(&forged, "chunk content hash");

    let mut forged = original.clone();
    forged.chunks[0].construction_schema = "future-schema".into();
    assert_invalid_projection(&forged, "chunk construction schema");

    let mut forged = original.clone();
    forged.mentions.pop();
    assert_invalid_projection(&forged, "omitted mention");

    let mut forged = original.clone();
    forged.mentions[0].to = "entities/not-the-mentioned-entity".into();
    assert_invalid_projection(&forged, "mention endpoint");

    let mut forged = original.clone();
    forged.facts[0].space_id = "other-space".into();
    assert_invalid_projection(&forged, "fact space");

    let mut forged = original.clone();
    forged.facts[0].from = "entities/missing-source".into();
    assert_invalid_projection(&forged, "fact source endpoint");

    let mut forged = original.clone();
    forged.facts[0].evidence_chunk_id = "missing-chunk".into();
    assert_invalid_projection(&forged, "fact evidence chunk");

    let mut forged = original.clone();
    forged.facts[0].trigger_start += 1;
    assert_invalid_projection(&forged, "fact trigger span");

    let mut forged = original.clone();
    forged.facts[0].key = "fact-forged".into();
    assert_invalid_projection(&forged, "fact occurrence key");

    let mut forged = original.clone();
    forged.facts[0].construction_schema = "future-schema".into();
    assert_invalid_projection(&forged, "fact construction schema");

    let mut forged = original.clone();
    forged.facts[0].provenance = "unverified".into();
    assert_invalid_projection(&forged, "fact provenance");

    let mut forged = original;
    forged.facts[0].neuron_id = None;
    forged.facts[0].reviewed_by = Some("forged-reviewer".into());
    assert_invalid_projection(&forged, "review without licensing neuron");
}

#[tokio::test]
async fn full_occurrences_remain_distinct_while_semantic_facts_collapse() {
    let config = space(json!({
        "id": "occurrences",
        "entities": [
            {"name": "Alpha", "type": "company"},
            {"name": "Beta", "type": "product"}
        ],
        "relation_rules": [
            {"source": "Alpha", "relation": "CONNECTS", "target": "Beta", "when_any": ["first link"]},
            {"source": "Alpha", "relation": "CONNECTS", "target": "Beta", "when_any": ["second link"]}
        ]
    }));
    let chunks = vec![Chunk {
        id: "c1".into(),
        title: String::new(),
        text: "Alpha made a first link to Beta and then a second link.".into(),
    }];

    let projection = derive_materialized_graph("target", &chunks, &config, &[], options())
        .await
        .unwrap();
    assert_eq!(projection.facts.len(), 2);
    assert_eq!(projection.semantic_facts().unwrap().len(), 1);
    assert_eq!(
        projection.semantic_facts().unwrap(),
        derive_fact_rows(
            &chunks,
            &config,
            &[],
            DerivationOptions {
                max_fact_count: 1,
                yield_every_chunks: 1,
            },
        )
        .await
        .unwrap()
    );

    let mut bounded = options();
    bounded.max_fact_occurrence_count = 1;
    assert_eq!(
        derive_materialized_graph("target", &chunks, &config, &[], bounded).await,
        Err(MaterializationError::FactOccurrenceLimitExceeded {
            max_fact_occurrence_count: 1
        })
    );
}

#[tokio::test]
async fn malformed_keys_and_missing_entity_endpoints_fail_closed() {
    let mut colliding_entities = config();
    colliding_entities.entities.push(
        serde_json::from_value(json!({
            "name": "Alpha!",
            "type": "different",
            "aliases": []
        }))
        .unwrap(),
    );
    assert_eq!(
        derive_materialized_graph("target", &chunks(), &colliding_entities, &[], options()).await,
        Err(MaterializationError::EntityKeyCollision {
            key: "alpha".into()
        })
    );

    let colliding_chunks = vec![
        Chunk {
            id: "same id".into(),
            title: String::new(),
            text: "Alpha connects Beta.".into(),
        },
        Chunk {
            id: "same-id".into(),
            title: String::new(),
            text: "Alpha connects Beta.".into(),
        },
    ];
    assert!(matches!(
        derive_materialized_graph("target", &colliding_chunks, &config(), &[], options()).await,
        Err(MaterializationError::ChunkKeyCollision { .. })
    ));

    let dangling = space(json!({
        "id": "dangling",
        "entities": [],
        "relation_rules": [{
            "source": "Alpha", "relation": "CONNECTS", "target": "Beta",
            "when_any": ["connects"]
        }]
    }));
    assert_eq!(
        derive_materialized_graph(
            "target",
            &[Chunk {
                id: "c1".into(),
                title: String::new(),
                text: "connects".into(),
            }],
            &dangling,
            &[],
            options()
        )
        .await,
        Err(MaterializationError::MissingEntityDefinition {
            name: "Alpha".into()
        })
    );
}

#[tokio::test]
async fn every_output_class_and_the_yield_interval_are_bounded() {
    let config = config();
    let chunks = chunks();

    let mut bounded = options();
    bounded.max_entity_count = 1;
    assert_eq!(
        derive_materialized_graph("target", &chunks, &config, &[], bounded).await,
        Err(MaterializationError::EntityLimitExceeded {
            max_entity_count: 1
        })
    );

    bounded = options();
    bounded.max_chunk_count = 1;
    assert_eq!(
        derive_materialized_graph("target", &chunks, &config, &[], bounded).await,
        Err(MaterializationError::ChunkLimitExceeded { max_chunk_count: 1 })
    );

    bounded = options();
    bounded.max_mention_count = 1;
    assert_eq!(
        derive_materialized_graph("target", &chunks, &config, &[], bounded).await,
        Err(MaterializationError::MentionLimitExceeded {
            max_mention_count: 1
        })
    );

    bounded = options();
    bounded.max_semantic_fact_count = 1;
    assert_eq!(
        derive_materialized_graph("target", &chunks, &config, &[], bounded).await,
        Err(MaterializationError::SemanticFactLimitExceeded {
            max_semantic_fact_count: 1
        })
    );

    bounded = options();
    bounded.yield_every_chunks = 0;
    assert_eq!(
        derive_materialized_graph("target", &chunks, &config, &[], bounded).await,
        Err(MaterializationError::InvalidYieldInterval)
    );
}
