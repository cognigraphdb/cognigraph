use std::sync::Mutex;

use super::*;

struct Sequence(Mutex<Vec<Value>>);

#[async_trait::async_trait]
impl CompletionProvider for Sequence {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        _schema: &Value,
    ) -> anyhow::Result<Value> {
        Ok(self.0.lock().expect("script lock").remove(0))
    }

    fn model_name(&self) -> &str {
        "scripted"
    }
}

fn proposed_hint() -> Neuron {
    Neuron {
        id: "hint-1".into(),
        source: "Nimbus".into(),
        relation: "SUPPLIES".into(),
        target: "DataCloud".into(),
        triggers: vec!["runs on nimbus".into()],
        evidence: vec!["DataCloud runs on Nimbus".into()],
        ..Neuron::default()
    }
}

fn chunks() -> Vec<Chunk> {
    vec![Chunk {
        id: "c1".into(),
        title: String::new(),
        text: "DataCloud runs on Nimbus infrastructure.".into(),
    }]
}

#[tokio::test]
async fn malformed_screen_fails_closed_without_invoking_judge() {
    let provider = Sequence(Mutex::new(vec![json!({
        "confidence": 0.99,
        "reasoning": "clean"
    })]));

    let result = judge_neuron(&provider, &proposed_hint(), &chunks())
        .await
        .unwrap();

    assert_eq!(result.verdict, "needs_human");
    assert_eq!(result.confidence, 0.0);
    assert!(result.reasoning.contains("malformed"));
    assert!(provider.0.lock().unwrap().is_empty());
}

#[tokio::test]
async fn malformed_judge_result_fails_closed() {
    let provider = Sequence(Mutex::new(vec![
        json!({ "tainted": false, "confidence": 0.98, "reasoning": "clean" }),
        json!({ "verdict": "accept", "confidence": 1.2, "reasoning": "supported" }),
    ]));

    let result = judge_neuron(&provider, &proposed_hint(), &chunks())
        .await
        .unwrap();

    assert_eq!(result.verdict, "needs_human");
    assert_eq!(result.confidence, 0.0);
    assert!(result.reasoning.contains("malformed"));
    assert_eq!(result.raw["screen"]["tainted"], false);
}

#[tokio::test]
async fn valid_typed_results_preserve_the_judge_verdict() {
    let provider = Sequence(Mutex::new(vec![
        json!({ "tainted": false, "confidence": 0.98, "reasoning": "clean" }),
        json!({
            "verdict": "accept",
            "confidence": 0.96,
            "reasoning": "the evidence supports the exact endpoints",
            "concerns": []
        }),
    ]));

    let result = judge_neuron(&provider, &proposed_hint(), &chunks())
        .await
        .unwrap();

    assert_eq!(result.verdict, "accept");
    assert_eq!(result.confidence, 0.96);
    assert_eq!(result.raw["screen"]["tainted"], false);
}
