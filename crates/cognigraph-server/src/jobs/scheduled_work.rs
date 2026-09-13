//! Scheduled work.

use super::*;

impl JobManager {
    pub(super) fn remove_scheduled(&self, tenant: &str, id: &str) {
        let mut scheduler = self.scheduler.lock().expect("job scheduler lock");
        if let Some(queue) = scheduler.by_tenant.get_mut(tenant) {
            queue.retain(|work| work.id != id);
            if queue.is_empty() {
                scheduler.by_tenant.remove(tenant);
                scheduler.rotation.retain(|queued| queued != tenant);
            }
        }
        let key = (tenant.to_string(), id.to_string());
        if scheduler.running.as_ref() != Some(&key) {
            self.scheduled
                .lock()
                .expect("job schedule lock")
                .remove(&key);
        }
    }

    pub(super) fn park_scheduled_tenant(&self, tenant: &str) {
        let mut scheduler = self.scheduler.lock().expect("job scheduler lock");
        let removed = scheduler.by_tenant.remove(tenant).unwrap_or_default();
        scheduler.rotation.retain(|queued| queued != tenant);
        let running = scheduler.running.clone();
        drop(scheduler);
        let mut scheduled = self.scheduled.lock().expect("job schedule lock");
        for work in removed {
            let key = (work.tenant, work.id);
            if running.as_ref() != Some(&key) {
                scheduled.remove(&key);
            }
        }
    }
}
