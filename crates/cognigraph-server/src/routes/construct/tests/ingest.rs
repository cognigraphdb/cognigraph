//! Ingest.

use super::*;

#[tokio::test]
async fn grounds_facts_from_chunks() {
    let state = seeded_state().await;
    let Json(response) = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("Everyone knows DataCloud runs on Nimbus these days."),
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["facts_grounded"], 1);
    assert_eq!(response["chunks"], 1);

    let facts = state
        .backend
        .list_documents("facts", None, None)
        .await
        .unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0]["relation_type"], "SUPPLIES");
    assert_eq!(facts[0]["evidence_chunk_id"], "c1");
    assert_eq!(facts[0]["space_id"], "acme");

    // Re-ingesting the same chunks is idempotent (edges upsert).
    let Json(again) = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("Everyone knows DataCloud runs on Nimbus these days."),
        }),
    )
    .await
    .unwrap();
    assert_eq!(again["facts_grounded"], 1);
    let facts = state
        .backend
        .list_documents("facts", None, None)
        .await
        .unwrap();
    assert_eq!(facts.len(), 1);
}
#[tokio::test]
async fn governed_ingest_fails_closed_without_approved_selected_authority() {
    let state = seeded_state().await;
    let result = governed_ingest(
        State(state.clone()),
        Json(GovernedIngestRequest {
            target: PromotionTarget {
                space_type: "acme".into(),
                channel: "m25-test".into(),
            },
            chunks: chunks("Everyone knows DataCloud runs on Nimbus these days."),
        }),
    )
    .await;

    assert!(result.is_err());
    assert!(
        state
            .backend
            .list_documents("facts", None, None)
            .await
            .unwrap_or_default()
            .is_empty()
    );
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn governed_ingest_rejects_unbounded_work_before_authority_lookup() {
    let state = seeded_state().await;
    let result = governed_ingest(
        State(state.clone()),
        Json(GovernedIngestRequest {
            target: PromotionTarget {
                space_type: "acme".into(),
                channel: "m25-test".into(),
            },
            chunks: (0..=MAX_GOVERNED_INGEST_CHUNKS)
                .map(|index| Chunk {
                    id: format!("c-{index}"),
                    title: String::new(),
                    text: "bounded fixture".into(),
                })
                .collect(),
        }),
    )
    .await;

    assert!(matches!(
        result.unwrap_err().0,
        CogniGraphError::CapacityExceeded(_)
    ));
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn negated_trigger_does_not_ground() {
    let state = seeded_state().await;
    let Json(response) = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("It is not true that DataCloud runs on Nimbus."),
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["facts_grounded"], 0);
}
#[tokio::test]
async fn accepted_blocker_vetoes_grounding() {
    let state = seeded_state().await;
    state
        .managed_backend
        .create_document(
            "neurons",
            json!({
                "_key": "b1", "id": "b1",
                "type": "relation_blocker", "status": "accepted",
                "space_type": "acme",
                "source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
                "triggers": ["according to the rumor"]
            }),
        )
        .await
        .unwrap();
    let Json(response) = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "acme".into(),
            chunks: chunks("According to the rumor, DataCloud runs on Nimbus."),
        }),
    )
    .await
    .unwrap();
    assert_eq!(response["accepted_neurons"], 1);
    assert_eq!(response["facts_grounded"], 0);
}
/// Scripted completion: always returns the same relation-hint JSON.
pub(super) struct ScriptedCompletion(pub(super) Value);
#[async_trait::async_trait]
impl cognigraph_embeddings::completion::CompletionProvider for ScriptedCompletion {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        schema: &Value,
    ) -> anyhow::Result<Value> {
        if schema["properties"].get("tainted").is_some() && self.0.get("tainted").is_none() {
            return Ok(json!({
                "tainted": false,
                "confidence": 0.99,
                "reasoning": "clean"
            }));
        }
        Ok(self.0.clone())
    }
    fn model_name(&self) -> &str {
        "scripted"
    }
}
