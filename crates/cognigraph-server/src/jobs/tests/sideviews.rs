//! Sideviews.

use super::*;

#[tokio::test]
async fn sideviews_prepare_requires_completion_and_embedding_providers() {
    // Providers absent (seeded state wires neither): prepare_payload refuses
    // an ordinary, eligible source collection before enumerating anything.
    let state = seeded().await;
    let input = json!({ "collection": "notes" });
    let error = prepare_payload(&state, JobKind::SideviewsGenerate, &input, 8)
        .await
        .expect_err("side-view generation without providers must be refused");
    assert!(
        matches!(&error, CogniGraphError::ValidationError(message)
            if message.contains("side-view generation requires")),
        "unexpected error: {error}"
    );
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn sideviews_prepare_rejects_ineligible_source_collections() {
    // Governed (managed), control (system), and machine-generated
    // collections are all rejected as sources BEFORE the provider check, so
    // the guard holds even with no provider configured. The side_views
    // collection is generated and therefore never its own source.
    let state = seeded().await;
    for collection in ["neurons", "_users", SIDE_VIEWS_COLLECTION] {
        let input = json!({ "collection": collection });
        let error = prepare_payload(&state, JobKind::SideviewsGenerate, &input, 8)
            .await
            .unwrap_err();
        assert!(
            matches!(&error, CogniGraphError::ValidationError(message)
                if message.contains("cannot be a side-view source")),
            "collection `{collection}` should be ineligible, got: {error}"
        );
    }
    state.jobs.shutdown().await;
}
#[test]
fn sideviews_payload_round_trips_through_serde() {
    let payload = JobPayload::SideviewsGenerate {
        collection: "notes".into(),
        text_field: "body".into(),
        count: 12,
        keys: vec!["a".into(), "b".into()],
        batch_size: 25,
        regenerate: true,
    };
    let value = serde_json::to_value(&payload).unwrap();
    assert_eq!(value["kind"], json!("sideviews_generate"));
    let restored: JobPayload = serde_json::from_value(value.clone()).unwrap();
    // JobPayload has no PartialEq; assert re-serialization is identical and
    // destructure to check every field survived the round-trip.
    assert_eq!(serde_json::to_value(&restored).unwrap(), value);
    match restored {
        JobPayload::SideviewsGenerate {
            collection,
            text_field,
            count,
            keys,
            batch_size,
            regenerate,
        } => {
            assert_eq!(collection, "notes");
            assert_eq!(text_field, "body");
            assert_eq!(count, 12);
            assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
            assert_eq!(batch_size, 25);
            assert!(regenerate);
        }
        other => panic!("wrong variant: {other:?}"),
    }
}
/// Drafting plumbing only — `prepare_payload` never calls the provider, it
/// only requires one to be configured.
pub(super) struct StubCompletion;
#[async_trait::async_trait]
impl CompletionProvider for StubCompletion {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        _schema: &Value,
    ) -> anyhow::Result<Value> {
        Ok(json!({}))
    }
    fn model_name(&self) -> &str {
        "stub"
    }
}
