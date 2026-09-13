//! Recovery.

use super::*;

#[tokio::test]
async fn summary_listing_is_bounded_sorted_and_paged_across_storage_pages() {
    let state = seeded().await;
    state.jobs.pause_worker_claims();
    for index in 0..20 {
        state
            .jobs
            .submit(
                state.clone(),
                "default".into(),
                "default".into(),
                JobActor::request(None),
                &format!("list-r{index}"),
                JobKind::ConstructEvaluate,
                eval_input("listing"),
            )
            .await
            .unwrap();
    }
    wait_unscheduled(&state).await;
    let (all, total) = state
        .jobs
        .list("default", "default", None, None, 200, 0)
        .await
        .unwrap();
    let (page, page_total) = state
        .jobs
        .list("default", "default", None, None, 5, 3)
        .await
        .unwrap();
    assert_eq!(total, 20);
    assert_eq!(page_total, total);
    let expected = all[3..8]
        .iter()
        .map(|job| job.id.as_str())
        .collect::<Vec<_>>();
    let actual = page.iter().map(|job| job.id.as_str()).collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(page[0].public_value().get("input").is_none());
    assert!(
        state
            .jobs
            .list("default", "default", None, None, 5, MAX_JOB_LIST_OFFSET + 1,)
            .await
            .is_err()
    );
}
#[tokio::test]
async fn restart_recovers_running_job_from_persistent_store() {
    let path = std::env::temp_dir().join(format!(
        "cognigraph-m16-recovery-{}-{}.redb",
        std::process::id(),
        now_millis()
    ));
    let job_id;
    {
        let raw = Arc::new(NativeBackend::open(&path).unwrap());
        seed_raw(&*raw).await;
        let state = AppState::new_shared(raw);
        state.jobs.pause_worker_claims();
        let submission = state
            .jobs
            .submit(
                state.clone(),
                "default".into(),
                "default".into(),
                JobActor::request(None),
                "recover-r1",
                JobKind::ConstructIngest,
                ingest_input("Meridian supplies Compound X."),
            )
            .await
            .unwrap();
        job_id = submission.job.id;
        let mut interrupted = state.jobs.get("default", "default", &job_id).await.unwrap();
        interrupted.status = JobStatus::Running;
        interrupted.progress.phase = "running".into();
        interrupted.attempt = 1;
        state.jobs.save_raw("default", &interrupted).await.unwrap();
        wait_unscheduled(&state).await;
        state.jobs.shutdown().await;
    }
    {
        let raw = Arc::new(NativeBackend::open(&path).unwrap());
        let state = AppState::new_shared(raw);
        let recovered = state
            .jobs
            .recover_tenant(state.clone(), "default".into(), "default".into())
            .await
            .unwrap();
        assert_eq!(recovered, 1);
        let finished = wait_terminal(&state, &job_id).await;
        assert_eq!(finished.status, JobStatus::Succeeded);
        assert_eq!(finished.recoveries, 1);
        assert!(
            finished
                .events
                .iter()
                .any(|event| event.event == "recovered")
        );
        state.jobs.shutdown().await;
    }
    std::fs::remove_file(&path).unwrap();
}
#[tokio::test]
async fn suspended_recovery_rebuilds_global_capacity_without_scheduling() {
    let raw = Arc::new(NativeBackend::new());
    let auth = Arc::new(AuthProvider::new(raw.clone()).await.unwrap());
    let suspended = auth.create_tenant("suspended", None).await.unwrap();
    let other = auth.create_tenant("other", None).await.unwrap();
    let mut original = AppState::new_shared(raw.clone());
    original.auth = Some(auth.clone());
    original.jobs.pause_worker_claims();
    original
        .jobs
        .submit(
            original.clone(),
            suspended.name.clone(),
            suspended.incarnation.clone(),
            JobActor::request(None),
            "suspended-capacity-r1",
            JobKind::ConstructEvaluate,
            eval_input("suspended-capacity"),
        )
        .await
        .unwrap();
    wait_unscheduled(&original).await;
    auth.set_tenant_status("suspended", TenantStatus::Suspended)
        .await
        .unwrap();
    original.jobs.shutdown().await;

    let mut restarted = AppState::new_shared(raw);
    restarted.auth = Some(auth);
    restarted.jobs.configure_governance(1, 1, 86_400, 100);
    restarted.jobs.pause_tenant("suspended").await;
    assert_eq!(
        restarted
            .jobs
            .recover_tenant(
                restarted.clone(),
                suspended.name.clone(),
                suspended.incarnation.clone(),
            )
            .await
            .unwrap(),
        1
    );
    let status = restarted
        .jobs
        .operator_status(&restarted, &suspended.name, &suspended.incarnation)
        .await
        .unwrap();
    assert_eq!(status["paused"], true);
    assert_eq!(status["queue"]["active"], 1);
    assert_eq!(status["queue"]["ready"], 0);
    assert!(matches!(
        restarted
            .jobs
            .submit(
                restarted.clone(),
                other.name,
                other.incarnation,
                JobActor::request(None),
                "other-blocked-by-suspended",
                JobKind::ConstructEvaluate,
                eval_input("other-capacity"),
            )
            .await,
        Err(CogniGraphError::CapacityExceeded(_))
    ));
    restarted.jobs.shutdown().await;
}
