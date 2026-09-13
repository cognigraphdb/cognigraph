//! Serialize neuron validation and publication in the supported single-writer
//! server. Provider calls never hold this lock. Request incarnations keep a
//! retired tenant's in-flight work separate from its replacement.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::tenancy::{current_tenant, request_incarnation};

type Scopes = HashMap<(String, Option<String>), Weak<AsyncMutex<()>>>;

#[derive(Default)]
pub(crate) struct NeuronLifecycle {
    scopes: Mutex<Scopes>,
}

impl NeuronLifecycle {
    pub async fn lock(&self) -> OwnedMutexGuard<()> {
        let tenant = current_tenant();
        let key = (tenant.clone(), request_incarnation(&tenant));
        let scope = {
            let mut scopes = self.scopes.lock().expect("neuron lifecycle scopes lock");
            scopes.retain(|_, scope| scope.strong_count() > 0);
            let slot = scopes.entry(key).or_default();
            slot.upgrade().unwrap_or_else(|| {
                let scope = Arc::new(AsyncMutex::new(()));
                *slot = Arc::downgrade(&scope);
                scope
            })
        };
        scope.lock_owned().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tenancy::TenantContext;

    #[tokio::test]
    async fn locks_are_shared_only_within_the_admitted_tenant_incarnation() {
        let lifecycle = NeuronLifecycle::default();
        let context = |tenant: &str, incarnation: &str| {
            TenantContext::admitted(tenant.into(), incarnation.into(), None).unwrap()
        };
        let held = context("a", "first").scope(lifecycle.lock()).await;
        let same = context("a", "first").scope(lifecycle.lock());
        tokio::pin!(same);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut same)
                .await
                .is_err()
        );
        for (tenant, incarnation) in [("b", "first"), ("a", "replacement")] {
            let independent = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                context(tenant, incarnation).scope(lifecycle.lock()),
            )
            .await
            .unwrap();
            drop(independent);
        }
        drop(held);
        drop(same.await);
        drop(context("a", "first").scope(lifecycle.lock()).await);
        assert_eq!(
            lifecycle.scopes.lock().unwrap().len(),
            1,
            "unused scopes are pruned"
        );
    }
}
