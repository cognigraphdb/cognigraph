//! Directed construction (D12): the model nominates, the gates decide.

use async_trait::async_trait;
use cognigraph_construct::{Chunk, DirectedRelation, directed_ingest};
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};

/// Returns a fixed proposal set, whatever the prompt.
struct CannedProvider(Value);

#[async_trait]
impl CompletionProvider for CannedProvider {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        _schema: &Value,
    ) -> anyhow::Result<Value> {
        Ok(self.0.clone())
    }
    fn model_name(&self) -> &str {
        "canned"
    }
}

fn taxonomy() -> Vec<DirectedRelation> {
    vec![
        DirectedRelation {
            relation: "GRANTS_EXCLUSIVITY".into(),
            description: "one party grants another exclusive rights".into(),
            require_in_sentence: vec!["exclusiv".into()],
        },
        DirectedRelation {
            relation: "GOVERNED_BY".into(),
            description: "the agreement names its governing law".into(),
            require_in_sentence: vec!["governed by".into(), "laws of".into()],
        },
    ]
}

fn chunks() -> Vec<Chunk> {
    vec![
        Chunk {
            id: "doc::0".into(),
            title: "doc".into(),
            text: "Producer grants ConvergTV the exclusive right to distribute the Programs. \
                   Nothing here concerns law."
                .into(),
        },
        Chunk {
            id: "doc::1".into(),
            title: "doc".into(),
            text: "This Agreement shall be governed by the laws of the State of Delaware. \
                   The parties waive nothing."
                .into(),
        },
    ]
}

#[tokio::test]
async fn grounds_good_proposals_and_rejects_each_gate_violation() {
    let backend = NativeBackend::new();
    let provider = CannedProvider(json!({ "facts": [
        // Good: verbatim evidence, endpoints in sentence, vocabulary affirmed.
        { "source": "Producer", "source_type": "party",
          "target": "ConvergTV", "target_type": "party",
          "relation": "GRANTS_EXCLUSIVITY",
          "evidence": "Producer grants ConvergTV the exclusive right to distribute the Programs.",
          "chunk_id": "doc::0" },
        // Good: governing law, whitespace-mangled quote still matches loosely.
        { "source": "This Agreement", "source_type": "agreement",
          "target": "State of Delaware", "target_type": "jurisdiction",
          "relation": "GOVERNED_BY",
          "evidence": "This  Agreement shall be governed by the laws of the State of  Delaware.",
          "chunk_id": "doc::1" },
        // Hallucinated relation.
        { "source": "Producer", "source_type": "party",
          "target": "ConvergTV", "target_type": "party",
          "relation": "OWES_MONEY",
          "evidence": "Producer grants ConvergTV the exclusive right to distribute the Programs.",
          "chunk_id": "doc::0" },
        // Fabricated evidence.
        { "source": "Producer", "source_type": "party",
          "target": "ConvergTV", "target_type": "party",
          "relation": "GRANTS_EXCLUSIVITY",
          "evidence": "Producer promises ConvergTV exclusive worldwide dominion.",
          "chunk_id": "doc::0" },
        // Endpoint not in the evidence sentence.
        { "source": "Distributor", "source_type": "party",
          "target": "ConvergTV", "target_type": "party",
          "relation": "GRANTS_EXCLUSIVITY",
          "evidence": "Producer grants ConvergTV the exclusive right to distribute the Programs.",
          "chunk_id": "doc::0" },
        // Vocabulary missing from the cited sentence.
        { "source": "The parties", "source_type": "party",
          "target": "nothing", "target_type": "term",
          "relation": "GOVERNED_BY",
          "evidence": "The parties waive nothing.",
          "chunk_id": "doc::1" },
        // Unknown chunk.
        { "source": "Producer", "source_type": "party",
          "target": "ConvergTV", "target_type": "party",
          "relation": "GRANTS_EXCLUSIVITY",
          "evidence": "Producer grants ConvergTV the exclusive right to distribute the Programs.",
          "chunk_id": "other::9" },
    ]}));

    let outcome = directed_ingest(&backend, "d12_test", &taxonomy(), &chunks(), &provider)
        .await
        .unwrap();

    assert_eq!(outcome.proposed, 7);
    assert_eq!(outcome.facts_grounded, 2, "skips were: {:?}", outcome.skips);
    assert_eq!(outcome.skips.len(), 5);
    assert!(
        outcome
            .extracted_by
            .starts_with("directed:canned@directed-policy-v2")
    );

    // The written rows are ordinary occurrence-v1 facts with recovered
    // offsets into the ORIGINAL chunk text.
    let facts = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(facts.len(), 2);
    let exclusivity = facts
        .iter()
        .find(|f| f["relation_type"] == "GRANTS_EXCLUSIVITY")
        .unwrap();
    assert_eq!(exclusivity["space_id"], "d12_test");
    assert_eq!(exclusivity["construction_schema"], "occurrence-v1");
    assert_eq!(
        exclusivity["reviewed_by"],
        "directed:canned@directed-policy-v2"
    );
    let start = exclusivity["trigger_start"].as_u64().unwrap() as usize;
    let end = exclusivity["trigger_end"].as_u64().unwrap() as usize;
    let source_text = &chunks()[0].text[start..end];
    assert_eq!(exclusivity["trigger"], source_text);

    let governed = facts
        .iter()
        .find(|f| f["relation_type"] == "GOVERNED_BY")
        .unwrap();
    // The mangled quote grounded against the REAL text of chunk 1.
    assert_eq!(governed["evidence_chunk_id"], "doc::1");
}

#[tokio::test]
async fn reingest_is_idempotent_per_chunk() {
    let backend = NativeBackend::new();
    let provider = CannedProvider(json!({ "facts": [
        { "source": "Producer", "source_type": "party",
          "target": "ConvergTV", "target_type": "party",
          "relation": "GRANTS_EXCLUSIVITY",
          "evidence": "Producer grants ConvergTV the exclusive right to distribute the Programs.",
          "chunk_id": "doc::0" },
    ]}));
    for _ in 0..2 {
        let outcome = directed_ingest(&backend, "d12_idem", &taxonomy(), &chunks(), &provider)
            .await
            .unwrap();
        assert_eq!(outcome.facts_grounded, 1);
    }
    let facts = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(
        facts.len(),
        1,
        "delete-and-rebuild must hold for directed facts"
    );
}

#[tokio::test]
async fn taxonomy_without_vocabulary_is_refused() {
    let backend = NativeBackend::new();
    let provider = CannedProvider(json!({ "facts": [] }));
    let bare = vec![DirectedRelation {
        relation: "ANYTHING".into(),
        description: "no gate".into(),
        require_in_sentence: vec!["  ".into()],
    }];
    let err = directed_ingest(&backend, "d12_bad", &bare, &chunks(), &provider)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("restraint vocabulary"), "{err}");
}

#[tokio::test]
async fn malformed_completion_preserves_projection_but_explicit_empty_reconciles() {
    let backend = NativeBackend::new();
    let good = json!({
        "source": "Producer", "source_type": "party",
        "target": "ConvergTV", "target_type": "party",
        "relation": "GRANTS_EXCLUSIVITY",
        "evidence": "Producer grants ConvergTV the exclusive right to distribute the Programs.",
        "chunk_id": "doc::0"
    });
    directed_ingest(
        &backend,
        "preserve",
        &taxonomy(),
        &chunks(),
        &CannedProvider(json!({"facts": [good.clone()]})),
    )
    .await
    .unwrap();
    let before = backend.export_json().await.unwrap();
    let mut invalid = vec![
        Value::Null,
        json!([]),
        json!({}),
        json!({"facts": null}),
        json!({"facts": {}}),
        json!({"facts": "invalid"}),
        json!({"facts": [good.clone(), null]}),
        json!({"facts": [], "unexpected": true}),
    ];
    // Every schema-required field must be present and string-typed. One
    // malformed item invalidates the entire response, including valid peers.
    for field in good.as_object().unwrap().keys() {
        let mut missing = good.clone();
        missing.as_object_mut().unwrap().remove(field);
        invalid.push(json!({"facts": [good.clone(), missing]}));
        let mut wrong_type = good.clone();
        wrong_type[field] = json!(42);
        invalid.push(json!({"facts": [wrong_type]}));
    }
    let mut extra = good.clone();
    extra["unexpected"] = json!(true);
    invalid.push(json!({"facts": [extra]}));
    for response in invalid {
        let err = directed_ingest(
            &backend,
            "preserve",
            &taxonomy(),
            &chunks(),
            &CannedProvider(response.clone()),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("directed completion returned invalid facts"),
            "{response}: {err}"
        );
        assert_eq!(
            backend.export_json().await.unwrap(),
            before,
            "changed state for {response}"
        );
    }
    let outcome = directed_ingest(
        &backend,
        "preserve",
        &taxonomy(),
        &chunks(),
        &CannedProvider(json!({"facts": []})),
    )
    .await
    .unwrap();
    assert_eq!(outcome.proposed, 0);
    assert_eq!(outcome.facts_grounded, 0);
    assert!(
        backend
            .list_documents("facts", None, None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn symbol_only_endpoints_are_skipped_not_fatal() {
    let backend = NativeBackend::new();
    // CUAD-style redacted text: "[***]" is a legal endpoint per the
    // in-sentence gate but has no sanitizable identity. It must be SKIPPED
    // (restraint), not allowed to fail the whole atomic write.
    let chunk = vec![Chunk {
        id: "doc::0".into(),
        title: "doc".into(),
        text: "Producer grants [***] the exclusive right to distribute the Programs.".into(),
    }];
    let provider = CannedProvider(json!({ "facts": [
        { "source": "Producer", "source_type": "party",
          "target": "[***]", "target_type": "party",
          "relation": "GRANTS_EXCLUSIVITY",
          "evidence": "Producer grants [***] the exclusive right to distribute the Programs.",
          "chunk_id": "doc::0" },
    ]}));
    let outcome = directed_ingest(&backend, "d12_redact", &taxonomy(), &chunk, &provider)
        .await
        .unwrap();
    assert_eq!(outcome.facts_grounded, 0);
    assert!(
        outcome
            .skips
            .iter()
            .any(|s| s.contains("no usable identity")),
        "skips: {:?}",
        outcome.skips
    );
}
