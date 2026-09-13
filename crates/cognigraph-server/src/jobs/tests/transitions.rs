//! Transitions.

use super::*;

#[tokio::test]
async fn submission_is_idempotent_and_conflicts_on_changed_input() {
    let state = seeded().await;
    let first = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "ingest-r1",
            JobKind::ConstructIngest,
            ingest_input("Meridian supplies Compound X."),
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
            "ingest-r1",
            JobKind::ConstructIngest,
            ingest_input("Meridian supplies Compound X."),
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(first.job.id, replay.job.id);
    let conflict = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "ingest-r1",
            JobKind::ConstructIngest,
            ingest_input("different"),
        )
        .await;
    assert!(matches!(
        conflict,
        Err(CogniGraphError::DocumentConflict(_))
    ));

    let finished = wait_terminal(&state, &first.job.id).await;
    assert_eq!(finished.status, JobStatus::Succeeded);
    assert_eq!(finished.progress.completed, 1);
}
#[tokio::test]
async fn queued_cancel_and_idempotent_retry_are_audited() {
    let state = seeded().await;
    state.jobs.pause_worker_claims();
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "cancel-r1",
            JobKind::ConstructIngest,
            ingest_input("Meridian supplies Compound X."),
        )
        .await
        .unwrap();
    let canceled = state
        .jobs
        .cancel(
            "default",
            "default",
            &submission.job.id,
            JobActor::request(None),
            Some("superseded".into()),
        )
        .await
        .unwrap();
    assert!(canceled.changed);
    assert_eq!(canceled.job.status, JobStatus::Canceled);

    wait_unscheduled(&state).await;
    state.jobs.resume_worker_claims();
    let retry = state
        .jobs
        .retry(
            state.clone(),
            "default",
            "default",
            &submission.job.id,
            JobActor::request(None),
            "retry-r1",
            RetryMode::Restart,
            None,
        )
        .await
        .unwrap();
    assert!(!retry.replayed);
    let replay = state
        .jobs
        .retry(
            state.clone(),
            "default",
            "default",
            &submission.job.id,
            JobActor::request(None),
            "retry-r1",
            RetryMode::Restart,
            None,
        )
        .await
        .unwrap();
    assert!(replay.replayed);
    let conflict = state
        .jobs
        .retry(
            state.clone(),
            "default",
            "default",
            &submission.job.id,
            JobActor::request(None),
            "retry-r1",
            RetryMode::Resume,
            Some("changed request".into()),
        )
        .await;
    assert!(matches!(
        conflict,
        Err(CogniGraphError::DocumentConflict(_))
    ));
    let finished = wait_terminal(&state, &submission.job.id).await;
    assert_eq!(finished.status, JobStatus::Succeeded);
    assert!(
        finished
            .events
            .iter()
            .any(|event| event.event == "retry_requested")
    );
}
#[tokio::test]
async fn concurrent_same_key_creates_one_job() {
    let state = seeded().await;
    state.jobs.pause_worker_claims();
    let submit = || {
        state.jobs.submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "concurrent-r1",
            JobKind::ConstructIngest,
            ingest_input("Meridian supplies Compound X."),
        )
    };
    let (left, right) = tokio::join!(submit(), submit());
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(left.job.id, right.job.id);
    assert_ne!(left.replayed, right.replayed);
    let (jobs, total) = state
        .jobs
        .list("default", "default", None, None, 10, 0)
        .await
        .unwrap();
    assert_eq!(total, 1);
    assert_eq!(jobs.len(), 1);
    let summary = jobs[0].public_value();
    assert!(summary.get("input").is_none());
    assert!(summary.get("events").is_none());
    assert!(state.jobs.health().is_ok());
    wait_unscheduled(&state).await;
}
