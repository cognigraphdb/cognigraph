//! Test controls.

use super::*;

impl JobManager {
    #[cfg(test)]
    pub(super) fn pause_worker_claims(&self) {
        self.worker_claims_paused.store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub(super) fn resume_worker_claims(&self) {
        self.worker_claims_paused.store(false, Ordering::Release);
    }

    #[cfg(test)]
    pub(super) fn panic_worker_once(&self) {
        self.panic_next_worker.store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub(super) fn pause_admission_reservations(&self) {
        self.admission_reservations_paused
            .store(true, Ordering::Release);
    }

    #[cfg(test)]
    pub(super) fn resume_admission_reservations(&self) {
        self.admission_reservations_paused
            .store(false, Ordering::Release);
    }

    #[cfg(test)]
    pub(super) async fn wait_for_admission_candidate(&self) {
        for _ in 0..1_000 {
            if self.admission_waiting.load(Ordering::Acquire) {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("job submission never reached the final admission boundary");
    }

    #[cfg(test)]
    pub(super) fn set_cleanup_delay_ms(&self, delay: u64) {
        self.cleanup_delay_ms.store(delay, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn set_artifact_consumption_delay_ms(&self, delay: u64) {
        self.artifact_consumption_delay_ms
            .store(delay, Ordering::Release);
        self.artifact_consumption_started
            .store(false, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) async fn wait_for_artifact_consumption_started(&self) {
        for _ in 0..1_000 {
            if self
                .artifact_consumption_started
                .swap(false, Ordering::AcqRel)
            {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("M21 worker never entered artifact consumption");
    }
}
