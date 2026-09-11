//! Draft payload.

use super::*;

#[tokio::test]
async fn draft_prepare_refuses_existing_spaces_empty_corpora_and_missing_providers() {
    // No completion provider is wired by `seeded()`, and `pharma` already
    // exists in `space_types`.
    let state = seeded().await;

    // D3, checked at submission so an ineligible corpus never queues.
    let error = prepare_payload(
        &state,
        JobKind::ConstructDraft,
        &json!({ "space_type": "pharma", "chunks": [{"id": "c1", "text": "text"}] }),
        8,
    )
    .await
    .expect_err("drafting an existing space must be refused");
    assert!(
        matches!(&error, CogniGraphError::ValidationError(message)
            if message.contains("already exists in `space_types`")
                && message.contains("(D3)")),
        "unexpected error: {error}"
    );

    let error = prepare_payload(
        &state,
        JobKind::ConstructDraft,
        &json!({ "space_type": "fresh", "chunks": [] }),
        8,
    )
    .await
    .expect_err("an empty corpus must be refused");
    assert!(
        matches!(&error, CogniGraphError::ValidationError(message)
            if message.contains("chunks is empty")),
        "unexpected error: {error}"
    );

    let error = prepare_payload(
        &state,
        JobKind::ConstructDraft,
        &json!({ "space_type": "fresh", "chunks": [{"id": "c1", "text": "text"}] }),
        8,
    )
    .await
    .expect_err("drafting without a completion provider must be refused");
    assert!(
        matches!(&error, CogniGraphError::ValidationError(message)
            if message.contains("No completion provider configured")),
        "unexpected error: {error}"
    );
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn draft_prepare_freezes_documents_grouped_by_title() {
    let state = seeded().await.with_completion(StubCompletion);
    let chunk = |id: &str, title: &str| json!({ "id": id, "title": title, "text": "text" });
    let payload = prepare_payload(
        &state,
        JobKind::ConstructDraft,
        &json!({
            "space_type": "labels",
            "chunks": [
                chunk("a1", "Label A"),
                chunk("b1", "Label B"),
                chunk("a2", "Label A"),
                chunk("b2", "Label B"),
                chunk("a3", "Label A"),
                chunk("b3", "Label B"),
            ],
        }),
        8,
    )
    .await
    .expect("an eligible corpus prepares");
    match payload {
        JobPayload::Draft {
            space_type,
            documents,
            sample_cap,
            batch_size,
        } => {
            assert_eq!(space_type, "labels");
            // Six chunks, two titles: two documents in first-seen order,
            // each drafted densely against its OWN chunks.
            assert_eq!(documents.len(), 2, "chunks group by title: {documents:?}");
            assert_eq!(
                documents[0]
                    .iter()
                    .map(|chunk| chunk.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["a1", "a2", "a3"]
            );
            assert_eq!(
                documents[1]
                    .iter()
                    .map(|chunk| chunk.id.as_str())
                    .collect::<Vec<_>>(),
                vec!["b1", "b2", "b3"]
            );
            assert_eq!(sample_cap, DEFAULT_DRAFT_SAMPLE_CAP);
            assert_eq!(batch_size, 8);
        }
        other => panic!("wrong variant: {other:?}"),
    }
    state.jobs.shutdown().await;
}
#[test]
fn draft_payload_round_trips_through_serde() {
    let payload = JobPayload::Draft {
        space_type: "labels".into(),
        documents: vec![
            vec![Chunk {
                id: "a1".into(),
                title: "Label A".into(),
                text: "alpha".into(),
            }],
            vec![Chunk {
                id: "b1".into(),
                title: "Label B".into(),
                text: "beta".into(),
            }],
        ],
        sample_cap: 40,
        batch_size: 25,
    };
    let value = serde_json::to_value(&payload).unwrap();
    assert_eq!(value["kind"], json!("draft"));
    let restored: JobPayload = serde_json::from_value(value.clone()).unwrap();
    // JobPayload has no PartialEq; assert re-serialization is identical and
    // destructure to check every field survived the round-trip.
    assert_eq!(serde_json::to_value(&restored).unwrap(), value);
    match restored {
        JobPayload::Draft {
            space_type,
            documents,
            sample_cap,
            batch_size,
        } => {
            assert_eq!(space_type, "labels");
            assert_eq!(documents.len(), 2);
            assert_eq!(documents[0][0].id, "a1");
            assert_eq!(documents[1][0].text, "beta");
            assert_eq!(sample_cap, 40);
            assert_eq!(batch_size, 25);
        }
        other => panic!("wrong variant: {other:?}"),
    }
}
/// Scripted, offline drafting model: hands back canned stage-1/stage-2 JSON
/// in order. No network, no live LLM.
pub(super) struct SeqCompletion(pub(super) Mutex<Vec<Value>>);
#[async_trait::async_trait]
impl CompletionProvider for SeqCompletion {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        _schema: &Value,
    ) -> anyhow::Result<Value> {
        let next = {
            let mut queue = self.0.lock().expect("scripted completion lock");
            (!queue.is_empty()).then(|| queue.remove(0))
        };
        next.ok_or_else(|| anyhow::anyhow!("scripted completion exhausted"))
    }
    fn model_name(&self) -> &str {
        "scripted"
    }
}
