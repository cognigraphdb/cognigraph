//! Runs CGQL off the async runtime workers (CG-197).
//!
//! A query fetches its sources and then joins, groups and sorts without
//! yielding. On a runtime worker that makes every other request scheduled on
//! the worker wait, health checks included. Both query routes therefore hand
//! the whole query to a blocking thread, a bounded number at a time, and the
//! query stops when the request that asked for it is dropped.

use std::collections::HashMap;
use std::sync::Arc;

use cognigraph_core::CogniGraphError;
use cognigraph_query::{Cancellation, QueryMode};
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::error::AppError;
use crate::state::AppState;
use crate::tenancy::{TenantContext, current_tenant};

/// How many CGQL queries execute at once. A query beyond the limit waits for
/// a slot; its request timeout bounds the wait.
#[derive(Clone)]
pub struct CgqlSlots {
    permits: Arc<Semaphore>,
    limit: usize,
}

impl CgqlSlots {
    pub fn new(limit: usize) -> Self {
        let limit = limit.clamp(1, Semaphore::MAX_PERMITS);
        Self {
            permits: Arc::new(Semaphore::new(limit)),
            limit,
        }
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Slots no query holds right now.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn free(&self) -> usize {
        self.permits.available_permits()
    }
}

impl Default for CgqlSlots {
    fn default() -> Self {
        Self::new(default_concurrency())
    }
}

/// One query per CPU: the work is CPU-bound, so more would only take turns.
fn default_concurrency() -> usize {
    std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
}

/// `COGNIGRAPH_CGQL_MAX_CONCURRENT_QUERIES`: a positive number wins. Unset,
/// zero or unparsable, it is the number of CPUs; there is no unlimited.
pub fn concurrency(configured: Option<&str>) -> usize {
    configured
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|limit| *limit > 0)
        .unwrap_or_else(default_concurrency)
}

/// Dropping the request future tells its running query to stop.
struct CancelOnDrop(Cancellation);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// Execute one CGQL query for an HTTP request, under the server's budget.
///
/// The time budget starts when the query starts, not while it waits for a
/// slot. A read-write query sweeps cached results when it ends, whatever the
/// outcome and whether or not the request is still there: it may have
/// written before it failed or was cancelled.
pub(crate) async fn run(
    state: &AppState,
    query: String,
    bind_vars: HashMap<String, Value>,
    mode: QueryMode,
) -> Result<Vec<Value>, AppError> {
    let budget = state.cgql_budget;
    let permit = state
        .cgql_slots
        .permits
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| unfinished("the CGQL slots are closed"))?;
    let cancellation = Cancellation::new();
    let _cancel = CancelOnDrop(cancellation.clone());
    // A blocking thread is outside the request's tenant scope; without the
    // captured context the query would read the default tenant's store.
    let context = TenantContext::capture(current_tenant());
    let worker_context = context.clone();
    let backend = state.backend.clone();
    let runtime = tokio::runtime::Handle::current();
    let supervisor_state = state.clone();
    // The supervisor outlives a dropped request, so that the sweep below
    // still runs after a cancelled or timed-out write.
    let supervisor = tokio::spawn(context.scope(async move {
        let worker = tokio::task::spawn_blocking(move || {
            // Held until the work has really stopped, not until the request
            // has gone.
            let _permit = permit;
            runtime.block_on(worker_context.scope(async move {
                cognigraph_query::parse_and_execute_backend_cancellable(
                    &query,
                    backend.as_ref(),
                    &bind_vars,
                    mode,
                    budget,
                    &cancellation,
                )
                .await
            }))
        })
        .await;
        if mode == QueryMode::ReadWrite {
            supervisor_state.invalidate_search_results().await;
        }
        worker
    }));
    supervisor
        .await
        .map_err(|error| unfinished(&error.to_string()))?
        .map_err(|error| unfinished(&error.to_string()))?
        .map_err(|error| crate::cgql_errors::cgql_error(error, &budget))
}

/// The worker panicked or the runtime is shutting down: a server failure.
fn unfinished(reason: &str) -> AppError {
    tracing::error!(reason, "CGQL query did not finish");
    AppError(CogniGraphError::QueryError(
        "the query did not finish".into(),
    ))
}

#[cfg(test)]
#[path = "cgql_runner_tests.rs"]
mod tests;

#[cfg(all(test, feature = "enterprise"))]
#[path = "cgql_runner_tenant_tests.rs"]
mod tenant_tests;
