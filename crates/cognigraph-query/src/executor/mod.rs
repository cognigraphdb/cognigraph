mod eval;
mod explain;
mod materialize;
mod mutation;
mod pipeline;
mod resolve;
mod run;

use std::collections::HashMap;

use serde_json::Value;
use thiserror::Error;

use crate::planner::{LogicalPlan, PlanError, parse_and_plan};
use cognigraph_core::{CogniGraphError, GraphBackend};

use materialize::materialize_memory;
use mutation::execute_mutation;
use run::{RunCx, run_plan};

/// Execute under EXPLAIN ANALYZE (backend): run fully instrumented and
/// return the statistics report instead of the rows.
async fn analyze_backend(
    plan: &LogicalPlan,
    backend: &dyn GraphBackend,
    bind_vars: &BindVars,
    budget: ExecutionBudget,
    deadline: Deadline,
) -> Result<Vec<Value>, ExecutionError> {
    let cx = RunCx::new(bind_vars, deadline, &budget).with_analyze(plan);
    let started = std::time::Instant::now();
    let results = resolve::execute_read(plan, backend, &cx).await?;
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
    let source_rows = cx.used_rows.load(std::sync::atomic::Ordering::Relaxed);
    let state = cx
        .analyze
        .as_ref()
        .and_then(|a| a.lock().ok())
        .ok_or_else(|| ExecutionError::Backend("analyze state poisoned".into()))?;
    Ok(explain::analyze_report(
        plan,
        &state,
        total_ms,
        source_rows,
        results.len(),
    ))
}

/// Execute under EXPLAIN ANALYZE (in-memory).
fn analyze_memory(
    plan: &LogicalPlan,
    dataset: &InMemoryDataset,
    bind_vars: &BindVars,
) -> Result<Vec<Value>, ExecutionError> {
    let budget = ExecutionBudget::default();
    let cx = RunCx::new(bind_vars, Deadline(None), &budget)
        .with_analyze(plan)
        .with_documents(dataset)
        .with_traversals(dataset);
    let started = std::time::Instant::now();
    let mut materialized = materialize_memory(plan, dataset, &cx)?;
    let mut site = 0usize;
    let results = run_plan(plan, Env::new(), &mut materialized, &mut site, &cx, true)?;
    let total_ms = started.elapsed().as_secs_f64() * 1000.0;
    let source_rows = cx.used_rows.load(std::sync::atomic::Ordering::Relaxed);
    let state = cx
        .analyze
        .as_ref()
        .and_then(|a| a.lock().ok())
        .ok_or_else(|| ExecutionError::Backend("analyze state poisoned".into()))?;
    Ok(explain::analyze_report(
        plan,
        &state,
        total_ms,
        source_rows,
        results.len(),
    ))
}

/// In-memory dataset used to test CGQL semantics without a storage backend.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InMemoryDataset {
    collections: HashMap<String, Vec<Value>>,
    /// `_id` -> document, built on the first `DOCUMENT()` call.
    document_index: std::sync::OnceLock<HashMap<String, Value>>,
}

impl InMemoryDataset {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_collection(mut self, name: impl Into<String>, rows: Vec<Value>) -> Self {
        self.collections.insert(name.into(), rows);
        self
    }

    pub fn insert_collection(&mut self, name: impl Into<String>, rows: Vec<Value>) {
        self.collections.insert(name.into(), rows);
    }

    fn collection(&self, name: &str) -> Result<&[Value], ExecutionError> {
        self.collections
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| ExecutionError::CollectionNotFound(name.to_string()))
    }

    fn vertex_by_id(&self, id: &str) -> Option<Value> {
        self.collections
            .values()
            .flat_map(|rows| rows.iter())
            .find(|row| row.get("_id").and_then(Value::as_str) == Some(id))
            .cloned()
    }
}

/// In-memory execution error.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ExecutionError {
    #[error(transparent)]
    Plan(#[from] PlanError),

    #[error("collection `{0}` not found")]
    CollectionNotFound(String),

    #[error("bind variable `{0}` not found")]
    BindVariableNotFound(String),

    #[error("missing bind variables: {0}")]
    MissingBindVariables(String),

    #[error("unexpected bind variables: {0}")]
    UnexpectedBindVariables(String),

    #[error("identifier `{0}` not found")]
    IdentifierNotFound(String),

    #[error("expected numeric value")]
    ExpectedNumber,

    #[error("expected boolean value")]
    ExpectedBool,

    #[error("expected vector value")]
    ExpectedVector,

    #[error("unsupported function `{0}`")]
    UnsupportedFunction(String),

    #[error("backend execution error: {0}")]
    Backend(String),

    /// Preserve unavailable backends so API layers can return HTTP 503.
    #[error("backend connection error: {0}")]
    Connection(String),

    /// A backend facade rejected access to protected data. Keep this typed so
    /// API layers can return 403 instead of flattening it into a generic query
    /// failure.
    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("mutations are not allowed on this endpoint")]
    MutationNotAllowed,

    #[error("mutations require a storage backend")]
    MutationUnsupported,

    #[error("mutation key must be a string")]
    InvalidDocumentKey,

    #[error("FOR source must be an array")]
    ExpectedArraySource,

    #[error("query exceeded the source row budget ({0} rows)")]
    RowBudgetExceeded(u64),

    #[error("query exceeded its time budget")]
    TimeBudgetExceeded,
}

/// Whether a CGQL execution is allowed to modify data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    ReadOnly,
    ReadWrite,
}

/// Per-query execution budget for untrusted callers. `None` = unlimited.
#[derive(Debug, Clone, Copy, Default)]
pub struct ExecutionBudget {
    /// Query-wide cap on materialized rows, backend document lookup attempts,
    /// and runtime array/correlated-traversal expansions, including retries.
    pub max_source_rows: Option<u64>,
    /// Wall-clock budget for the whole execution.
    pub time_budget_ms: Option<u64>,
}

#[derive(Clone, Copy)]
struct Deadline(Option<std::time::Instant>);

impl Deadline {
    fn start(budget: &ExecutionBudget) -> Self {
        Self(
            budget
                .time_budget_ms
                .map(|ms| std::time::Instant::now() + std::time::Duration::from_millis(ms)),
        )
    }

    fn check(&self) -> Result<(), ExecutionError> {
        if self
            .0
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            return Err(ExecutionError::TimeBudgetExceeded);
        }
        Ok(())
    }
}

type BindVars = HashMap<String, Value>;
type Env = HashMap<String, Value>;

pub(super) fn backend_execution_error(error: CogniGraphError) -> ExecutionError {
    match error {
        CogniGraphError::Forbidden(message) => ExecutionError::Forbidden(message),
        CogniGraphError::ConnectionError(message) => ExecutionError::Connection(message),
        other => ExecutionError::Backend(other.to_string()),
    }
}

/// Parse, validate, plan, and execute a CGQL query against an in-memory dataset.
pub fn parse_and_execute(
    input: &str,
    dataset: &InMemoryDataset,
    bind_vars: &BindVars,
) -> Result<Vec<Value>, ExecutionError> {
    let plan = parse_and_plan(input)?;
    execute_plan(&plan, dataset, bind_vars)
}

/// Parse, validate, plan, and execute a CGQL query against a GraphBackend.
pub async fn parse_and_execute_backend(
    input: &str,
    backend: &dyn GraphBackend,
    bind_vars: &BindVars,
) -> Result<Vec<Value>, ExecutionError> {
    parse_and_execute_backend_with_mode(input, backend, bind_vars, QueryMode::ReadOnly).await
}

/// Parse, validate, plan, and execute with an explicit read/write mode.
pub async fn parse_and_execute_backend_with_mode(
    input: &str,
    backend: &dyn GraphBackend,
    bind_vars: &BindVars,
    mode: QueryMode,
) -> Result<Vec<Value>, ExecutionError> {
    parse_and_execute_backend_with_options(
        input,
        backend,
        bind_vars,
        mode,
        ExecutionBudget::default(),
    )
    .await
}

/// Full-control entry point: mode plus a per-query execution budget.
pub async fn parse_and_execute_backend_with_options(
    input: &str,
    backend: &dyn GraphBackend,
    bind_vars: &BindVars,
    mode: QueryMode,
    budget: ExecutionBudget,
) -> Result<Vec<Value>, ExecutionError> {
    // Parsing/planning consumes the caller's remaining wall-clock allowance;
    // starting after it would extend a Lua script's shared deadline.
    let deadline = Deadline::start(&budget);
    let plan = parse_and_plan(input)?;
    // Plain EXPLAIN never touches data or bind variables — a mutation
    // under EXPLAIN is described, not executed, so read-only mode is fine.
    // EXPLAIN ANALYZE executes (validation already rejected mutations),
    // so bind variables and budgets apply as usual.
    if plan.explain && !plan.analyze {
        return Ok(explain::explain_plan(&plan));
    }
    if plan.analyze {
        check_bind_vars(&plan.bind_vars, bind_vars)?;
        return analyze_backend(&plan, backend, bind_vars, budget, deadline).await;
    }
    if plan.mutation.is_some() {
        if mode == QueryMode::ReadOnly {
            return Err(ExecutionError::MutationNotAllowed);
        }
        check_bind_vars(&plan.bind_vars, bind_vars)?;
        return execute_mutation(&plan, backend, bind_vars, &budget, deadline).await;
    }
    check_bind_vars(&plan.bind_vars, bind_vars)?;
    let cx = RunCx::new(bind_vars, deadline, &budget);
    resolve::execute_read(&plan, backend, &cx).await
}

/// Execute a logical CGQL plan against a GraphBackend.
pub async fn execute_backend_plan(
    plan: &LogicalPlan,
    backend: &dyn GraphBackend,
    bind_vars: &BindVars,
) -> Result<Vec<Value>, ExecutionError> {
    if plan.explain && !plan.analyze {
        return Ok(explain::explain_plan(plan));
    }
    if plan.mutation.is_some() {
        return Err(ExecutionError::MutationNotAllowed);
    }
    check_bind_vars(&plan.bind_vars, bind_vars)?;
    if plan.analyze {
        return analyze_backend(
            plan,
            backend,
            bind_vars,
            ExecutionBudget::default(),
            Deadline(None),
        )
        .await;
    }
    let budget = ExecutionBudget::default();
    let cx = RunCx::new(bind_vars, Deadline(None), &budget);
    resolve::execute_read(plan, backend, &cx).await
}

/// Execute a non-executing logical plan against an in-memory dataset.
pub fn execute_plan(
    plan: &LogicalPlan,
    dataset: &InMemoryDataset,
    bind_vars: &BindVars,
) -> Result<Vec<Value>, ExecutionError> {
    if plan.explain && !plan.analyze {
        return Ok(explain::explain_plan(plan));
    }
    if plan.mutation.is_some() {
        return Err(ExecutionError::MutationUnsupported);
    }
    check_bind_vars(&plan.bind_vars, bind_vars)?;
    if plan.analyze {
        return analyze_memory(plan, dataset, bind_vars);
    }
    let budget = ExecutionBudget::default();
    // The in-memory dataset holds everything, so DOCUMENT() resolves directly —
    // no fetch-and-retry needed on this path.
    let cx = RunCx::new(bind_vars, Deadline(None), &budget)
        .with_documents(dataset)
        .with_traversals(dataset);
    let mut materialized = materialize_memory(plan, dataset, &cx)?;
    let mut site = 0usize;
    run_plan(plan, Env::new(), &mut materialized, &mut site, &cx, true)
}

/// `DOCUMENT()` over the in-memory dataset: an id index built once on demand.
impl eval::DocumentSource for InMemoryDataset {
    fn get(&self, id: &str) -> Option<Value> {
        let index = self.document_index.get_or_init(|| {
            let mut index = HashMap::new();
            for rows in self.collections.values() {
                for row in rows {
                    if let Some(key) = row.get("_id").and_then(Value::as_str) {
                        index.insert(key.to_string(), row.clone());
                    }
                }
            }
            index
        });
        index.get(id).cloned()
    }
}

impl run::TraversalSource for InMemoryDataset {
    fn get(
        &self,
        _site: usize,
        start: &str,
        opts: &cognigraph_core::TraversalOpts,
    ) -> Option<Vec<run::TraversalHit>> {
        // The whole dataset is in hand, so a correlated traversal is answered
        // on the spot — no miss, and no second round.
        materialize::traversal_hits(
            self,
            start,
            &opts.edge_collection,
            from_core_direction(opts.direction),
            opts.min_depth,
            opts.max_depth,
        )
        .ok()
    }
}

fn from_core_direction(direction: cognigraph_core::Direction) -> crate::ast::Direction {
    match direction {
        cognigraph_core::Direction::Outbound => crate::ast::Direction::Outbound,
        cognigraph_core::Direction::Inbound => crate::ast::Direction::Inbound,
        cognigraph_core::Direction::Any => crate::ast::Direction::Any,
    }
}

/// Require the provided bind variables to match the query's declared set
/// exactly, before any storage access happens.
fn check_bind_vars(expected: &[String], provided: &BindVars) -> Result<(), ExecutionError> {
    let missing = expected
        .iter()
        .filter(|name| !provided.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ExecutionError::MissingBindVariables(missing.join(", ")));
    }

    let mut unexpected = provided
        .keys()
        .filter(|key| !expected.contains(key))
        .cloned()
        .collect::<Vec<_>>();
    if !unexpected.is_empty() {
        unexpected.sort();
        return Err(ExecutionError::UnexpectedBindVariables(
            unexpected.join(", "),
        ));
    }
    Ok(())
}
