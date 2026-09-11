//! Panic cleanup.

use super::*;

#[tokio::test]
async fn worker_panic_releases_all_runtime_state_and_capacity() {
    let state = seeded().await;
    state.jobs.configure_governance(1, 1, 86_400, 100);
    state.jobs.panic_worker_once();
    let submission = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "panic-cleanup-r1",
            JobKind::ConstructEvaluate,
            eval_input("panic-cleanup"),
        )
        .await
        .unwrap();

    let failed = wait_terminal(&state, &submission.job.id).await;
    wait_unscheduled(&state).await;
    assert_eq!(failed.status, JobStatus::Failed);
    assert_eq!(
        failed
            .error
            .as_ref()
            .and_then(|error| error["code"].as_str()),
        Some("execution_failed")
    );
    assert_eq!(state.jobs.metrics.running.load(Ordering::Relaxed), 0);
    assert!(
        state
            .jobs
            .running_tenant
            .lock()
            .expect("running tenant lock")
            .is_none()
    );
    {
        let scheduler = state.jobs.scheduler.lock().expect("job scheduler lock");
        assert!(scheduler.running.is_none());
    }
    assert!(
        !state
            .jobs
            .scheduled
            .lock()
            .expect("job schedule lock")
            .contains(&("default".into(), submission.job.id.clone()))
    );
    assert!(
        !state
            .jobs
            .active_jobs
            .lock()
            .expect("active jobs lock")
            .contains(&(
                "default".into(),
                "default".into(),
                submission.job.id.clone(),
            ))
    );

    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        state.jobs.pause_tenant("default"),
    )
    .await
    .expect("pause must not hang after a worker panic");
    let recovered = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        state
            .jobs
            .recover_tenant(state.clone(), "default".into(), "default".into()),
    )
    .await
    .expect("recovery must not hang after a worker panic")
    .unwrap();
    assert_eq!(recovered, 0);

    state.jobs.resume_tenant("default");
    let replacement = state
        .jobs
        .submit(
            state.clone(),
            "default".into(),
            "default".into(),
            JobActor::request(None),
            "panic-cleanup-r2",
            JobKind::ConstructEvaluate,
            eval_input("panic-cleanup"),
        )
        .await
        .expect("the failed job must release its only capacity slot");
    assert_eq!(
        wait_terminal(&state, &replacement.job.id).await.status,
        JobStatus::Succeeded
    );
    state.jobs.shutdown().await;
}
#[test]
fn promotion_eval_validation_requires_real_disjoint_denominators() {
    let valid: EvalSpec = serde_json::from_value(json!({
        "space_id": "pharma",
        "questions": [{
            "id": "q1",
            "expected_facts": ["A --REL--> B", "A --REL--> B"],
            "forbidden_facts": ["A --REL--> C"]
        }]
    }))
    .unwrap();
    assert_eq!(validate_promotion_eval_spec(&valid).unwrap(), (1, 1));

    for invalid in [
        json!({"space_id":"pharma", "questions":[]}),
        json!({"space_id":"pharma", "questions":[{"id":"q1", "expected_facts":["not a fact"], "forbidden_facts":["A --REL--> C"]}]}),
        json!({"space_id":"pharma", "questions":[{"id":"q1", "expected_facts":["A --REL--> B"], "forbidden_facts":[]}]}),
        json!({"space_id":"pharma", "questions":[{"id":"q1", "expected_facts":["A --REL--> B"], "forbidden_facts":["A --REL--> B"]}]}),
        json!({"space_id":"pharma", "questions":[{"id":"q1", "expected_facts":["A --REL--> B"], "forbidden_facts":["A --REL--> C"]}, {"id":"q1", "expected_facts":[], "forbidden_facts":[]}]}),
    ] {
        let spec: EvalSpec = serde_json::from_value(invalid).unwrap();
        assert!(validate_promotion_eval_spec(&spec).is_err());
    }
}
#[test]
fn canonical_digest_ignores_object_key_order() {
    let left = serde_json::from_str::<Value>(r#"{"a":1,"b":{"x":2,"y":3}}"#).unwrap();
    let right = serde_json::from_str::<Value>(r#"{"b":{"y":3,"x":2},"a":1}"#).unwrap();
    assert_eq!(digest_json(&left).unwrap(), digest_json(&right).unwrap());
}
