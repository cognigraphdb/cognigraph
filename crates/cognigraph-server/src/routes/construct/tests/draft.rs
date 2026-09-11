//! Draft.

use super::*;

#[tokio::test]
async fn async_drafting_enqueues_a_durable_job_and_requires_an_idempotency_key() {
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({ "entities": [], "rules": [] })));
    let request = || DraftRequest {
        space_type: "pilot".into(),
        chunks: chunks("Vellum built the Kite platform."),
        sample_cap: None,
        per_document: false,
        run_async: true,
    };

    // Replay-safety is not optional for a background submission.
    let refused = draft(
        State(state.clone()),
        None,
        axum::http::HeaderMap::new(),
        Json(request()),
    )
    .await;
    assert!(matches!(
        refused,
        Err(AppError(CogniGraphError::ValidationError(ref message)))
            if message.contains("Idempotency-Key")
    ));

    let mut headers = axum::http::HeaderMap::new();
    headers.insert("Idempotency-Key", "draft-pilot-1".parse().unwrap());
    let accepted = draft(State(state.clone()), None, headers.clone(), Json(request()))
        .await
        .unwrap();
    assert_eq!(accepted.status(), axum::http::StatusCode::ACCEPTED);
    assert!(
        accepted
            .headers()
            .get(axum::http::header::LOCATION)
            .is_some_and(|location| location.to_str().unwrap().starts_with("/api/jobs/")),
        "the submission envelope points at the job"
    );

    // Same key, same input: an idempotent replay, not a second run.
    let replayed = draft(State(state.clone()), None, headers, Json(request()))
        .await
        .unwrap();
    assert_eq!(replayed.status(), axum::http::StatusCode::OK);
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn draft_is_structurally_inert_until_accepted_with_attribution() {
    // Scripted provider: stage 1 entities, then stage 2 rules (the
    // same JSON serves both calls; unknown fields are ignored).
    let state = seeded_state()
        .await
        .with_completion(ScriptedCompletion(json!({
            "entities": [
                { "name": "Vellum", "type": "org", "aliases": [] },
                { "name": "Kite", "type": "platform", "aliases": [] }
            ],
            "rules": [
                { "source": "Vellum", "relation": "BUILT", "target": "Kite",
                  "when_any": ["vellum built the kite platform"] }
            ]
        })));

    // D3: an existing accepted space refuses drafting.
    let refused = draft_sync(
        state.clone(),
        None,
        DraftRequest {
            space_type: "acme".into(),
            chunks: chunks("Vellum built the Kite platform."),
            sample_cap: None,
            per_document: false,
            run_async: false,
        },
    )
    .await;
    assert!(refused.is_err());

    let Json(response) = draft_sync(
        state.clone(),
        None,
        DraftRequest {
            space_type: "pilot".into(),
            chunks: chunks("Vellum built the Kite platform."),
            sample_cap: None,
            per_document: false,
            run_async: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(response["status"], "draft");
    assert_eq!(response["entities"], 2);
    assert_eq!(response["relation_rules"], 1);
    assert_eq!(response["drafted_by"], "draft:scripted@draft-policy-v1");

    // Structural inertness (D1): the draft is invisible to every
    // space-loading path — ingest cannot use it.
    let inert = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "pilot".into(),
            chunks: chunks("Vellum built the Kite platform."),
        }),
    )
    .await;
    assert!(inert.is_err());

    // Acceptance copies to space_types with full attribution.
    let Json(accepted) = draft_accept(
        State(state.clone()),
        axum::extract::Path("pilot".to_string()),
        None,
    )
    .await
    .unwrap();
    assert_eq!(accepted["status"], "accepted");
    let stored = state
        .backend
        .get_document(SPACE_TYPES, "pilot")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored["drafted_by"], "draft:scripted@draft-policy-v1");
    assert_eq!(stored["accepted_by"], "anonymous");
    // Now grounding works — and re-accepting refuses.
    let Json(grounded) = ingest(
        State(state.clone()),
        Json(IngestRequest {
            space_type: "pilot".into(),
            chunks: chunks("Vellum built the Kite platform."),
        }),
    )
    .await
    .unwrap();
    assert_eq!(grounded["facts_grounded"], 1);
    let again = draft_accept(State(state), axum::extract::Path("pilot".to_string()), None).await;
    assert!(again.is_err());
}
