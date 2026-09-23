use cognigraph_core::GraphBackend;
use cognigraph_native::NativeBackend;
use serde_json::json;

use super::*;
use crate::types::EvalQuestion;

fn fact(raw: &str) -> Fact {
    Fact::parse(raw).expect("valid fact fixture")
}

fn spec() -> EvalSpec {
    EvalSpec {
        space_id: "verified-space".into(),
        questions: vec![
            EvalQuestion {
                id: "q1".into(),
                question: String::new(),
                expected_facts: vec!["Alpha --REL--> Beta".into(), "malformed".into()],
                forbidden_facts: vec!["Gamma --BLOCKED--> Delta".into()],
            },
            EvalQuestion {
                id: "q2".into(),
                question: String::new(),
                expected_facts: vec!["Epsilon --REL--> Zeta".into(), "Alpha --REL--> Beta".into()],
                forbidden_facts: vec![
                    "Gamma --BLOCKED--> Delta".into(),
                    "Eta --BLOCKED--> Theta".into(),
                ],
            },
        ],
    }
}

#[test]
fn immutable_fact_scoring_deduplicates_and_preserves_spec_order() {
    let facts = HashSet::from([
        fact("Alpha --REL--> Beta"),
        fact("Gamma --BLOCKED--> Delta"),
    ]);

    let outcome = evaluate_facts("verified-space", &spec(), &facts);

    assert_eq!(outcome.expected_total, 2);
    assert_eq!(outcome.expected_found, 1);
    assert_eq!(
        outcome.missing,
        vec![fact("Epsilon --REL--> Zeta")],
        "missing facts follow their first EvalSpec occurrence"
    );
    assert_eq!(outcome.forbidden_total, 2);
    assert_eq!(outcome.forbidden_triggered, 1);
    assert_eq!(
        outcome.violations,
        vec![fact("Gamma --BLOCKED--> Delta")],
        "duplicate forbidden mentions count once"
    );
}

#[tokio::test]
async fn immutable_fact_scoring_matches_backend_scoring() {
    let backend = NativeBackend::new();
    let facts = HashSet::from([
        fact("Alpha --REL--> Beta"),
        fact("Gamma --BLOCKED--> Delta"),
    ]);
    for (index, observed) in facts.iter().enumerate() {
        backend
            .create_edge(
                "facts",
                json!({
                    "_key": format!("fact-{index}"),
                    "_from": format!("entities/{}", entity_key(&observed.source)),
                    "_to": format!("entities/{}", entity_key(&observed.target)),
                    "relation_type": observed.relation,
                    "space_id": "verified-space",
                    "evidence_chunk_id": format!("chunk-{index}"),
                }),
            )
            .await
            .unwrap();
    }

    let from_backend = evaluate(&backend, "verified-space", &spec()).await.unwrap();
    let from_facts = evaluate_facts("verified-space", &spec(), &facts);

    assert_eq!(from_facts, from_backend);
}
