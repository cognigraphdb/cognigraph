use std::future::Future;
use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cognigraph_query::ExecutionBudget;
use tokio::runtime::Handle;
use tokio::sync::watch;

const INSTRUCTIONS: u8 = 1;
const TIME: u8 = 2;

/// Shared execution limits and cancellation for one Lua engine. Cancellation
/// is permanent; instruction/time allowances restart on each `execute` call.
#[derive(Clone)]
pub struct LuaExecutionControl(Arc<State>);

struct State {
    budget: ExecutionBudget,
    limit: AtomicU32,
    count: AtomicU64,
    stopped: AtomicU8,
    deadline: Mutex<Option<Instant>>,
    cancelled: watch::Sender<bool>,
}

impl LuaExecutionControl {
    pub fn new(budget: ExecutionBudget) -> Self {
        Self(Arc::new(State {
            budget,
            limit: AtomicU32::new(1_000_000),
            count: AtomicU64::new(0),
            stopped: AtomicU8::new(0),
            deadline: Mutex::new(None),
            cancelled: watch::channel(false).0,
        }))
    }

    /// Signal a dropped/timed-out request to its worker and pending callbacks.
    pub fn cancel(&self) {
        self.0.cancelled.send_replace(true);
    }

    pub(crate) fn start(&self) -> mlua::Result<()> {
        self.0.count.store(0, Ordering::Relaxed);
        self.0.stopped.store(0, Ordering::Relaxed);
        *self.0.deadline.lock().expect("Lua deadline lock") = self
            .0
            .budget
            .time_budget_ms
            .map(|ms| {
                Instant::now()
                    .checked_add(Duration::from_millis(ms))
                    .ok_or_else(|| mlua::Error::external("Lua time budget is too large"))
            })
            .transpose()?;
        self.check()
    }

    pub(crate) fn set_instruction_limit(&self, limit: u32) {
        self.0.limit.store(limit, Ordering::Relaxed);
    }

    fn deadline(&self) -> Option<Instant> {
        *self.0.deadline.lock().expect("Lua deadline lock")
    }

    pub(crate) fn check(&self) -> mlua::Result<()> {
        if *self.0.cancelled.borrow() {
            return Err(mlua::Error::external("Script execution cancelled"));
        }
        if self.deadline().is_some_and(|d| Instant::now() >= d) {
            self.0
                .stopped
                .compare_exchange(0, TIME, Ordering::Relaxed, Ordering::Relaxed)
                .ok();
        }
        match self.0.stopped.load(Ordering::Relaxed) {
            INSTRUCTIONS => Err(mlua::Error::external("Script exceeded instruction limit")),
            TIME => Err(mlua::Error::external("Script exceeded time budget")),
            _ => Ok(()),
        }
    }

    pub(crate) fn tick(&self) -> mlua::Result<()> {
        let count = self.0.count.fetch_add(1_000, Ordering::Relaxed) + 1_000;
        if count >= u64::from(self.0.limit.load(Ordering::Relaxed)) {
            self.0
                .stopped
                .compare_exchange(0, INSTRUCTIONS, Ordering::Relaxed, Ordering::Relaxed)
                .ok();
        }
        self.check()
    }

    /// Rows are per CGQL query; elapsed time is shared by the whole script.
    pub(crate) fn query_budget(&self) -> mlua::Result<ExecutionBudget> {
        self.check()?;
        Ok(ExecutionBudget {
            max_source_rows: self.0.budget.max_source_rows,
            time_budget_ms: self.deadline().map(|d| {
                d.saturating_duration_since(Instant::now())
                    .as_millis()
                    .min(u64::MAX as u128) as u64
            }),
        })
    }

    /// Enforce the same deadline/cancellation around every graph callback.
    /// Synchronous backend work is cooperative: it must yield or return before
    /// a timer can stop its future; no later Lua callback is admitted.
    pub(crate) fn block_on<F: Future>(
        &self,
        handle: &Handle,
        future: F,
    ) -> mlua::Result<F::Output> {
        self.check()?;
        let mut cancelled = self.0.cancelled.subscribe();
        let deadline = self.deadline();
        let output = handle.block_on(async {
            let timeout = async {
                match deadline {
                    Some(d) => tokio::time::sleep_until(d.into()).await,
                    None => std::future::pending().await,
                }
            };
            // Subscribe before checking, so a cancellation cannot be missed.
            self.check()?;
            tokio::select! {
                biased;
                _ = cancelled.changed() => Err(mlua::Error::external("Script execution cancelled")),
                _ = timeout => {
                    self.0.stopped.store(TIME, Ordering::Relaxed);
                    Err(mlua::Error::external("Script exceeded time budget"))
                }
                result = future => Ok(result),
            }
        })?;
        self.check()?;
        Ok(output)
    }
}

impl Default for LuaExecutionControl {
    fn default() -> Self {
        Self::new(ExecutionBudget::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    struct Dropped(Arc<AtomicBool>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancellation_drops_a_pending_backend_future() {
        let control = LuaExecutionControl::default();
        control.start().unwrap();
        let worker_control = control.clone();
        let dropped = Arc::new(AtomicBool::new(false));
        let worker_dropped = dropped.clone();
        let (entered, ready) = tokio::sync::oneshot::channel();
        let handle = Handle::current();
        let worker = tokio::task::spawn_blocking(move || {
            worker_control
                .block_on(&handle, async {
                    let _guard = Dropped(worker_dropped);
                    entered.send(()).unwrap();
                    std::future::pending::<()>().await;
                })
                .map_err(|e| e.to_string())
        });
        ready.await.unwrap();
        control.cancel();
        let err = tokio::time::timeout(Duration::from_secs(2), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(err.contains("cancelled"));
        assert!(dropped.load(Ordering::SeqCst));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn callbacks_share_one_absolute_deadline() {
        let handle = Handle::current();
        tokio::task::spawn_blocking(move || {
            let control = LuaExecutionControl::new(ExecutionBudget {
                time_budget_ms: Some(150),
                ..ExecutionBudget::default()
            });
            control.start().unwrap();
            let original = control.deadline();
            control
                .block_on(&handle, tokio::time::sleep(Duration::from_millis(30)))
                .unwrap();
            assert_eq!(control.deadline(), original);
            assert!(control.query_budget().unwrap().time_budget_ms.unwrap() < 150);
            let dropped = Arc::new(AtomicBool::new(false));
            let guard = Dropped(dropped.clone());
            let err = control
                .block_on(&handle, async move {
                    let _guard = guard;
                    std::future::pending::<()>().await;
                })
                .unwrap_err();
            assert!(err.to_string().contains("time budget"));
            assert!(dropped.load(Ordering::SeqCst));
            assert!(
                control.query_budget().is_err(),
                "new callbacks must not reset an expired deadline"
            );
        })
        .await
        .unwrap();
    }
}
