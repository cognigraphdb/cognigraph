use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use cognigraph_embeddings::completion::CompletionProvider;
use cognigraph_native::NativeBackend;
use tokio::sync::Notify;

use super::*;

#[derive(Default)]
struct CountingProvider {
    calls: AtomicUsize,
    entered: Notify,
    release: Option<Notify>,
}

#[async_trait]
impl CompletionProvider for CountingProvider {
    async fn complete_json(
        &self,
        _system: &str,
        _user: &str,
        _schema: &Value,
    ) -> anyhow::Result<Value> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if let Some(release) = &self.release {
            release.notified().await;
        }
        Ok(json!({"facts": []}))
    }

    fn model_name(&self) -> &str {
        "cg3-test"
    }
}

fn request(space: &str) -> DirectedRequest {
    DirectedRequest {
        space_type: space.into(),
        taxonomy: vec![DirectedRelation {
            relation: "SUPPLIES".into(),
            description: "one company supplies another".into(),
            require_in_sentence: vec!["supplies".into()],
        }],
        chunks: vec![Chunk {
            id: "cg3-directed-chunk".into(),
            title: String::new(),
            text: "Acme supplies Beta.".into(),
        }],
    }
}

#[tokio::test]
async fn undeployed_space_accepts_directed_ingestion_and_releases_transition() {
    let mut state = AppState::new(NativeBackend::new());
    let provider = Arc::new(CountingProvider::default());
    state.completion = Some(provider.clone());
    let Json(result) = directed(State(state.clone()), None, Json(request("cg3-open")))
        .await
        .unwrap();
    assert_eq!(result["chunks"], 1);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert!(
        state
            .backend
            .get_document(SPACE_TYPES, "cg3-open")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        state
            .backend
            .list_documents("chunks", None, None)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(state.promotions.transition_lock.try_lock().is_ok());
}

#[tokio::test]
async fn paused_tenant_rejects_directed_before_space_creation_or_completion() {
    let mut state = AppState::new(NativeBackend::new());
    let provider = Arc::new(CountingProvider::default());
    state.completion = Some(provider.clone());
    state.promotions.pause_tenant(&current_tenant()).await;
    let before = state.backend.export_snapshot().await.unwrap();
    let err = directed(State(state.clone()), None, Json(request("cg3-paused")))
        .await
        .unwrap_err();
    assert!(matches!(err.0, CogniGraphError::Forbidden(_)), "{err:?}");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.backend.export_snapshot().await.unwrap(), before);
}

#[tokio::test]
async fn degraded_authority_rejects_directed_before_space_creation_or_completion() {
    let mut state = AppState::new(NativeBackend::new());
    let provider = Arc::new(CountingProvider::default());
    state.completion = Some(provider.clone());
    state
        .promotions
        .errors
        .lock()
        .unwrap()
        .insert(current_tenant(), "CG-3 synthetic recovery failure".into());
    let before = state.backend.export_snapshot().await.unwrap();
    let err = directed(State(state.clone()), None, Json(request("cg3-degraded")))
        .await
        .unwrap_err();
    assert!(
        matches!(err.0, CogniGraphError::ConnectionError(_)),
        "{err:?}"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.backend.export_snapshot().await.unwrap(), before);
}

/// Used by the signed M26 lifecycle test so this assertion exercises a real
/// generation and deployment, including all authority validation.
pub(crate) async fn deploy_after_directed<T>(
    state: &AppState,
    space: &str,
    deployment: impl Future<Output = Result<T, CogniGraphError>>,
) -> Result<T, CogniGraphError> {
    let provider = Arc::new(CountingProvider {
        release: Some(Notify::new()),
        ..Default::default()
    });
    let mut directed_state = state.clone();
    directed_state.completion = Some(provider.clone());
    let ingestion = directed(State(directed_state), None, Json(request(space)));
    tokio::pin!(ingestion);
    tokio::select! {
        entered = tokio::time::timeout(Duration::from_secs(5), provider.entered.notified()) => {
            entered.expect("directed ingestion did not reach its provider");
        }
        result = &mut ingestion => panic!("directed ingestion finished before provider release: {result:?}"),
    }
    assert!(
        state.promotions.transition_lock.try_lock().is_err(),
        "directed ingestion released the transition boundary during completion"
    );
    tokio::pin!(deployment);
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut deployment)
            .await
            .is_err(),
        "deployment overtook an admitted directed ingestion"
    );
    provider.release.as_ref().unwrap().notify_one();
    let (ingested, deployed) = tokio::join!(ingestion, deployment);
    assert_eq!(ingested.unwrap().0["chunks"], 1);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    deployed
}

pub(crate) async fn assert_materialized_fence(state: &AppState, space: &str) {
    let provider = Arc::new(CountingProvider::default());
    let mut directed_state = state.clone();
    directed_state.completion = Some(provider.clone());
    let before = state.backend.export_snapshot().await.unwrap();
    let err = directed(State(directed_state), None, Json(request(space)))
        .await
        .unwrap_err();
    assert!(
        matches!(err.0, CogniGraphError::DocumentConflict(_)),
        "{err:?}"
    );
    assert!(
        err.0
            .to_string()
            .contains("active M26 materialized generation")
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.backend.export_snapshot().await.unwrap(), before);
}
