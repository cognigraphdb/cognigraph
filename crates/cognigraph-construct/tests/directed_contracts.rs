//! Endpoint boundaries and exact request identifiers remain local gates even
//! when a completion provider ignores the request's response schema.

use async_trait::async_trait;
use cognigraph_construct::directed::{DIRECTED_POLICY, DirectedProposal, gate_directed_proposals};
use cognigraph_construct::{Chunk, DirectedRelation, directed_ingest};
use cognigraph_core::GraphBackend;
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use unicode_normalization::UnicodeNormalization;

fn taxonomy() -> Vec<DirectedRelation> {
    vec![DirectedRelation {
        relation: "OWNS".into(),
        description: "The source owns the target.".into(),
        require_in_sentence: vec!["owns".into()],
    }]
}

fn chunk(text: &str) -> Chunk {
    Chunk {
        id: "source-1".into(),
        title: "test".into(),
        text: text.into(),
    }
}

fn proposal(source: &str, target: &str, evidence: &str) -> DirectedProposal {
    DirectedProposal {
        source: source.into(),
        source_type: "entity".into(),
        target: target.into(),
        target_type: "entity".into(),
        relation: "OWNS".into(),
        evidence: evidence.into(),
        chunk_id: "source-1".into(),
    }
}

#[test]
fn both_endpoints_reject_embedded_words_and_unicode_word_continuations() {
    for (mention, endpoint) in [
        ("Joanne", "Ann"),
        ("South African", "South Africa"),
        ("ÉAnn", "Ann"),
        ("Annβ", "Ann"),
        ("Ann1", "Ann"),
        ("١Ann", "Ann"),
        ("Ann_", "Ann"),
        ("Ann\u{203f}", "Ann"), // Unicode connector punctuation
        ("Ann\u{0301}", "Ann"), // NFC changes the final letter
        ("Ann\u{20dd}", "Ann"), // enclosing mark survives NFC
        ("\u{093e}Ann", "Ann"), // spacing mark
        ("Ann\u{200d}", "Ann"), // join control
        ("東京A市", "東京A"),
    ] {
        for source_side in [true, false] {
            let (source, target, text) = if source_side {
                (endpoint, "Acme", format!("{mention} owns Acme."))
            } else {
                ("Acme", endpoint, format!("Acme owns {mention}."))
            };
            let (facts, entities, skips) = gate_directed_proposals(
                &[chunk(&text)],
                &taxonomy(),
                &[proposal(source, target, &text)],
                "test",
            );
            assert!(
                facts.is_empty() && entities.is_empty(),
                "{text}: {endpoint}"
            );
            assert_eq!(skips.len(), 1);
            let side = if source_side { "source" } else { "target" };
            assert!(
                skips[0].contains(&format!("{side} does not occur")),
                "{skips:?}"
            );
        }
    }
}

#[test]
fn endpoints_allow_punctuation_whitespace_and_later_complete_mentions() {
    for (text, source, target) in [
        ("(Ann) owns [Acme].", "Ann", "Acme"),
        ("O’Neill owns Smith-Jones.", "O’Neill", "Smith-Jones"),
        ("AT&T owns Acme, Inc.", "AT&T", "Acme, Inc."),
        ("Élodie owns Zürich.", "Élodie", "Zürich"),
        ("South   Africa owns New\nYork.", "South Africa", "New York"),
        (
            "Joanne and ANN own shares; Ann owns AcmeCorp and Acme.",
            "ann",
            "acme",
        ),
        // The boundary policy treats apostrophes/hyphens as separators.
        ("Ann's trust owns Acme-group.", "Ann", "Acme"),
    ] {
        let (facts, _, skips) = gate_directed_proposals(
            &[chunk(text)],
            &taxonomy(),
            &[proposal(source, target, text)],
            "test",
        );
        assert_eq!(facts.len(), 1, "{text}: {skips:?}");
        assert!(skips.is_empty());
        assert_eq!(facts[0].1.trigger, text);
        assert_eq!(facts[0].1.trigger_span, (0, text.len()));
    }
}

#[test]
fn quote_matching_keeps_nfc_byte_offsets_and_can_start_inside_a_word() {
    let text = "Préface. E\u{0301}lodie owns Acme.";
    // Quote lookup intentionally remains substring-based. Both complete
    // endpoints occur in the enclosing sentence despite the shortened quote.
    let quote = "lodie  owns ACME";
    let (facts, _, skips) = gate_directed_proposals(
        &[chunk(text)],
        &taxonomy(),
        &[proposal("Élodie", "Acme", quote)],
        "test",
    );
    assert_eq!(facts.len(), 1, "{skips:?}");
    let fact = &facts[0].1;
    let canonical: String = text.nfc().collect();
    let start = canonical.find("lodie").unwrap();
    assert_eq!(fact.trigger_span, (start, canonical.len() - 1));
    assert_eq!(fact.trigger, canonical[start..canonical.len() - 1]);
}

struct SchemaProvider {
    response: Value,
    chunk_ids: Value,
    relations: Value,
}

#[async_trait]
impl CompletionProvider for SchemaProvider {
    async fn complete_json(&self, _: &str, _: &str, schema: &Value) -> anyhow::Result<Value> {
        let item = &schema["properties"]["facts"]["items"];
        assert_eq!(
            item["properties"]["chunk_id"],
            json!({"type": "string", "enum": self.chunk_ids})
        );
        assert_eq!(
            item["properties"]["relation"],
            json!({"type": "string", "enum": self.relations})
        );
        for object in [schema, item] {
            assert_eq!(object["additionalProperties"], false);
            let required = object["required"].as_array().unwrap();
            let properties = object["properties"].as_object().unwrap();
            assert_eq!(required.len(), properties.len());
            assert!(properties.keys().all(|key| required.contains(&json!(key))));
        }
        Ok(self.response.clone())
    }

    fn model_name(&self) -> &str {
        "schema-ignoring-test"
    }
}

#[tokio::test]
async fn schema_binds_opaque_ids_but_gates_still_reject_inexact_identifiers() {
    let backend = NativeBackend::new();
    // Canonically equivalent ID spellings are distinct; whitespace and prompt
    // delimiters are part of an opaque identifier, not display decoration.
    let ids = [" doc::é] \"x\" ", " doc::e\u{0301}] \"x\" "];
    let chunks: Vec<_> = ids
        .iter()
        .map(|id| Chunk {
            id: (*id).into(),
            ..chunk("Ann owns Acme.")
        })
        .collect();
    let mut taxonomy = taxonomy();
    taxonomy.push(DirectedRelation {
        relation: "拥有".into(),
        ..taxonomy[0].clone()
    });
    let good = json!({"source": "Ann", "source_type": "entity", "target": "Acme", "target_type": "entity",
        "relation": "拥有", "evidence": "Ann owns Acme.", "chunk_id": ids[0]});
    let mut second = good.clone();
    second["chunk_id"] = json!(ids[1]);
    second["relation"] = json!("OWNS");
    let mut facts = vec![good.clone(), second];
    for id in [
        ids[0].trim().to_string(),
        format!("chunk {}", ids[0]),
        "missing".into(),
    ] {
        let mut wrong = good.clone();
        wrong["chunk_id"] = json!(id);
        facts.push(wrong);
    }
    for relation in ["owns", "OWNS ", "拥有 "] {
        let mut wrong = good.clone();
        wrong["relation"] = json!(relation);
        facts.push(wrong);
    }
    let mut sorted_ids = ids;
    sorted_ids.sort();
    let provider = SchemaProvider {
        response: json!({"facts": facts}),
        chunk_ids: json!(sorted_ids),
        relations: json!(["OWNS", "拥有"]),
    };
    let outcome = directed_ingest(&backend, "bound", &taxonomy, &chunks, &provider)
        .await
        .unwrap();
    assert_eq!(outcome.proposed, 8);
    assert_eq!(outcome.facts_grounded, 2);
    assert_eq!(outcome.skips.len(), 6);
    assert_eq!(
        outcome.extracted_by,
        format!("directed:schema-ignoring-test@{DIRECTED_POLICY}")
    );
    let stored = backend.list_documents("facts", None, None).await.unwrap();
    assert_eq!(stored.len(), 2);
    assert!(
        stored
            .iter()
            .all(|f| ids.contains(&f["evidence_chunk_id"].as_str().unwrap()))
    );
}
