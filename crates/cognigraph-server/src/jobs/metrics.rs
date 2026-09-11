//! Metrics.

use super::*;

#[derive(Default)]
pub struct JobMetrics {
    pub(super) submitted: AtomicU64,
    pub(super) replayed: AtomicU64,
    pub(super) conflicts: AtomicU64,
    pub(super) started: AtomicU64,
    pub(super) checkpoints: AtomicU64,
    pub(super) succeeded: AtomicU64,
    pub(super) failed: AtomicU64,
    pub(super) canceled: AtomicU64,
    pub(super) cancel_requests: AtomicU64,
    pub(super) retries: AtomicU64,
    pub(super) recovered: AtomicU64,
    pub(super) running: AtomicU64,
    pub(super) cancel_requested: AtomicU64,
    pub(super) backpressured_tenant: AtomicU64,
    pub(super) backpressured_global: AtomicU64,
    pub(super) dispatches: AtomicU64,
    pub(super) archived: AtomicU64,
    pub(super) catalog_upserts: AtomicU64,
    pub(super) catalog_deletes: AtomicU64,
    pub(super) reconciliations: AtomicU64,
}
impl JobMetrics {
    pub fn render(&self, active: usize, scheduled: usize, ready_tenants: usize) -> String {
        let executing = self.running.load(Ordering::Relaxed);
        let cancel_requested = self.cancel_requested.load(Ordering::Relaxed);
        let running = executing.saturating_sub(cancel_requested);
        let queued = (active as u64).saturating_sub(executing);
        format!(
            "# TYPE cognigraph_job_submissions_total counter\n\
             cognigraph_job_submissions_total{{outcome=\"accepted\"}} {}\n\
             cognigraph_job_submissions_total{{outcome=\"replayed\"}} {}\n\
             cognigraph_job_submissions_total{{outcome=\"conflict\"}} {}\n\
             # TYPE cognigraph_jobs_started_total counter\n\
             cognigraph_jobs_started_total {}\n\
             # TYPE cognigraph_job_checkpoints_total counter\n\
             cognigraph_job_checkpoints_total {}\n\
             # TYPE cognigraph_jobs_completed_total counter\n\
             cognigraph_jobs_completed_total{{status=\"succeeded\"}} {}\n\
             cognigraph_jobs_completed_total{{status=\"failed\"}} {}\n\
             cognigraph_jobs_completed_total{{status=\"canceled\"}} {}\n\
             # TYPE cognigraph_job_cancel_requests_total counter\n\
             cognigraph_job_cancel_requests_total {}\n\
             # TYPE cognigraph_job_retries_total counter\n\
             cognigraph_job_retries_total {}\n\
             # TYPE cognigraph_job_recoveries_total counter\n\
             cognigraph_job_recoveries_total {}\n\
             # TYPE cognigraph_job_backpressure_total counter\n\
             cognigraph_job_backpressure_total{{scope=\"tenant\"}} {}\n\
             cognigraph_job_backpressure_total{{scope=\"global\"}} {}\n\
             # TYPE cognigraph_job_dispatches_total counter\n\
             cognigraph_job_dispatches_total {}\n\
             # TYPE cognigraph_jobs_archived_total counter\n\
             cognigraph_jobs_archived_total {}\n\
             # TYPE cognigraph_job_catalog_repairs_total counter\n\
             cognigraph_job_catalog_repairs_total{{action=\"upsert\"}} {}\n\
             cognigraph_job_catalog_repairs_total{{action=\"delete\"}} {}\n\
             # TYPE cognigraph_job_reconciliations_total counter\n\
             cognigraph_job_reconciliations_total {}\n\
             # TYPE cognigraph_jobs_active gauge\n\
             cognigraph_jobs_active{{status=\"queued\"}} {queued}\n\
             cognigraph_jobs_active{{status=\"running\"}} {running}\n\
             cognigraph_jobs_active{{status=\"cancel_requested\"}} {cancel_requested}\n\
             # TYPE cognigraph_job_scheduler_entries gauge\n\
             cognigraph_job_scheduler_entries {scheduled}\n\
             # TYPE cognigraph_job_scheduler_ready_tenants gauge\n\
             cognigraph_job_scheduler_ready_tenants {ready_tenants}\n",
            self.submitted.load(Ordering::Relaxed),
            self.replayed.load(Ordering::Relaxed),
            self.conflicts.load(Ordering::Relaxed),
            self.started.load(Ordering::Relaxed),
            self.checkpoints.load(Ordering::Relaxed),
            self.succeeded.load(Ordering::Relaxed),
            self.failed.load(Ordering::Relaxed),
            self.canceled.load(Ordering::Relaxed),
            self.cancel_requests.load(Ordering::Relaxed),
            self.retries.load(Ordering::Relaxed),
            self.recovered.load(Ordering::Relaxed),
            self.backpressured_tenant.load(Ordering::Relaxed),
            self.backpressured_global.load(Ordering::Relaxed),
            self.dispatches.load(Ordering::Relaxed),
            self.archived.load(Ordering::Relaxed),
            self.catalog_upserts.load(Ordering::Relaxed),
            self.catalog_deletes.load(Ordering::Relaxed),
            self.reconciliations.load(Ordering::Relaxed),
        )
    }
}
