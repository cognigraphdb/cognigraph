use cognigraph_construct::{
    Chunk, DerivationError, DerivationOptions, DerivedFactRow, SpaceType, VetoRule,
    derive_fact_rows,
};

fn options(max_fact_count: usize) -> DerivationOptions {
    DerivationOptions {
        max_fact_count,
        yield_every_chunks: 1,
    }
}

fn space(value: serde_json::Value) -> SpaceType {
    serde_json::from_value(value).expect("test space type must deserialize")
}

#[tokio::test]
async fn rows_are_sorted_unique_and_keep_distinct_evidence_chunks() {
    let config = space(serde_json::json!({
        "id": "derivation-ordering",
        "entities": [],
        "relation_rules": [
            {
                "source": "Zulu",
                "relation": "USES",
                "target": "Tool",
                "when_any": ["uses tool"]
            },
            {
                "source": "Alpha",
                "relation": "BUILDS",
                "target": "Graph",
                "when_any": ["builds graph"]
            },
            {
                "source": "Alpha",
                "relation": "BUILDS",
                "target": "Graph",
                "when_any": ["builds graph"]
            }
        ]
    }));
    let chunks = vec![
        Chunk {
            id: "chunk-z".into(),
            title: String::new(),
            text: "Zulu uses tool. Alpha builds graph, and later builds graph again.".into(),
        },
        Chunk {
            id: "chunk-a".into(),
            title: String::new(),
            text: "Alpha builds graph.".into(),
        },
    ];

    let rows = derive_fact_rows(&chunks, &config, &[], options(10))
        .await
        .unwrap();

    assert_eq!(
        rows,
        vec![
            DerivedFactRow {
                source: "Alpha".into(),
                relation: "BUILDS".into(),
                target: "Graph".into(),
                evidence_chunk_id: "chunk-a".into(),
            },
            DerivedFactRow {
                source: "Alpha".into(),
                relation: "BUILDS".into(),
                target: "Graph".into(),
                evidence_chunk_id: "chunk-z".into(),
            },
            DerivedFactRow {
                source: "Zulu".into(),
                relation: "USES".into(),
                target: "Tool".into(),
                evidence_chunk_id: "chunk-z".into(),
            },
        ]
    );

    let mut reversed_chunks = chunks.clone();
    reversed_chunks.reverse();
    assert_eq!(
        derive_fact_rows(&reversed_chunks, &config, &[], options(10))
            .await
            .unwrap(),
        rows,
        "input chunk order must not alter the derived rows"
    );
}

#[tokio::test]
async fn derivation_preserves_negation_gate_template_and_veto_semantics() {
    let config = space(serde_json::json!({
        "id": "derivation-grounding-parity",
        "entities": [
            {"name": "Acme", "type": "org", "aliases": ["Acme Corp"]},
            {"name": "Nimbus", "type": "platform", "aliases": ["Nimbus Cloud"]}
        ],
        "relation_rules": [
            {
                "source": "Acme",
                "relation": "ACQUIRED",
                "target": "Nimbus",
                "when_any": ["{source} acquired {target}"],
                "require_in_sentence": ["source", "target"]
            },
            {
                "source": "Acme",
                "relation": "SELECTED",
                "target": "Nimbus",
                "when_any": ["selected the platform"],
                "require_in_sentence": ["source", "target"]
            }
        ]
    }));
    let vetoes = vec![VetoRule {
        source: "Acme".into(),
        relation: "ACQUIRED".into(),
        target: "Nimbus".into(),
        when_any: vec!["unconfirmed rumor".into()],
    }];
    let chunks = vec![
        Chunk {
            id: "template-affirmed".into(),
            title: String::new(),
            text: "Acme Corp acquired Nimbus Cloud after review.".into(),
        },
        Chunk {
            id: "template-negated".into(),
            title: String::new(),
            text: "Acme did not acquire Nimbus.".into(),
        },
        Chunk {
            id: "gate-rejected".into(),
            title: String::new(),
            text: "Acme appeared elsewhere. Another company selected the platform Nimbus.".into(),
        },
        Chunk {
            id: "gate-affirmed".into(),
            title: String::new(),
            text: "Acme selected the platform Nimbus for production.".into(),
        },
        Chunk {
            id: "vetoed".into(),
            title: String::new(),
            text: "Acme acquired Nimbus, according to an unconfirmed rumor.".into(),
        },
    ];

    let rows = derive_fact_rows(&chunks, &config, &vetoes, options(10))
        .await
        .unwrap();

    assert_eq!(
        rows,
        vec![
            DerivedFactRow {
                source: "Acme".into(),
                relation: "ACQUIRED".into(),
                target: "Nimbus".into(),
                evidence_chunk_id: "template-affirmed".into(),
            },
            DerivedFactRow {
                source: "Acme".into(),
                relation: "SELECTED".into(),
                target: "Nimbus".into(),
                evidence_chunk_id: "gate-affirmed".into(),
            },
        ]
    );
}

#[tokio::test]
async fn derivation_fails_closed_at_its_unique_fact_limit() {
    let config = space(serde_json::json!({
        "id": "derivation-limit",
        "entities": [],
        "relation_rules": [{
            "source": "A",
            "relation": "R",
            "target": "B",
            "when_any": ["grounds"]
        }]
    }));
    let chunks = vec![
        Chunk {
            id: "c1".into(),
            title: String::new(),
            text: "grounds".into(),
        },
        Chunk {
            id: "c2".into(),
            title: String::new(),
            text: "grounds".into(),
        },
    ];

    assert_eq!(
        derive_fact_rows(&chunks, &config, &[], options(1)).await,
        Err(DerivationError::FactLimitExceeded { max_fact_count: 1 })
    );
    assert_eq!(
        derive_fact_rows(
            &chunks,
            &config,
            &[],
            DerivationOptions {
                max_fact_count: 2,
                yield_every_chunks: 0,
            },
        )
        .await,
        Err(DerivationError::InvalidYieldInterval)
    );
}
