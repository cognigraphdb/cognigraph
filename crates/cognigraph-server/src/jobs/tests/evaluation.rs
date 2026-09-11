//! Evaluation.

use super::*;

#[tokio::test]
async fn m21_evaluation_input_closes_nested_eval_fields_without_breaking_legacy() {
    let state = seeded().await;
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/m18/promotion-evaluation.json"
    ))
    .unwrap();

    for pointer in ["/eval", "/eval/questions/0"] {
        let mut legacy = fixture["input"].clone();
        legacy
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .unwrap()
            .insert(
                "unverified_semantics".into(),
                json!("ignored by the legacy diagnostic contract"),
            );
        prepare_payload(&state, JobKind::ConstructEvaluate, &legacy, 1)
            .await
            .expect("pre-M21 evaluation inputs retain their historical permissive shape");

        let mut m21 = legacy;
        m21["promotion_context"]["schema_version"] =
            json!(crate::promotions::M21_PROMOTION_CONTEXT_SCHEMA_VERSION);
        let error = prepare_payload(&state, JobKind::ConstructEvaluate, &m21, 1)
            .await
            .expect_err("M21 evaluation input must reject unknown nested EvalSpec fields");
        assert!(
            matches!(error, CogniGraphError::ValidationError(ref message) if message.contains("unknown field `unverified_semantics`")),
            "unexpected M21 strict-wire error: {error}"
        );
    }
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn promotion_source_recomputes_and_latches_tampered_job_input_digest() {
    let raw = Arc::new(NativeBackend::new());
    seed_raw(&*raw).await;
    raw.ensure_collection("facts", CollectionType::Edge)
        .await
        .unwrap();
    let state = AppState::new_shared(raw.clone());
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../fixtures/m18/promotion-evaluation.json"
    ))
    .unwrap();
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "m18-input-digest-corruption",
            JobKind::ConstructEvaluate,
            fixture["input"].clone(),
        )
        .await
        .unwrap();
    let job = wait_terminal(&state, &submission.job.id).await;
    assert_eq!(job.status, JobStatus::Succeeded);

    let mut stored = raw
        .get_document(JOBS_COLLECTION, &job.id)
        .await
        .unwrap()
        .unwrap();
    stored["input_digest"] = json!("0".repeat(64));
    raw.replace_document(JOBS_COLLECTION, &job.id, stored)
        .await
        .unwrap();

    assert!(matches!(
        state
            .jobs
            .promotion_evaluation_source("default", "default", &job.id)
            .await,
        Err(CogniGraphError::BackendError(_))
    ));
    assert!(state.jobs.health().is_err());
    state.jobs.shutdown().await;
}
