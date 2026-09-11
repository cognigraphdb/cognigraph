//! Retry and scope.

use super::*;

#[tokio::test]
async fn immediate_retry_is_rescheduled_after_worker_cleanup() {
    let state = seeded().await;
    state.jobs.set_cleanup_delay_ms(50);
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "cleanup-race-submit",
            JobKind::ConstructEvaluate,
            eval_input("cleanup-race"),
        )
        .await
        .unwrap();
    let mut first = wait_terminal(&state, &submission.job.id).await;
    assert_eq!(first.attempt, 1);
    assert_eq!(first.status, JobStatus::Succeeded);
    assert!(
        state
            .jobs
            .scheduled
            .lock()
            .expect("job schedule lock")
            .contains(&("default".into(), submission.job.id.clone()))
    );
    first.status = JobStatus::Failed;
    first.progress.phase = "failed".into();
    first.error = Some(json!({
        "code": "test_failure",
        "message": "retryable terminal state during worker cleanup",
        "retryable": true,
    }));
    state.jobs.save_raw("default", &first).await.unwrap();
    assert!(matches!(
        state
            .jobs
            .recover_tenant(state.clone(), "default".into(), "default".into())
            .await,
        Err(CogniGraphError::DocumentConflict(_))
    ));

    let retried = state
        .jobs
        .retry(
            state.clone(),
            "default",
            "default",
            &submission.job.id,
            JobActor::request(None),
            "cleanup-race-retry",
            RetryMode::Restart,
            None,
        )
        .await
        .unwrap();
    assert_eq!(retried.job.status, JobStatus::Queued);
    let finished = wait_terminal(&state, &submission.job.id).await;
    assert_eq!(finished.status, JobStatus::Succeeded);
    assert_eq!(finished.attempt, 2);
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn idle_import_fence_rejects_active_queue_without_pausing_it() {
    let state = seeded().await;
    state.jobs.pause_worker_claims();
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "import-fence-r1",
            JobKind::ConstructIngest,
            ingest_input("Meridian supplies Compound X."),
        )
        .await
        .unwrap();
    wait_unscheduled(&state).await;
    assert!(
        !state
            .jobs
            .pause_tenant_if_idle("default", "default")
            .await
            .unwrap()
    );
    assert!(!state.jobs.tenant_paused("default"));

    state.jobs.resume_worker_claims();
    state
        .jobs
        .recover_tenant(state.clone(), "default".into(), "default".into())
        .await
        .unwrap();
    let finished = wait_terminal(&state, &submission.job.id).await;
    assert_eq!(finished.status, JobStatus::Succeeded);
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn explicit_default_tenant_record_preserves_existing_job_identity() {
    let raw = Arc::new(NativeBackend::new());
    let auth = Arc::new(
        cognigraph_auth::AuthProvider::new(raw.clone())
            .await
            .unwrap(),
    );
    let mut state = AppState::new_shared(raw);
    state.auth = Some(auth.clone());
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "default-incarnation-r1",
            JobKind::ConstructEvaluate,
            eval_input("default-incarnation"),
        )
        .await
        .unwrap();
    let tenant = auth.create_tenant(DEFAULT_TENANT, None).await.unwrap();
    assert_eq!(tenant.incarnation, DEFAULT_TENANT);
    let incarnation = JobManager::tenant_incarnation(&state, DEFAULT_TENANT)
        .await
        .unwrap();
    assert_eq!(incarnation, DEFAULT_TENANT);
    assert!(
        state
            .jobs
            .get(DEFAULT_TENANT, &incarnation, &submission.job.id)
            .await
            .is_ok()
    );
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn worker_never_falls_back_to_default_tenant() {
    use crate::tenancy::{RoutedBackend, TenantRegistry};

    let registry = Arc::new(TenantRegistry::new(
        Box::new(|_| Ok(Arc::new(NativeBackend::new()) as Arc<dyn GraphBackend>)),
        None,
    ));
    let acme = registry.store("acme").unwrap();
    let other = registry.store("other").unwrap();
    seed_raw(&*acme).await;
    seed_raw(&*other).await;
    let mut state = AppState::new_shared(Arc::new(RoutedBackend::new(registry.clone())));
    state.tenant_registry = Some(registry.clone());
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "acme".into(),
            "acme-v1".into(),
            JobActor::request(None),
            "tenant-r1",
            JobKind::ConstructIngest,
            ingest_input("Meridian supplies Compound X."),
        )
        .await
        .unwrap();
    let finished = wait_terminal_in(&state, "acme", "acme-v1", &submission.job.id).await;
    assert_eq!(finished.status, JobStatus::Succeeded);
    assert!(
        acme.list_collections()
            .await
            .unwrap()
            .iter()
            .any(|c| c.name == "facts")
    );
    assert!(
        !other
            .list_collections()
            .await
            .unwrap()
            .iter()
            .any(|c| c.name == "facts")
    );
    let default = registry.store("default").unwrap();
    assert!(
        !default
            .list_collections()
            .await
            .unwrap()
            .iter()
            .any(|c| c.name == "facts")
    );
    assert!(
        state
            .jobs
            .get("other", "other-v1", &submission.job.id)
            .await
            .is_err()
    );
    assert!(!state.jobs.metrics_text().contains("acme"));
    state.jobs.shutdown().await;
}
#[tokio::test]
async fn only_submitter_or_admin_can_manage_a_user_job() {
    let state = seeded().await;
    state.jobs.pause_worker_claims();
    let owner = User {
        key: "owner-key".into(),
        username: "owner".into(),
        role: Role::Editor,
        tenant: "default".into(),
    };
    let other = User {
        key: "other-key".into(),
        username: "other".into(),
        role: Role::Editor,
        tenant: "default".into(),
    };
    let admin = User {
        key: "admin-key".into(),
        username: "admin".into(),
        role: Role::Admin,
        tenant: "default".into(),
    };
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(Some(&owner)),
            "owned-r1",
            JobKind::ConstructIngest,
            ingest_input("Meridian supplies Compound X."),
        )
        .await
        .unwrap();

    assert!(JobManager::can_manage(&submission.job, Some(&owner)));
    assert!(JobManager::can_manage(&submission.job, Some(&admin)));
    assert!(!JobManager::can_manage(&submission.job, Some(&other)));
    assert!(!JobManager::can_manage(&submission.job, None));
    wait_unscheduled(&state).await;
}
