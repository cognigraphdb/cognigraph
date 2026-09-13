//! Manager.

use super::*;

pub struct JobManager {
    pub(super) raw_backend: Arc<dyn GraphBackend>,
    pub(super) transition_lock: AsyncMutex<()>,
    pub(super) scheduler: Mutex<FairQueue>,
    pub(super) scheduled: Mutex<HashSet<(String, String)>>,
    pub(super) active_jobs: Mutex<HashSet<(String, String, String)>>,
    pub(super) paused_tenants: Mutex<HashSet<String>>,
    pub(super) running_tenant: Mutex<Option<String>>,
    pub(super) shutting_down: AtomicBool,
    pub(super) batch_size: AtomicUsize,
    pub(super) max_active_total: AtomicUsize,
    pub(super) max_active_per_tenant: AtomicUsize,
    pub(super) retention_secs: AtomicU64,
    pub(super) archive_batch_size: AtomicUsize,
    pub(super) metrics: JobMetrics,
    pub(super) collection_errors: Mutex<HashMap<String, String>>,
    pub(super) data_errors: Mutex<HashMap<String, String>>,
    pub(super) catalog_errors: Mutex<HashMap<String, String>>,
    pub(super) last_reconciliation: Mutex<HashMap<String, ReconcileResult>>,
    #[cfg(test)]
    pub(super) worker_claims_paused: AtomicBool,
    #[cfg(test)]
    pub(super) panic_next_worker: AtomicBool,
    #[cfg(test)]
    pub(super) admission_reservations_paused: AtomicBool,
    #[cfg(test)]
    pub(super) admission_waiting: AtomicBool,
    #[cfg(test)]
    pub(super) cleanup_delay_ms: AtomicU64,
    #[cfg(test)]
    pub(super) artifact_consumption_delay_ms: AtomicU64,
    #[cfg(test)]
    pub(super) artifact_consumption_started: AtomicBool,
}

impl JobManager {
    pub fn new(raw_backend: Arc<dyn GraphBackend>, batch_size: usize) -> Self {
        Self {
            raw_backend,
            transition_lock: AsyncMutex::new(()),
            scheduler: Mutex::new(FairQueue::default()),
            scheduled: Mutex::new(HashSet::new()),
            active_jobs: Mutex::new(HashSet::new()),
            paused_tenants: Mutex::new(HashSet::new()),
            running_tenant: Mutex::new(None),
            shutting_down: AtomicBool::new(false),
            batch_size: AtomicUsize::new(batch_size.max(1)),
            max_active_total: AtomicUsize::new(1_000),
            max_active_per_tenant: AtomicUsize::new(100),
            retention_secs: AtomicU64::new(30 * 24 * 60 * 60),
            archive_batch_size: AtomicUsize::new(100),
            metrics: JobMetrics::default(),
            collection_errors: Mutex::new(HashMap::new()),
            data_errors: Mutex::new(HashMap::new()),
            catalog_errors: Mutex::new(HashMap::new()),
            last_reconciliation: Mutex::new(HashMap::new()),
            #[cfg(test)]
            worker_claims_paused: AtomicBool::new(false),
            #[cfg(test)]
            panic_next_worker: AtomicBool::new(false),
            #[cfg(test)]
            admission_reservations_paused: AtomicBool::new(false),
            #[cfg(test)]
            admission_waiting: AtomicBool::new(false),
            #[cfg(test)]
            cleanup_delay_ms: AtomicU64::new(0),
            #[cfg(test)]
            artifact_consumption_delay_ms: AtomicU64::new(0),
            #[cfg(test)]
            artifact_consumption_started: AtomicBool::new(false),
        }
    }

    pub fn set_batch_size(&self, batch_size: usize) {
        self.batch_size.store(batch_size.max(1), Ordering::Relaxed);
    }

    pub fn configure_governance(
        &self,
        max_active_total: usize,
        max_active_per_tenant: usize,
        retention_secs: u64,
        archive_batch_size: usize,
    ) {
        self.max_active_total
            .store(max_active_total.max(1), Ordering::Relaxed);
        self.max_active_per_tenant
            .store(max_active_per_tenant.max(1), Ordering::Relaxed);
        self.retention_secs.store(retention_secs, Ordering::Relaxed);
        self.archive_batch_size.store(
            archive_batch_size.clamp(1, MAX_JOB_OPERATOR_BATCH),
            Ordering::Relaxed,
        );
    }

    pub fn max_active_per_tenant(&self) -> usize {
        self.max_active_per_tenant.load(Ordering::Relaxed)
    }

    /// Merge tenant quota policy under the same barrier as final job
    /// admission. A successful lowering therefore linearizes either before a
    /// candidate reservation (which must observe it) or after an already
    /// accepted durable job; it can never be bypassed by a cached limit.
    pub async fn merge_tenant_quotas(
        &self,
        auth: &AuthProvider,
        tenant: &str,
        update: &serde_json::Map<String, Value>,
    ) -> Result<Tenant, CogniGraphError> {
        let _guard = self.transition_lock.lock().await;
        let current =
            auth.get_tenant(tenant)
                .await?
                .ok_or_else(|| CogniGraphError::DocumentNotFound {
                    collection: "tenants".into(),
                    key: tenant.into(),
                })?;
        let mut merged = current
            .quotas
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        for (key, value) in update {
            if value.is_null() {
                merged.remove(key);
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }
        if let Some(value) = merged.get("max_active_jobs") {
            let limit = value.as_u64().ok_or_else(|| {
                CogniGraphError::ValidationError(
                    "quota `max_active_jobs` must be a non-negative integer".into(),
                )
            })?;
            let host_limit = self.max_active_per_tenant.load(Ordering::Relaxed) as u64;
            if limit > host_limit {
                return Err(CogniGraphError::ValidationError(format!(
                    "quota `max_active_jobs` cannot exceed host limit {host_limit}"
                )));
            }
        }
        let quotas = (!merged.is_empty()).then_some(Value::Object(merged));
        auth.set_tenant_quotas(tenant, quotas).await
    }

    pub fn metrics_text(&self) -> String {
        let scheduler = self.scheduler.lock().expect("job scheduler lock");
        let ready_tenants = scheduler.rotation.len();
        let scheduled = self.scheduled.lock().expect("job schedule lock").len();
        drop(scheduler);
        let active = self.active_jobs.lock().expect("active jobs lock").len();
        self.metrics.render(active, scheduled, ready_tenants)
    }

    pub fn health(&self) -> Result<(), String> {
        let mut errors = self
            .collection_errors
            .lock()
            .expect("job collection health lock")
            .iter()
            .map(|(tenant, error)| format!("tenant `{tenant}` collection: {error}"))
            .collect::<Vec<_>>();
        errors.extend(
            self.data_errors
                .lock()
                .expect("job data health lock")
                .iter()
                .map(|(tenant, error)| format!("tenant `{tenant}` data: {error}")),
        );
        errors.extend(
            self.catalog_errors
                .lock()
                .expect("job catalog health lock")
                .iter()
                .map(|(tenant, error)| format!("tenant `{tenant}` catalog: {error}")),
        );
        if errors.is_empty() {
            Ok(())
        } else {
            errors.sort();
            Err(errors.join("; "))
        }
    }
}
