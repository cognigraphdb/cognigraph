//! Deterministic races through the real routers; no provider or scheduling luck.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use cognigraph_core::{CogniGraphError, GraphBackend};
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::NativeBackend;
use serde_json::{Value, json};
use tokio::sync::Notify;
use tower::ServiceExt;

use crate::state::AppState;
mod backend;
use backend::ControlledBackend;

const SPACE: &str = "cg21";

async fn setup() -> (AppState, Arc<ControlledBackend>) {
    let backend = Arc::new(ControlledBackend::default());
    backend.inner.import_snapshot(&json!({"collections":{
        "space_types":{"type":"document","documents":{SPACE:{
            "_key":SPACE,"id":SPACE,
            "entities":[{"name":"Nimbus","type":"org"},{"name":"DataCloud","type":"platform"}],
            "relation_rules":[{"source":"Nimbus","relation":"HOSTS","target":"DataCloud","when_any":[]}]}}},
        "review_policies":{"type":"document","documents":{SPACE:{
            "_key":SPACE,"injection_suite_passed":true,"sampling_rate":1.0,
            "auto_accept":{"kinds":["relation_hint"],"min_confidence":0.9,"qualified_judges":["cg21-judge"]}}}}
    }})).await.unwrap();
    (AppState::new_shared(backend.clone()), backend)
}

fn app(state: AppState) -> Router {
    Router::new()
        .nest("/api/neurons", super::neurons::router())
        .nest("/api/construct", super::construct::router())
        .with_state(state)
}

async fn post(app: &Router, path: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn create(app: &Router, key: &str, kind: &str) {
    let (status, body) = post(
        app,
        "/api/neurons",
        json!({
            "space_type":SPACE,"id":key,"type":kind,"confidence":0.9,
            "evidence":["Nimbus hosts DataCloud"],"source":"Nimbus","relation":"HOSTS",
            "target":"DataCloud","triggers":["nimbus hosts datacloud"]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

async fn human(app: &Router, key: &str, action: &str) -> (StatusCode, Value) {
    post(
        app,
        &format!("/api/neurons/{key}/{action}"),
        json!({"note":"human decision"}),
    )
    .await
}
async fn review(app: &Router) -> (StatusCode, Value) {
    post(app, "/api/construct/review", json!({"space_type":SPACE})).await
}
async fn stored(backend: &ControlledBackend, key: &str) -> Value {
    backend
        .inner
        .get_document("neurons", key)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn conflicting_human_acceptances_serialize_validation_through_commit() {
    for first_kind in ["relation_hint", "relation_blocker"] {
        let (state, backend) = setup().await;
        let app = app(state);
        create(&app, "first", first_kind).await;
        create(
            &app,
            "second",
            if first_kind == "relation_hint" {
                "relation_blocker"
            } else {
                "relation_hint"
            },
        )
        .await;
        backend.pause_scan.store(true, Ordering::SeqCst);
        let first_app = app.clone();
        let first = tokio::spawn(async move { human(&first_app, "first", "accept").await });
        backend.entered.notified().await; // First request has captured the old accepted set.
        let second = human(&app, "second", "accept");
        tokio::pin!(second);
        let premature = tokio::time::timeout(Duration::from_millis(50), &mut second).await;
        backend.release.notify_one();
        let first = first.await.unwrap();
        assert_eq!(first.0, StatusCode::OK, "{}", first.1);
        let second = match premature {
            Ok(response) => response,
            Err(_) => second.await,
        };
        assert_eq!(second.0, StatusCode::BAD_REQUEST, "{}", second.1);
        assert_eq!(stored(&backend, "first").await["status"], "accepted");
        assert_eq!(stored(&backend, "second").await["status"], "proposed");
    }
}

#[derive(Default)]
struct PausedJudge {
    entered: Notify,
    release: Notify,
    queue: bool,
    partner: bool,
}
#[async_trait::async_trait]
impl CompletionProvider for PausedJudge {
    fn model_name(&self) -> &str {
        if self.partner {
            "cg21-partner"
        } else {
            "cg21-judge"
        }
    }
    async fn complete_json(&self, _: &str, _: &str, schema: &Value) -> anyhow::Result<Value> {
        if schema["properties"].get("tainted").is_some() {
            return Ok(json!({"tainted":false,"confidence":1.0,"reasoning":"synthetic clean"}));
        }
        self.entered.notify_one();
        self.release.notified().await;
        Ok(
            json!({"verdict":if self.queue {"needs_human"} else {"accept"},"confidence":0.99,
            "reasoning":format!("{}éé", "x".repeat(299)),"concerns":[]}),
        )
    }
}

#[tokio::test]
async fn human_decisions_survive_late_acceptance_and_triage_results() {
    for queue in [false, true] {
        for action in ["reject", "retire", "accept"] {
            let (mut state, backend) = setup().await;
            let judge = Arc::new(PausedJudge {
                queue,
                ..Default::default()
            });
            state.judge = Some(judge.clone());
            let app = app(state);
            create(&app, "hint", "relation_hint").await;
            let review_app = app.clone();
            let pending = tokio::spawn(async move { review(&review_app).await });
            judge.entered.notified().await;
            let human = tokio::time::timeout(Duration::from_secs(2), human(&app, "hint", action))
                .await
                .unwrap();
            assert_eq!(human.0, StatusCode::OK, "{}", human.1);
            let decided = stored(&backend, "hint").await;
            judge.release.notify_one();
            let (status, result) = pending.await.unwrap();
            assert_eq!(status, StatusCode::OK, "{result}");
            assert_eq!(result["skipped"].as_array().unwrap().len(), 1, "{result}");
            assert_eq!(result["reviewed"], 0);
            assert_eq!(result["pending_remaining"], 0);
            assert_eq!(stored(&backend, "hint").await, decided);
        }
    }
}

#[tokio::test]
async fn late_review_revalidates_against_a_newly_accepted_blocker() {
    let (mut state, backend) = setup().await;
    let judge = Arc::new(PausedJudge::default());
    state.judge = Some(judge.clone());
    let app = app(state);
    create(&app, "hint", "relation_hint").await;
    let review_app = app.clone();
    let pending = tokio::spawn(async move { review(&review_app).await });
    judge.entered.notified().await;
    create(&app, "blocker", "relation_blocker").await;
    assert_eq!(human(&app, "blocker", "accept").await.0, StatusCode::OK);
    judge.release.notify_one();
    let (status, result) = pending.await.unwrap();
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["auto_accepted"], json!([]));
    assert!(
        result["queued"][0]["lane_b_reason"]
            .as_str()
            .unwrap()
            .contains("accepted hint and blocker")
    );
    assert_eq!(stored(&backend, "hint").await["status"], "proposed");
    assert_eq!(stored(&backend, "hint").await["judge_verdict"], "accept");
}

#[tokio::test]
async fn overlapping_reviews_publish_only_one_result_even_when_both_queue() {
    for queue in [false, true] {
        let (mut state, backend) = setup().await;
        let judge = Arc::new(PausedJudge {
            queue,
            ..Default::default()
        });
        state.judge = Some(judge.clone());
        let app = app(state);
        create(&app, "hint", "relation_hint").await;
        let a = app.clone();
        let first = tokio::spawn(async move { review(&a).await });
        judge.entered.notified().await;
        let a = app.clone();
        let second = tokio::spawn(async move { review(&a).await });
        judge.entered.notified().await;
        judge.release.notify_waiters();
        let one = first.await.unwrap();
        let two = second.await.unwrap();
        assert_eq!(one.0, StatusCode::OK, "{}", one.1);
        assert_eq!(two.0, StatusCode::OK, "{}", two.1);
        assert_eq!(
            one.1["reviewed"].as_u64().unwrap() + two.1["reviewed"].as_u64().unwrap(),
            1
        );
        assert_eq!(
            one.1["skipped"].as_array().unwrap().len() + two.1["skipped"].as_array().unwrap().len(),
            1
        );
        let doc = stored(&backend, "hint").await;
        assert_eq!(doc["status"], if queue { "proposed" } else { "accepted" });
        if !queue {
            assert_eq!(doc["review_note"].as_str().unwrap().len(), 299);
        }
    }
}

#[tokio::test]
async fn unchanged_status_does_not_hide_source_edits_during_review() {
    for collection in ["neurons", "space_types", "review_policies"] {
        let (mut state, backend) = setup().await;
        let judge = Arc::new(PausedJudge::default());
        state.judge = Some(judge.clone());
        let app = app(state);
        create(&app, "hint", "relation_hint").await;
        let a = app.clone();
        let pending = tokio::spawn(async move { review(&a).await });
        judge.entered.notified().await;
        let (key, merge) = match collection {
            "neurons" => ("hint", json!({"rationale":"changed proposal"})),
            "space_types" => (SPACE, json!({"description":"changed ontology"})),
            _ => (SPACE, json!({"sampling_rate":0.5})),
        };
        backend
            .inner
            .update_document(collection, key, merge)
            .await
            .unwrap();
        let current = stored(&backend, "hint").await;
        judge.release.notify_one();
        let (status, result) = pending.await.unwrap();
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(
            result["skipped"].as_array().unwrap().len(),
            1,
            "{collection}: {result}"
        );
        assert_eq!(stored(&backend, "hint").await, current);
        assert_eq!(
            result["pending_remaining"], 1,
            "changed proposal still needs review"
        );
    }
}

#[tokio::test]
async fn accepted_set_read_failures_and_malformed_rows_cannot_bypass_validation() {
    for malformed in [false, true] {
        let (state, backend) = setup().await;
        let app = app(state);
        create(&app, "hint", "relation_hint").await;
        if malformed {
            backend
                .inner
                .create_document(
                    "neurons",
                    json!({"_key":"broken","space_type":SPACE,"status":"accepted"}),
                )
                .await
                .unwrap();
        } else {
            backend.fail_scan.store(true, Ordering::SeqCst);
        }
        let (status, result) = human(&app, "hint", "accept").await;
        assert_eq!(
            status,
            if malformed {
                StatusCode::INTERNAL_SERVER_ERROR
            } else {
                StatusCode::SERVICE_UNAVAILABLE
            },
            "{result}"
        );
        assert_eq!(stored(&backend, "hint").await["status"], "proposed");
        backend
            .inner
            .delete_document("neurons", "broken")
            .await
            .unwrap();
        assert_eq!(
            human(&app, "hint", "accept").await.0,
            StatusCode::OK,
            "failure must release lock"
        );
    }
}

#[tokio::test]
async fn agreement_lane_rechecks_after_the_partner_returns() {
    for action in ["none", "reject", "blocker"] {
        let (mut state, backend) = setup().await;
        backend
            .inner
            .update_document(
                "space_types",
                SPACE,
                json!({"relation_rules":[{
            "source":"Nimbus","relation":"HOSTS","target":"DataCloud",
            "when_any":["{source} hosts {target}"]}]}),
            )
            .await
            .unwrap();
        backend
            .inner
            .update_document(
                "review_policies",
                SPACE,
                json!({"auto_accept":{
            "kinds":["relation_hint"],"min_confidence":0.9,"qualified_judges":["cg21-judge"],
            "agreement":{"judges":["cg21-judge","cg21-partner"],"kinds":["relation_hint"],
                "min_confidence":0.9,"concordance_measured":true}}}),
            )
            .await
            .unwrap();
        let primary = Arc::new(PausedJudge::default());
        let partner = Arc::new(PausedJudge {
            partner: true,
            ..Default::default()
        });
        state.judge = Some(primary.clone());
        state.judge_partner = Some(partner.clone());
        let app = app(state);
        create(&app, "hint", "relation_hint").await;
        let a = app.clone();
        let pending = tokio::spawn(async move { review(&a).await });
        primary.entered.notified().await;
        primary.release.notify_one();
        partner.entered.notified().await;
        if action == "reject" {
            assert_eq!(human(&app, "hint", "reject").await.0, StatusCode::OK);
        }
        if action == "blocker" {
            create(&app, "blocker", "relation_blocker").await;
            assert_eq!(human(&app, "blocker", "accept").await.0, StatusCode::OK);
        }
        partner.release.notify_one();
        let (status, result) = pending.await.unwrap();
        assert_eq!(status, StatusCode::OK, "{result}");
        let doc = stored(&backend, "hint").await;
        match action {
            "none" => {
                assert_eq!(doc["status"], "accepted");
                assert_eq!(doc["lane"], "A+");
            }
            "reject" => {
                assert_eq!(doc["status"], "rejected");
                assert_eq!(result["skipped"].as_array().unwrap().len(), 1);
            }
            _ => {
                assert_eq!(doc["status"], "proposed");
                assert_eq!(result["queued"].as_array().unwrap().len(), 1);
            }
        }
    }
}

#[tokio::test]
async fn automated_acceptance_propagates_current_accepted_set_outages() {
    let (mut state, backend) = setup().await;
    let judge = Arc::new(PausedJudge::default());
    state.judge = Some(judge.clone());
    let app = app(state);
    create(&app, "hint", "relation_hint").await;
    let before = stored(&backend, "hint").await;
    let a = app.clone();
    let pending = tokio::spawn(async move { review(&a).await });
    judge.entered.notified().await;
    backend.fail_scan.store(true, Ordering::SeqCst);
    judge.release.notify_one();
    let (status, result) = pending.await.unwrap();
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{result}");
    assert_eq!(stored(&backend, "hint").await, before);
    assert_eq!(human(&app, "hint", "accept").await.0, StatusCode::OK);
}
