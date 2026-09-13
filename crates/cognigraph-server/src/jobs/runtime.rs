//! Runtime.

use super::*;

#[derive(Clone)]
pub(super) struct JobRuntime {
    pub(super) backend: Arc<dyn GraphBackend>,
    pub(super) side_views: Arc<crate::side_views::SideViews>,
    pub(super) cache: Option<Arc<dyn QueryCache>>,
    pub(super) auth: Option<Arc<AuthProvider>>,
    pub(super) promotions: Weak<crate::promotions::PromotionManager>,
    pub(super) artifact_cas: Option<Arc<crate::artifact_cas::LocalArtifactCas>>,
    pub(super) artifact_executable_digest: Option<String>,
    pub(super) embedder: Option<Arc<dyn EmbeddingProvider>>,
    pub(super) sideviews_completion: Option<Arc<dyn CompletionProvider>>,
    /// The general completion provider (`state.completion`), used by
    /// `construct.draft`. Deliberately NOT the side-view provider: drafting
    /// proposes governed vocabulary, side-view generation writes quarantined
    /// retrieval surfaces, and the two are configured independently.
    pub(super) completion: Option<Arc<dyn CompletionProvider>>,
}
impl JobRuntime {
    pub(super) fn from_state(state: &AppState) -> Self {
        Self {
            backend: state.managed_backend.clone(),
            side_views: state.side_views.clone(),
            cache: state.cache.clone(),
            auth: state.auth.clone(),
            promotions: Arc::downgrade(&state.promotions),
            artifact_cas: state.artifact_cas.clone(),
            artifact_executable_digest: state.artifact_executable_digest.clone(),
            embedder: state.embedder.clone(),
            sideviews_completion: state.sideviews_completion.clone(),
            completion: state.completion.clone(),
        }
    }

    pub(super) async fn invalidate_search_results(&self) {
        if let Some(cache) = self.cache.as_deref() {
            cache.invalidate_results().await;
        }
    }
}
#[derive(Clone)]
pub(super) struct ScheduledWork {
    pub(super) runtime: JobRuntime,
    pub(super) tenant: String,
    pub(super) id: String,
}
#[derive(Default)]
pub(super) struct FairQueue {
    pub(super) by_tenant: HashMap<String, VecDeque<ScheduledWork>>,
    pub(super) rotation: VecDeque<String>,
    pub(super) running: Option<(String, String)>,
    pub(super) dispatcher_alive: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DispatchOutcome {
    Complete,
    Continue,
    Parked,
}
