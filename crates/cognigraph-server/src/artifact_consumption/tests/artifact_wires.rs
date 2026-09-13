//! Artifact wires.

use super::*;

#[test]
fn m21_oracle_eval_spec_and_nested_questions_reject_unknown_fields() {
    let oracle = |eval_spec| {
        json!({
            "schema_version": 1,
            "case_manifest_digest": format!("sha256:{}", "1".repeat(64)),
            "corpus_manifest_digest": format!("sha256:{}", "2".repeat(64)),
            "eval_spec": eval_spec
        })
    };

    let unknown_spec = oracle(json!({
        "space_id": "medical",
        "questions": [],
        "unverified_semantics": "ignored-before-M21"
    }));
    assert!(serde_json::from_value::<PromotionOracleArtifact>(unknown_spec).is_err());

    let unknown_question = oracle(json!({
        "space_id": "medical",
        "questions": [{
            "id": "q1",
            "question": "What is supplied?",
            "expected_facts": [],
            "forbidden_facts": [],
            "unverified_semantics": "ignored-before-M21"
        }]
    }));
    assert!(serde_json::from_value::<PromotionOracleArtifact>(unknown_question).is_err());
}
#[test]
fn m22_corpus_candidate_and_graph_wires_are_closed_and_canonical() {
    let corpus = PreparedChunkCorpusArtifact {
        schema_version: 1,
        space_type: "medical".into(),
        corpus_revision_id: "corpus-v1".into(),
        preprocessing_digest: format!("sha256:{}", "1".repeat(64)),
        chunks: vec![PreparedChunkArtifact {
            id: "chunk-1".into(),
            title: String::new(),
            text: "Acme supplies Compound X.".into(),
        }],
    };
    let canonical = cognigraph_governance::canonical_json_bytes(&corpus).unwrap();
    require_canonical_json("test corpus", &canonical, &corpus).unwrap();
    let pretty = serde_json::to_vec_pretty(&corpus).unwrap();
    assert!(require_canonical_json("test corpus", &pretty, &corpus).is_err());

    let mut corpus_value = serde_json::to_value(&corpus).unwrap();
    corpus_value["chunks"][0]["ignored"] = json!(true);
    assert!(serde_json::from_value::<PreparedChunkCorpusArtifact>(corpus_value).is_err());

    let candidate = json!({
        "schema_version": 1,
        "kind": "semantic-neuron-bundle",
        "id": "candidate-v1",
        "revision": "1",
        "base_space_type": {
            "id": "medical",
            "name": "Medical",
            "version": 1,
            "description": "test",
            "entities": [],
            "relation_rules": []
        },
        "accepted_neurons": [{
            "type": "alias",
            "id": "acme-alias",
            "evidence": ["chunk-1"],
            "entity": "Acme",
            "aliases": ["Acme Corp"],
            "confidence": 0.99
        }]
    });
    assert!(serde_json::from_value::<ConstructionCandidateArtifact>(candidate).is_err());

    let graph = json!({
        "schema_version": 1,
        "space_type": "medical",
        "graph_revision_id": "graph-v1",
        "corpus_manifest_digest": format!("sha256:{}", "1".repeat(64)),
        "corpus_semantic_digest": format!("sha256:{}", "2".repeat(64)),
        "candidate_digest": format!("sha256:{}", "3".repeat(64)),
        "construction_config_digest": format!("sha256:{}", "4".repeat(64)),
        "derivation_plan_digest": format!("sha256:{}", "5".repeat(64)),
        "facts_digest": format!("sha256:{}", "6".repeat(64)),
        "facts": [],
        "unverified_rows": []
    });
    assert!(serde_json::from_value::<ReproducibleEvaluationGraphArtifact>(graph).is_err());
}
#[test]
fn m23_raw_document_wire_is_closed_and_exact_byte_addressed() {
    let bytes = b"\xef\xbb\xbfCafe\xcc\x81\r\ntext";
    let document = RawDocumentArtifact {
        id: "source-1".into(),
        title: "Source".into(),
        media_type: "text/plain; charset=utf-8".into(),
        byte_length: bytes.len() as u64,
        blob_digest: digest_bytes(bytes),
        content_base64url: URL_SAFE_NO_PAD.encode(bytes),
    };
    assert_eq!(decode_raw_document_bytes(&document).unwrap(), bytes);

    let mut padded = document.clone();
    padded.content_base64url.push('=');
    assert!(decode_raw_document_bytes(&padded).is_err());
    let mut wrong_length = document.clone();
    wrong_length.byte_length += 1;
    assert!(decode_raw_document_bytes(&wrong_length).is_err());
    let mut wrong_digest = document.clone();
    wrong_digest.blob_digest = digest_bytes(b"different");
    assert!(decode_raw_document_bytes(&wrong_digest).is_err());

    let set = RawDocumentSetArtifact {
        schema_version: 1,
        space_type: "medical".into(),
        corpus_revision_id: "corpus-v1".into(),
        preparation_plan_digest: format!("sha256:{}", "1".repeat(64)),
        documents: vec![document],
    };
    let canonical = cognigraph_governance::canonical_json_bytes(&set).unwrap();
    require_canonical_json("test raw document set", &canonical, &set).unwrap();
    let mut unknown = serde_json::to_value(&set).unwrap();
    unknown["documents"][0]["unverified"] = json!(true);
    assert!(serde_json::from_value::<RawDocumentSetArtifact>(unknown).is_err());
}
