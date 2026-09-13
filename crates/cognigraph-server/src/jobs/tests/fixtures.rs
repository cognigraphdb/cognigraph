//! Fixtures.

use super::*;

pub(super) async fn seed_raw(raw: &dyn GraphBackend) {
    raw.create_document(
        "space_types",
        json!({
            "_key": "pharma",
            "id": "pharma",
            "entities": [
                {"name": "Meridian", "type": "org"},
                {"name": "Compound X", "type": "compound"}
            ],
            "relation_rules": [{
                "source": "Meridian",
                "relation": "SUPPLIES",
                "target": "Compound X",
                "when_any": ["meridian supplies compound x"]
            }]
        }),
    )
    .await
    .unwrap();
}
pub(super) async fn seeded() -> AppState {
    let raw = Arc::new(NativeBackend::new());
    seed_raw(&*raw).await;
    AppState::new_shared(raw)
}
pub(super) fn ingest_input(text: &str) -> Value {
    json!({
        "space_type": "pharma",
        "chunks": [{"id": "c1", "text": text}]
    })
}
pub(super) fn eval_input(space: &str) -> Value {
    json!({
        "space_type": space,
        "eval": {
            "space_id": space,
            "expected": [],
            "forbidden": []
        }
    })
}
pub(super) async fn wait_terminal(state: &AppState, id: &str) -> JobRecord {
    wait_terminal_in(state, "default", "default", id).await
}
pub(super) async fn wait_terminal_in(
    state: &AppState,
    tenant: &str,
    incarnation: &str,
    id: &str,
) -> JobRecord {
    for _ in 0..200 {
        let job = state.jobs.get(tenant, incarnation, id).await.unwrap();
        if job.status.terminal() {
            return job;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    panic!("job did not finish")
}
pub(super) async fn wait_unscheduled(state: &AppState) {
    for _ in 0..200 {
        if state
            .jobs
            .scheduled
            .lock()
            .expect("job schedule lock")
            .is_empty()
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    panic!("job stayed scheduled")
}
