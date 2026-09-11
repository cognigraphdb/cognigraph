use cognigraph_construct::ingest::chunk_key;
use cognigraph_construct::{
    Chunk, EntityDef, EvalQuestion, EvalSpec, RelationRule, SpaceType, evaluate, ingest_chunks,
};
use cognigraph_core::{Direction, GraphBackend};
use cognigraph_native::NativeBackend;

fn space() -> SpaceType {
    SpaceType {
        id: "test".into(),
        name: "test".into(),
        version: 1,
        description: String::new(),
        entities: ["Alpha", "Beta", "Gamma"]
            .into_iter()
            .map(|name| EntityDef {
                name: name.into(),
                entity_type: "test".into(),
                aliases: Vec::new(),
            })
            .collect(),
        relation_rules: vec![RelationRule {
            source: "Alpha".into(),
            relation: "LINKS".into(),
            target: "Beta".into(),
            when_any: vec!["links beta".into()],
            require_in_sentence: Vec::new(),
            trigger_provenance: Default::default(),
        }],
    }
}

fn chunk(id: &str, text: &str) -> Chunk {
    Chunk {
        id: id.into(),
        title: id.into(),
        text: text.into(),
    }
}

fn eval(space_id: &str) -> EvalSpec {
    EvalSpec {
        space_id: space_id.into(),
        questions: vec![EvalQuestion {
            id: "q1".into(),
            question: String::new(),
            expected_facts: vec!["Alpha --LINKS--> Beta".into()],
            forbidden_facts: Vec::new(),
        }],
    }
}

#[tokio::test]
async fn evidence_occurrences_survive_independent_chunk_and_space_revisions() {
    let backend = NativeBackend::new();
    let config = space();

    ingest_chunks(
        &backend,
        "space-a",
        &config,
        &[
            chunk("c1", "Alpha links Beta."),
            chunk("c2", "Alpha links Beta."),
        ],
        &[],
    )
    .await
    .unwrap();
    ingest_chunks(
        &backend,
        "space-b",
        &config,
        &[chunk("c1", "Alpha links Beta.")],
        &[],
    )
    .await
    .unwrap();

    let facts = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(facts.len(), 3, "one edge per evidence occurrence");
    let keys: std::collections::HashSet<_> = facts
        .iter()
        .map(|fact| fact["_key"].as_str().unwrap())
        .collect();
    assert_eq!(keys.len(), 3, "occurrence keys include space and chunk");
    assert!(
        evaluate(&backend, "space-a", &eval("space-a"))
            .await
            .unwrap()
            .recall_ok()
    );
    assert!(
        evaluate(&backend, "space-b", &eval("space-b"))
            .await
            .unwrap()
            .recall_ok()
    );

    let grounded = ingest_chunks(
        &backend,
        "space-a",
        &config,
        &[chunk("c1", "Gamma is present alone.")],
        &[],
    )
    .await
    .unwrap();
    assert_eq!(grounded, 0);

    let stored = backend
        .get_document("chunks", &chunk_key("space-a", "c1"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["text"], "Gamma is present alone.");

    let revised_vertex = format!("chunks/{}", chunk_key("space-a", "c1"));
    let mentions = backend
        .get_edges("mentions", &revised_vertex, Direction::Outbound)
        .await
        .unwrap();
    assert_eq!(mentions.len(), 1, "stale mentions were removed");
    assert_eq!(mentions[0]["_to"], "entities/gamma");

    let facts = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(facts.len(), 2, "only the revised occurrence was retracted");
    assert!(
        facts
            .iter()
            .any(|fact| { fact["space_id"] == "space-a" && fact["evidence_chunk_id"] == "c2" })
    );
    assert!(
        facts
            .iter()
            .any(|fact| { fact["space_id"] == "space-b" && fact["evidence_chunk_id"] == "c1" })
    );
    assert!(
        evaluate(&backend, "space-a", &eval("space-a"))
            .await
            .unwrap()
            .recall_ok(),
        "canonical fact remains grounded by c2"
    );
    assert!(
        evaluate(&backend, "space-b", &eval("space-b"))
            .await
            .unwrap()
            .recall_ok()
    );
}

#[tokio::test]
async fn duplicate_chunk_keys_fail_before_construction_writes() {
    let backend = NativeBackend::new();
    let err = ingest_chunks(
        &backend,
        "space-a",
        &space(),
        &[
            chunk("same id", "Alpha links Beta."),
            chunk("same-id", "Gamma."),
        ],
        &[],
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("colliding chunk id"));
    assert!(
        backend
            .get_document("chunks", &chunk_key("space-a", "same id"))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn chunk_key_collisions_across_requests_and_spaces_fail_closed() {
    let backend = NativeBackend::new();
    let config = space();
    ingest_chunks(
        &backend,
        "space a",
        &config,
        &[chunk("same id", "Alpha links Beta.")],
        &[],
    )
    .await
    .unwrap();

    for (space_id, chunk_id) in [("space a", "same-id"), ("space-a", "same id")] {
        let err = ingest_chunks(
            &backend,
            space_id,
            &config,
            &[chunk(chunk_id, "Gamma only.")],
            &[],
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("chunk key collision"), "{err}");
    }

    let original = backend
        .get_document("chunks", &chunk_key("space a", "same id"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original["space_id"], "space a");
    assert_eq!(original["chunk_id"], "same id");
    assert_eq!(original["text"], "Alpha links Beta.");
    let facts = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["space_id"], "space a");
    assert_eq!(facts[0]["evidence_chunk_id"], "same id");
}

#[tokio::test]
async fn entity_key_collisions_never_merge_distinct_identities() {
    let backend = NativeBackend::new();
    let mut config = space();
    config.entities.push(EntityDef {
        name: "Alpha!".into(),
        entity_type: "other".into(),
        aliases: Vec::new(),
    });
    let err = ingest_chunks(
        &backend,
        "space-a",
        &config,
        &[chunk("c1", "Alpha links Beta.")],
        &[],
    )
    .await
    .unwrap_err();
    assert!(
        err.to_string().contains("distinct entity definitions"),
        "{err}"
    );

    ingest_chunks(
        &backend,
        "space-a",
        &space(),
        &[chunk("c1", "Alpha links Beta.")],
        &[],
    )
    .await
    .unwrap();
    let mut colliding = space();
    colliding.entities[0].name = "ALPHA".into();
    let err = ingest_chunks(
        &backend,
        "space-b",
        &colliding,
        &[chunk("c1", "ALPHA links Beta.")],
        &[],
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("entity key collision"), "{err}");
}

#[tokio::test]
async fn concurrent_revisions_leave_one_coherent_chunk_state() {
    let backend = std::sync::Arc::new(NativeBackend::new());
    let config = std::sync::Arc::new(space());
    let first = {
        let backend = backend.clone();
        let config = config.clone();
        tokio::spawn(async move {
            ingest_chunks(
                backend.as_ref(),
                "space-a",
                config.as_ref(),
                &[chunk("c1", "Alpha links Beta.")],
                &[],
            )
            .await
        })
    };
    let second = {
        let backend = backend.clone();
        let config = config.clone();
        tokio::spawn(async move {
            ingest_chunks(
                backend.as_ref(),
                "space-a",
                config.as_ref(),
                &[chunk("c1", "Gamma only.")],
                &[],
            )
            .await
        })
    };
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();

    let stored = backend
        .get_document("chunks", &chunk_key("space-a", "c1"))
        .await
        .unwrap()
        .unwrap();
    let facts = backend.list_documents("facts", None, None).await.unwrap();
    match stored["text"].as_str().unwrap() {
        "Alpha links Beta." => assert_eq!(facts.len(), 1),
        "Gamma only." => assert!(facts.is_empty()),
        other => panic!("unexpected chunk state: {other}"),
    }
}

#[tokio::test]
async fn reingest_retracts_matching_legacy_fact_edges() {
    let backend = NativeBackend::new();
    let config = space();
    ingest_chunks(
        &backend,
        "space-a",
        &config,
        &[chunk("c1", "Alpha links Beta.")],
        &[],
    )
    .await
    .unwrap();

    let current = backend.list_documents("facts", None, None).await.unwrap();
    backend
        .delete_document("facts", current[0]["_key"].as_str().unwrap())
        .await
        .unwrap();
    backend
        .create_edge(
            "facts",
            serde_json::json!({
                "_key": "legacy-upserted-fact",
                "_from": "entities/alpha",
                "_to": "entities/beta",
                "relation_type": "LINKS",
                "space_id": "space-a",
                "evidence_chunk_id": "c1",
                "trigger": "links beta",
            }),
        )
        .await
        .unwrap();

    ingest_chunks(
        &backend,
        "space-a",
        &config,
        &[chunk("c1", "Gamma only.")],
        &[],
    )
    .await
    .unwrap();

    assert!(
        backend
            .list_documents("facts", None, None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn arango_is_rejected_before_any_preparatory_write() {
    let backend = cognigraph_arango::ArangoBackend::connect(
        "http://127.0.0.1:1",
        "unreachable",
        "unused",
        "unused",
    );

    let err = ingest_chunks(
        &backend,
        "space-a",
        &space(),
        &[chunk("c1", "Alpha links Beta.")],
        &[],
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("requires atomic batch support"));
    assert!(err.to_string().contains("backend `arango`"));
}

/// A space whose trigger does NOT name the target, so a licensing sentence can
/// legitimately omit it — the shape the semantics detector exists to catch.
fn semantics_space() -> SpaceType {
    SpaceType {
        relation_rules: vec![RelationRule {
            source: "Alpha".into(),
            relation: "LINKS".into(),
            target: "Gamma".into(),
            when_any: vec!["alpha connects onward".into(), "alpha links gamma".into()],
            require_in_sentence: Vec::new(),
            trigger_provenance: Default::default(),
        }],
        ..space()
    }
}

/// The relation-semantics verdict is written as a QUARANTINED SIDECAR keyed by
/// the fact's own key — never as fields on the attested fact edge, so improving
/// the detector can never invalidate a signed M26 projection
/// (decision_pilot_clinical_graph.md, D2).
#[tokio::test]
async fn semantics_verdicts_land_in_the_sidecar_and_never_on_the_fact_edge() {
    let backend = NativeBackend::new();
    // The trigger fires, but its sentence names neither the target nor anything
    // resembling the relation: two signals.
    let suspect = chunk("c1", "Alpha connects onward. Gamma is described far below.");
    ingest_chunks(&backend, "s1", &semantics_space(), &[suspect], &[])
        .await
        .unwrap();

    let facts = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(facts.len(), 1, "{facts:?}");
    let fact = &facts[0];
    assert!(
        fact.get("suspect").is_none() && fact.get("signals").is_none(),
        "the attested fact record must stay frozen: {fact}"
    );

    let rows = backend
        .list_documents(cognigraph_construct::FACT_SEMANTICS_COLLECTION, None, None)
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "one verdict for the one suspect fact: {rows:?}"
    );
    let row = &rows[0];
    assert_eq!(
        row.get("_key").and_then(|v| v.as_str()),
        fact.get("_key").and_then(|v| v.as_str()),
        "keyed by the fact key so the join is trivial"
    );
    assert_eq!(row.get("suspect").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        row.get("detector_rev").and_then(|v| v.as_str()),
        Some(cognigraph_construct::SEMANTICS_REV)
    );
    assert_eq!(
        row.get("signals").unwrap().as_array().unwrap().len(),
        2,
        "target-absent and relation-vocabulary-absent both fire: {row}"
    );
}

/// A well-evidenced fact gets NO row: the sidecar is the review queue, so for an
/// ingested space "absent" means "analyzed and clean", and a re-ingest must not
/// leave a stale verdict behind once the evidence improves.
#[tokio::test]
async fn a_clean_fact_has_no_verdict_and_reingest_clears_a_stale_one() {
    let backend = NativeBackend::new();
    let suspect = chunk("c1", "Alpha connects onward. Gamma is described far below.");
    ingest_chunks(&backend, "s1", &semantics_space(), &[suspect], &[])
        .await
        .unwrap();
    assert_eq!(
        backend
            .list_documents(cognigraph_construct::FACT_SEMANTICS_COLLECTION, None, None)
            .await
            .unwrap()
            .len(),
        1
    );

    // Same chunk id, better evidence: the sentence now names Gamma AND uses the
    // relation's own vocabulary, so no signal fires.
    let clean = chunk("c1", "Alpha links Gamma in the registry.");
    ingest_chunks(&backend, "s1", &semantics_space(), &[clean], &[])
        .await
        .unwrap();

    assert_eq!(
        backend
            .list_documents("facts", None, None)
            .await
            .unwrap()
            .len(),
        1,
        "still one fact"
    );
    assert!(
        backend
            .list_documents(cognigraph_construct::FACT_SEMANTICS_COLLECTION, None, None)
            .await
            .unwrap()
            .is_empty(),
        "the stale verdict must not outlive the evidence that produced it"
    );
}
