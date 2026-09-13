//! Manager.

use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct PromotionMutation<T> {
    pub record: T,
    pub replayed: bool,
    pub head_changed: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct PromotionPage<T> {
    pub records: Vec<T>,
    pub next_cursor: Option<String>,
}
#[derive(Default)]
pub(super) struct PromotionMetrics {
    pub(super) evidence_created: AtomicU64,
    pub(super) evidence_replayed: AtomicU64,
    pub(super) decisions_created: AtomicU64,
    pub(super) decisions_replayed: AtomicU64,
    pub(super) blocked: AtomicU64,
    pub(super) reconciliations: AtomicU64,
    pub(super) repairs: AtomicU64,
    pub(super) conflicts: AtomicU64,
    pub(super) m26_generations_created: AtomicU64,
    pub(super) m26_generation_replays: AtomicU64,
    pub(super) m26_deployments_created: AtomicU64,
    pub(super) m26_deployment_replays: AtomicU64,
    pub(super) m26_reconciliations: AtomicU64,
    pub(super) m26_repairs: AtomicU64,
}
/// Singleton M18 control-plane repository. The process-local transition lock
/// is intentional: CogniGraph does not claim multi-writer/HA coordination.
pub struct PromotionManager {
    pub(crate) backend: Arc<dyn GraphBackend>,
    pub(crate) jobs: Arc<JobManager>,
    pub(crate) transition_lock: tokio::sync::Mutex<()>,
    pub(crate) paused_tenants: Mutex<HashSet<String>>,
    pub(crate) errors: Mutex<HashMap<String, String>>,
    pub(crate) governance_root: std::sync::RwLock<Option<cognigraph_governance::VerificationKey>>,
    pub(super) metrics: PromotionMetrics,
}

impl PromotionManager {
    pub fn new(backend: Arc<dyn GraphBackend>, jobs: Arc<JobManager>) -> Self {
        Self {
            backend,
            jobs,
            transition_lock: tokio::sync::Mutex::new(()),
            paused_tenants: Mutex::new(HashSet::new()),
            errors: Mutex::new(HashMap::new()),
            governance_root: std::sync::RwLock::new(None),
            metrics: PromotionMetrics::default(),
        }
    }

    pub fn health(&self) -> Result<(), String> {
        let mut errors = self
            .errors
            .lock()
            .expect("promotion health lock")
            .iter()
            .map(|(tenant, error)| format!("tenant `{tenant}`: {error}"))
            .collect::<Vec<_>>();
        if errors.is_empty() {
            Ok(())
        } else {
            errors.sort();
            Err(errors.join("; "))
        }
    }

    pub fn retire_tenant(&self, tenant: &str) {
        self.errors
            .lock()
            .expect("promotion health lock")
            .remove(tenant);
    }

    /// Fence new promotion mutations and drain any request that entered before
    /// tenant suspension. The fence remains set until explicit recovery and
    /// resume, including across store quarantine and same-name recreation.
    pub async fn pause_tenant(&self, tenant: &str) {
        let _guard = self.transition_lock.lock().await;
        self.paused_tenants
            .lock()
            .expect("promotion tenant fence lock")
            .insert(tenant.into());
    }

    pub fn resume_tenant(&self, tenant: &str) {
        self.paused_tenants
            .lock()
            .expect("promotion tenant fence lock")
            .remove(tenant);
    }

    pub fn metrics_text(&self) -> String {
        format!(
            "# HELP cognigraph_promotion_evidence_total Immutable M18 evidence bundles created\n\
         # TYPE cognigraph_promotion_evidence_total counter\n\
         cognigraph_promotion_evidence_total {}\n\
         # HELP cognigraph_promotion_evidence_replays_total Idempotent evidence replays\n\
         # TYPE cognigraph_promotion_evidence_replays_total counter\n\
         cognigraph_promotion_evidence_replays_total {}\n\
         # HELP cognigraph_promotion_decisions_total Immutable M18 decisions created\n\
         # TYPE cognigraph_promotion_decisions_total counter\n\
         cognigraph_promotion_decisions_total {}\n\
         # HELP cognigraph_promotion_decision_replays_total Idempotent decision replays\n\
         # TYPE cognigraph_promotion_decision_replays_total counter\n\
         cognigraph_promotion_decision_replays_total {}\n\
         # HELP cognigraph_promotion_blocked_total Promotion attempts blocked by gates\n\
         # TYPE cognigraph_promotion_blocked_total counter\n\
         cognigraph_promotion_blocked_total {}\n\
         # HELP cognigraph_promotion_reconciliations_total Promotion-head reconciliation passes\n\
         # TYPE cognigraph_promotion_reconciliations_total counter\n\
         cognigraph_promotion_reconciliations_total {}\n\
         # HELP cognigraph_promotion_repairs_total Derived promotion heads repaired\n\
         # TYPE cognigraph_promotion_repairs_total counter\n\
         cognigraph_promotion_repairs_total {}\n\
         # HELP cognigraph_promotion_conflicts_total M18 idempotency or head conflicts\n\
         # TYPE cognigraph_promotion_conflicts_total counter\n\
         cognigraph_promotion_conflicts_total {}\n\
         # HELP cognigraph_semantic_repair_generations_total Immutable M26 generations created\n\
         # TYPE cognigraph_semantic_repair_generations_total counter\n\
         cognigraph_semantic_repair_generations_total {}\n\
         # HELP cognigraph_semantic_repair_generation_replays_total Idempotent M26 generation replays\n\
         # TYPE cognigraph_semantic_repair_generation_replays_total counter\n\
         cognigraph_semantic_repair_generation_replays_total {}\n\
         # HELP cognigraph_semantic_repair_deployments_total Signed M26 deployments created\n\
         # TYPE cognigraph_semantic_repair_deployments_total counter\n\
         cognigraph_semantic_repair_deployments_total {}\n\
         # HELP cognigraph_semantic_repair_deployment_replays_total Idempotent M26 deployment replays\n\
         # TYPE cognigraph_semantic_repair_deployment_replays_total counter\n\
         cognigraph_semantic_repair_deployment_replays_total {}\n\
         # HELP cognigraph_semantic_repair_reconciliations_total M26 full validation and recovery passes\n\
         # TYPE cognigraph_semantic_repair_reconciliations_total counter\n\
         cognigraph_semantic_repair_reconciliations_total {}\n\
         # HELP cognigraph_semantic_repair_repairs_total M26 derived heads or targets repaired\n\
         # TYPE cognigraph_semantic_repair_repairs_total counter\n\
         cognigraph_semantic_repair_repairs_total {}\n",
            self.metrics.evidence_created.load(Ordering::Relaxed),
            self.metrics.evidence_replayed.load(Ordering::Relaxed),
            self.metrics.decisions_created.load(Ordering::Relaxed),
            self.metrics.decisions_replayed.load(Ordering::Relaxed),
            self.metrics.blocked.load(Ordering::Relaxed),
            self.metrics.reconciliations.load(Ordering::Relaxed),
            self.metrics.repairs.load(Ordering::Relaxed),
            self.metrics.conflicts.load(Ordering::Relaxed),
            self.metrics.m26_generations_created.load(Ordering::Relaxed),
            self.metrics.m26_generation_replays.load(Ordering::Relaxed),
            self.metrics.m26_deployments_created.load(Ordering::Relaxed),
            self.metrics.m26_deployment_replays.load(Ordering::Relaxed),
            self.metrics.m26_reconciliations.load(Ordering::Relaxed),
            self.metrics.m26_repairs.load(Ordering::Relaxed),
        )
    }

    pub(crate) fn record_m26_generation(&self, replayed: bool) {
        let counter = if replayed {
            &self.metrics.m26_generation_replays
        } else {
            &self.metrics.m26_generations_created
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_m26_deployment(&self, replayed: bool) {
        let counter = if replayed {
            &self.metrics.m26_deployment_replays
        } else {
            &self.metrics.m26_deployments_created
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_m26_reconciliation(&self, repaired: usize) {
        self.metrics
            .m26_reconciliations
            .fetch_add(1, Ordering::Relaxed);
        self.metrics
            .m26_repairs
            .fetch_add(repaired as u64, Ordering::Relaxed);
    }
}
