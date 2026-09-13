//! Fairness.

use super::*;

#[tokio::test]
async fn scheduler_yields_long_ingest_at_checkpoints_between_tenants() {
    let state = seeded().await;
    state.jobs.set_batch_size(1);
    state.jobs.pause_worker_claims();
    let chunks = (0..100)
        .map(|index| json!({"id": format!("fair-{index}"), "text": "No relation here."}))
        .collect::<Vec<_>>();
    let long = state
        .jobs
        .submit(
            state.clone(),
            "tenant-a".into(),
            "a-v1".into(),
            JobActor::request(None),
            "fair-long",
            JobKind::ConstructIngest,
            json!({"space_type": "pharma", "chunks": chunks}),
        )
        .await
        .unwrap();
    let short = state
        .jobs
        .submit(
            state.clone(),
            "tenant-b".into(),
            "b-v1".into(),
            JobActor::request(None),
            "fair-short",
            JobKind::ConstructEvaluate,
            eval_input("fair-short"),
        )
        .await
        .unwrap();
    wait_unscheduled(&state).await;
    state.jobs.resume_worker_claims();
    state
        .jobs
        .recover_tenant(state.clone(), "tenant-a".into(), "a-v1".into())
        .await
        .unwrap();
    state
        .jobs
        .recover_tenant(state.clone(), "tenant-b".into(), "b-v1".into())
        .await
        .unwrap();
    let short_done = wait_terminal_in(&state, "tenant-b", "b-v1", &short.job.id).await;
    let long_at_that_point = state
        .jobs
        .get("tenant-a", "a-v1", &long.job.id)
        .await
        .unwrap();
    assert_eq!(short_done.status, JobStatus::Succeeded);
    assert!(
        !long_at_that_point.status.terminal(),
        "long tenant monopolized the singleton dispatcher"
    );
    let long_done = wait_terminal_in(&state, "tenant-a", "a-v1", &long.job.id).await;
    assert_eq!(long_done.status, JobStatus::Succeeded);
    state.jobs.shutdown().await;
}
