//! Job fixtures.

use super::*;

pub(super) async fn wait_terminal(state: &AppState, id: &str) -> JobRecord {
    // Artifact verification can exceed a few seconds in debug builds on CI.
    // Bound elapsed time, including repository reads, and retain useful failure state.
    let budget = std::time::Duration::from_secs(60);
    let mut last_observed = None;
    let result = tokio::time::timeout(budget, async {
        loop {
            let job = state
                .jobs
                .get(TENANT, INCARNATION, id)
                .await
                .expect("submitted job");
            if job.status.terminal() {
                return job;
            }
            last_observed = Some((job.status, job.attempt, job.progress));
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await;
    result.unwrap_or_else(|_| {
        panic!(
            "evaluation job `{id}` did not reach a terminal state within {budget:?}; \
             last status, attempt and progress: {last_observed:?}"
        )
    })
}
pub(super) async fn submit_evaluation(
    state: &AppState,
    idempotency_key: &str,
    context: PromotionContext,
) -> String {
    let submission = state
        .jobs
        .submit(
            state.clone(),
            TENANT.into(),
            INCARNATION.into(),
            JobActor::request(None),
            idempotency_key,
            JobKind::ConstructEvaluate,
            json!({
                "space_type": SPACE,
                "eval": eval_spec(),
                "promotion_context": context,
            }),
        )
        .await
        .expect("submit promotion evaluation");
    assert!(!submission.replayed);
    let id = submission.job.id;
    let finished = wait_terminal(state, &id).await;
    assert_eq!(
        finished.status,
        JobStatus::Succeeded,
        "evaluation failed: {:?}",
        finished.error
    );
    id
}
pub(super) async fn submit_delayed_evaluation(
    state: &AppState,
    idempotency_key: &str,
    context: PromotionContext,
) -> String {
    // Complete a real evaluation beyond the former 500 x 5 ms polling allowance.
    state.jobs.set_artifact_consumption_delay_ms(4_000);
    let started = tokio::time::Instant::now();
    let id = submit_evaluation(state, idempotency_key, context).await;
    assert!(started.elapsed() >= Duration::from_secs(4));
    state.jobs.set_artifact_consumption_delay_ms(0);
    id
}

pub(super) fn governed_context(
    candidate: &str,
    binding: &PolicyGovernanceBinding,
) -> PromotionContext {
    let mut context = promotion_context(candidate, &eval_spec());
    context.schema_version = M19_PROMOTION_CONTEXT_SCHEMA_VERSION;
    context.governance = Some(binding.clone());
    context
}
