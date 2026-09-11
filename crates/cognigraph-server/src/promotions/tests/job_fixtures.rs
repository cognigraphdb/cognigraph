//! Job fixtures.

use super::*;

pub(super) async fn wait_terminal(state: &AppState, id: &str) -> JobRecord {
    for _ in 0..500 {
        let job = state
            .jobs
            .get(TENANT, INCARNATION, id)
            .await
            .expect("submitted job");
        if job.status.terminal() {
            return job;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("evaluation job `{id}` did not reach a terminal state")
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
pub(super) fn governed_context(
    candidate: &str,
    binding: &PolicyGovernanceBinding,
) -> PromotionContext {
    let mut context = promotion_context(candidate, &eval_spec());
    context.schema_version = M19_PROMOTION_CONTEXT_SCHEMA_VERSION;
    context.governance = Some(binding.clone());
    context
}
