//! Capacity.

use super::*;

#[tokio::test]
async fn tenant_capacity_backpressures_new_work_but_allows_replay_and_releases_on_cancel() {
    let state = seeded().await;
    state.jobs.configure_governance(10, 1, 86_400, 100);
    state.jobs.pause_worker_claims();
    let first = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "quota-first",
            JobKind::ConstructEvaluate,
            eval_input("quota"),
        )
        .await
        .unwrap();
    let replay = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "quota-first",
            JobKind::ConstructEvaluate,
            eval_input("quota"),
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert!(matches!(
        state
            .jobs
            .submit(
                state.clone(),
                "default".into(),
                "default".into(),
                JobActor::request(None),
                "quota-second",
                JobKind::ConstructEvaluate,
                eval_input("quota"),
            )
            .await,
        Err(CogniGraphError::CapacityExceeded(_))
    ));
    state
        .jobs
        .cancel(
            "default",
            "default",
            &first.job.id,
            JobActor::request(None),
            Some("free capacity".into()),
        )
        .await
        .unwrap();
    let second = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "quota-second",
            JobKind::ConstructEvaluate,
            eval_input("quota"),
        )
        .await
        .unwrap();
    assert!(!second.replayed);
    assert!(state.jobs.metrics_text().contains("scope=\"tenant\"} 1"));
    state
        .jobs
        .cancel(
            "default",
            "default",
            &second.job.id,
            JobActor::request(None),
            None,
        )
        .await
        .unwrap();
    wait_unscheduled(&state).await;
}
#[tokio::test]
async fn global_capacity_is_released_when_a_tenant_store_is_retired() {
    let state = seeded().await;
    state.jobs.configure_governance(1, 1, 86_400, 100);
    state.jobs.pause_worker_claims();
    state
        .jobs
        .submit(
            state.clone(),
            "tenant-a".into(),
            "a-v1".into(),
            JobActor::request(None),
            "global-first",
            JobKind::ConstructEvaluate,
            eval_input("global-capacity"),
        )
        .await
        .unwrap();
    assert!(matches!(
        state
            .jobs
            .submit(
                state.clone(),
                "tenant-b".into(),
                "b-v1".into(),
                JobActor::request(None),
                "global-second",
                JobKind::ConstructEvaluate,
                eval_input("global-capacity"),
            )
            .await,
        Err(CogniGraphError::CapacityExceeded(_))
    ));

    state.jobs.pause_tenant("tenant-a").await;
    state.jobs.retire_tenant("tenant-a");
    let second = state
        .jobs
        .submit(
            state.clone(),
            "tenant-b".into(),
            "b-v1".into(),
            JobActor::request(None),
            "global-second",
            JobKind::ConstructEvaluate,
            eval_input("global-capacity"),
        )
        .await
        .unwrap();
    assert!(!second.replayed);
    assert!(state.jobs.metrics_text().contains("scope=\"global\"} 1"));
    state.jobs.pause_tenant("tenant-b").await;
}
#[tokio::test]
async fn quota_lowering_linearizes_before_final_admission() {
    let raw = Arc::new(NativeBackend::new());
    let auth = Arc::new(AuthProvider::new(raw.clone()).await.unwrap());
    let tenant = auth
        .create_tenant("quota-race", Some(json!({"max_active_jobs": 1})))
        .await
        .unwrap();
    let mut state = AppState::new_shared(raw);
    state.auth = Some(auth.clone());
    state.jobs.pause_admission_reservations();

    let submit_state = state.clone();
    let incarnation = tenant.incarnation.clone();
    let submission = tokio::spawn(async move {
        submit_state
            .jobs
            .submit(
                submit_state.clone(),
                "quota-race".into(),
                incarnation,
                JobActor::request(None),
                "quota-race-submit",
                JobKind::ConstructEvaluate,
                eval_input("quota-race"),
            )
            .await
    });
    state.jobs.wait_for_admission_candidate().await;

    let update = json!({"max_active_jobs": 0});
    let updated = state
        .jobs
        .merge_tenant_quotas(
            &auth,
            "quota-race",
            update.as_object().expect("quota update object"),
        )
        .await
        .unwrap();
    assert_eq!(updated.quotas.unwrap()["max_active_jobs"], 0);
    state.jobs.resume_admission_reservations();

    assert!(matches!(
        submission.await.unwrap(),
        Err(CogniGraphError::CapacityExceeded(_))
    ));
    assert!(
        state
            .jobs
            .active_jobs
            .lock()
            .expect("active jobs lock")
            .is_empty()
    );
    state.jobs.shutdown().await;
}
