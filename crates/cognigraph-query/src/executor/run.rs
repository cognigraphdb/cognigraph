//! The shared v2 pipeline: both engines execute the SAME synchronous body
//! walk; everything engine-specific happened earlier, in materialization
//! (see `materialize.rs`). Site numbering: every `PlanOp::For` consumes one
//! site id in pre-order (subquery plans included); materializer and runner
//! walk identically, so ids line up by construction.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use crate::planner::{LogicalPlan, PlanOp, PlanSource};

use super::eval::eval_expr;
use super::pipeline::{apply_collect_sort_limit, apply_projection};
use super::{BindVars, Deadline, Env, ExecutionBudget, ExecutionError};

/// Materialized rows per For site.
#[derive(Clone)]
pub(super) enum SiteRows {
    /// Collection scan results (documents; site-specific pushdown applied).
    Docs(Vec<Value>),
    /// The same documents plus a hash index on ONE attribute path, built the
    /// first time a correlated equality filters this site
    /// (decision_cgql_v2_workload_gaps.md, D1b).
    ///
    /// A correlated subquery re-runs its body per outer row against a site that
    /// was materialized once. Without this, each outer row expanded every
    /// document into an environment before the FILTER discarded almost all of
    /// them — O(outer x site) environment clones. Grouping once turns the
    /// per-row cost into a hash lookup.
    ///
    /// Only STRING values are bucketed. A string can never equal a non-string
    /// under `values_equal`, so restricting the index to strings lets the
    /// lookup skip every other document without changing which rows survive;
    /// a non-string correlated key falls back to the full expansion.
    IndexedDocs {
        docs: Vec<Value>,
        path: Vec<String>,
        buckets: HashMap<String, Vec<usize>>,
    },
    /// Vector-search / traversal results — full environments because those
    /// sources bind more than one variable (score, edge/path vars).
    Envs(Vec<Env>),
    /// A traversal whose start depends on the enclosing row (D2), so it cannot
    /// be materialized before the run. The site carries only its parameters;
    /// the rows are resolved per outer row against a `TraversalSource`.
    ///
    /// `bind_path` is false when the query never named a path variable. The
    /// path value is a whole serialized path per hit, so not building one that
    /// nothing can read is worth the flag.
    CorrelatedTraversal {
        opts: cognigraph_core::TraversalOpts,
        bind_edge: bool,
        bind_path: bool,
    },
}

/// One traversal result, already shaped into the three variables a traversal
/// binds. Kept separate from `Env` so a source can be shared by both engines.
#[derive(Clone)]
pub(super) struct TraversalHit {
    pub vertex: Value,
    pub edge: Value,
    pub path: Value,
}

/// Resolves a correlated traversal for one start vertex.
///
/// The in-memory engine answers immediately (it holds the whole dataset). The
/// backend engine cannot — `traverse` is async and row evaluation is not — so it
/// records a miss, the misses are fetched in one batch, and the run repeats,
/// exactly as `DOCUMENT()` does.
pub(super) trait TraversalSource: Send + Sync {
    fn get(
        &self,
        site: usize,
        start: &str,
        opts: &cognigraph_core::TraversalOpts,
    ) -> Option<Vec<TraversalHit>>;
    fn miss(&self, _site: usize, _start: &str) {}
}

impl SiteRows {
    fn docs(&self) -> Option<&Vec<Value>> {
        match self {
            SiteRows::Docs(docs) | SiteRows::IndexedDocs { docs, .. } => Some(docs),
            SiteRows::Envs(_) | SiteRows::CorrelatedTraversal { .. } => None,
        }
    }
}

/// Value at `path` inside `doc`, mirroring `eval_identifier`: a missing
/// segment yields null rather than an error.
fn doc_path_value<'a>(doc: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut value = doc;
    for part in path {
        value = value.get(part)?;
    }
    Some(value)
}

pub(super) type Materialized = HashMap<usize, SiteRows>;

/// Per-stage execution statistics: rows/runs from settled phases, plus all
/// attempted work including speculative backend-resolution passes. Correlated
/// subquery invocations are summed in both pairs of counters.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct StageStat {
    pub rows: u64,
    pub runs: u64,
    pub attempted_rows: u64,
    pub attempted_runs: u64,
}

/// Per-source statistics: rows materialized, fetch count, and fetch duration.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SiteStat {
    pub rows: u64,
    pub fetch_ms: f64,
    pub fetches: u64,
}

#[derive(Debug, Default)]
pub(super) struct AnalyzeState {
    pub sites: HashMap<usize, SiteStat>,
    pub stages: Vec<StageStat>,
    pub document_fetches: u64,
    pub document_fetch_ms: f64,
}

/// Shared execution context: bind variables, deadline, and the one
/// incremental row budget the whole query (subqueries included) draws from.
#[derive(Clone)]
pub(super) struct RunCx<'a> {
    pub bind_vars: &'a BindVars,
    /// Documents `DOCUMENT()` may read, when the caller supplies them.
    pub documents: Option<&'a dyn super::eval::DocumentSource>,
    /// Resolver for correlated traversals, when the caller supplies one.
    pub traversals: Option<&'a dyn TraversalSource>,
    pub deadline: Deadline,
    pub max_source_rows: Option<u64>,
    pub used_rows: std::sync::Arc<AtomicU64>,
    /// Present under EXPLAIN ANALYZE; None costs one branch per stage.
    pub analyze: Option<std::sync::Arc<std::sync::Mutex<AnalyzeState>>>,
}

impl<'a> RunCx<'a> {
    pub fn new(bind_vars: &'a BindVars, deadline: Deadline, budget: &ExecutionBudget) -> Self {
        Self {
            bind_vars,
            documents: None,
            traversals: None,
            deadline,
            max_source_rows: budget.max_source_rows,
            used_rows: std::sync::Arc::new(AtomicU64::new(0)),
            analyze: None,
        }
    }

    pub fn with_documents(mut self, source: &'a dyn super::eval::DocumentSource) -> Self {
        self.documents = Some(source);
        self
    }

    pub fn with_traversals(mut self, source: &'a dyn TraversalSource) -> Self {
        self.traversals = Some(source);
        self
    }

    /// The context expressions evaluate against.
    pub fn eval(&self) -> super::eval::EvalCtx<'_> {
        super::eval::EvalCtx {
            bind_vars: self.bind_vars,
            documents: self.documents,
        }
    }

    pub fn with_analyze(mut self, plan: &LogicalPlan) -> Self {
        self.analyze = Some(std::sync::Arc::new(std::sync::Mutex::new(AnalyzeState {
            stages: vec![StageStat::default(); stage_count(plan)],
            ..AnalyzeState::default()
        })));
        self
    }

    /// Only logical stage counts roll back on an unresolved read round. Source
    /// accounting and attempted stage work always retain every pass.
    pub fn stage_checkpoint(&self) -> Option<Vec<StageStat>> {
        self.analyze
            .as_ref()
            .and_then(|state| state.lock().ok())
            .map(|state| state.stages.clone())
    }

    pub fn discard_speculative_stages(&self, checkpoint: Option<Vec<StageStat>>) {
        if let (Some(analyze), Some(previous)) = (&self.analyze, checkpoint)
            && let Ok(mut state) = analyze.lock()
        {
            for (current, old) in state.stages.iter_mut().zip(previous) {
                current.rows = old.rows;
                current.runs = old.runs;
            }
        }
    }

    pub fn record_document_fetch(&self, fetch_ms: f64) {
        if let Some(analyze) = &self.analyze
            && let Ok(mut state) = analyze.lock()
        {
            state.document_fetches += 1;
            state.document_fetch_ms += fetch_ms;
        }
    }

    pub fn record_site(&self, site: usize, rows: usize, fetch_ms: f64) {
        if let Some(analyze) = &self.analyze
            && let Ok(mut state) = analyze.lock()
        {
            let stat = state.sites.entry(site).or_default();
            stat.rows += rows as u64;
            stat.fetch_ms += fetch_ms;
            stat.fetches += 1;
        }
    }

    pub(super) fn record_stage(&self, stage: usize, rows: usize) {
        if let Some(analyze) = &self.analyze
            && let Ok(mut state) = analyze.lock()
            && let Some(stat) = state.stages.get_mut(stage)
        {
            stat.rows += rows as u64;
            stat.runs += 1;
            stat.attempted_rows += rows as u64;
            stat.attempted_runs += 1;
        }
    }

    /// Charge produced/materialized source rows against the budget.
    pub fn charge(&self, rows: usize) -> Result<(), ExecutionError> {
        let used = self
            .used_rows
            .fetch_add(rows as u64, Ordering::Relaxed)
            .saturating_add(rows as u64);
        if let Some(max) = self.max_source_rows
            && used > max
        {
            return Err(ExecutionError::RowBudgetExceeded(max));
        }
        Ok(())
    }
}

/// Number of pipeline stages a plan contributes: one per body op (a LET
/// subquery adds its nested plan's stages after its own entry) plus the
/// tail clauses. Explain's stage labels are built in the same order.
pub(super) fn stage_count(plan: &LogicalPlan) -> usize {
    let body: usize = plan
        .body
        .iter()
        .map(|op| match op {
            PlanOp::LetSubquery { plan, .. } => 1 + stage_count(plan),
            _ => 1,
        })
        .sum();
    body + usize::from(plan.collect.is_some())
        + usize::from(plan.sort.is_some())
        + usize::from(plan.limit.is_some())
        + usize::from(plan.projection.is_some())
}

/// Number of For sites a plan consumes, nested subqueries included — used
/// to keep site numbering aligned across per-row subquery re-executions.
pub(super) fn count_sites(plan: &LogicalPlan) -> usize {
    plan.body
        .iter()
        .map(|op| match op {
            PlanOp::For { .. } => 1,
            PlanOp::LetSubquery { plan, .. } => count_sites(plan),
            _ => 0,
        })
        .sum()
}

/// Execute a full plan (body + tail) from an initial environment.
/// `single_pass` marks a body that executes exactly once for the query —
/// its sites may be consumed by move; a correlated subquery body re-runs
/// per outer row and must leave sites intact.
pub(super) fn run_plan(
    plan: &LogicalPlan,
    initial: Env,
    materialized: &mut Materialized,
    site: &mut usize,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<Vec<Value>, ExecutionError> {
    let mut stage = 0usize;
    run_plan_staged(
        plan,
        initial,
        materialized,
        site,
        &mut stage,
        cx,
        single_pass,
    )
}

fn run_plan_staged(
    plan: &LogicalPlan,
    initial: Env,
    materialized: &mut Materialized,
    site: &mut usize,
    stage: &mut usize,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<Vec<Value>, ExecutionError> {
    let head = run_plan_head_staged(plan, initial, materialized, site, stage, cx, single_pass)?;
    run_plan_deferred_tail(plan, head, materialized, cx, single_pass)
}

/// Rows that survived the head of a plan — the body before the deferred
/// suffix, then COLLECT/SORT/LIMIT — plus the positional counters the
/// deferred suffix and the projection will use. Splitting the plan here lets
/// the backend fetch-and-retry loop repeat only the phase that actually
/// recorded a miss.
#[derive(Clone)]
pub(super) struct HeadRows {
    pub rows: Vec<Env>,
    deferred_site: usize,
    deferred_stage: usize,
    projection_stage: usize,
}

/// Run a plan's head from a standing start (site and stage 0) — the entry
/// point the retry loop uses.
pub(super) fn run_plan_head(
    plan: &LogicalPlan,
    materialized: &mut Materialized,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<HeadRows, ExecutionError> {
    let mut site = 0usize;
    let mut stage = 0usize;
    run_plan_head_staged(
        plan,
        Env::new(),
        materialized,
        &mut site,
        &mut stage,
        cx,
        single_pass,
    )
}

fn run_plan_head_staged(
    plan: &LogicalPlan,
    initial: Env,
    materialized: &mut Materialized,
    site: &mut usize,
    stage: &mut usize,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<HeadRows, ExecutionError> {
    let split = deferred_split(plan);
    let main_ops = &plan.body[..split];
    let mut rows = run_ops(
        main_ops,
        plan,
        vec![initial],
        materialized,
        site,
        stage,
        cx,
        single_pass,
    )?;
    // The deferred ops keep their positional stage and site ids (the
    // materializer and stage_count both walk the body in order), so their
    // ranges are reserved here and used when they actually run — after the
    // tail has decided which rows survive.
    let deferred_site = *site;
    let deferred_stage = *stage;
    for op in &plan.body[split..] {
        match op {
            PlanOp::For { .. } => *site += 1,
            PlanOp::LetSubquery { plan: subplan, .. } => {
                *site += count_sites(subplan);
                *stage += stage_count(subplan);
            }
            _ => {}
        }
        *stage += 1;
    }
    apply_collect_sort_limit(plan, &mut rows, cx, stage)?;
    Ok(HeadRows {
        rows,
        deferred_site,
        deferred_stage,
        projection_stage: *stage,
    })
}

/// Run the deferred suffix and the projection over a head's surviving rows.
///
/// Takes the head by value so the ordinary single-run path moves the rows
/// through without a copy. The retry loop, which needs to call this more
/// than once, clones the head itself — and passes `single_pass: false` so
/// the deferred sites are borrowed rather than moved out of `materialized`.
pub(super) fn run_plan_deferred_tail(
    plan: &LogicalPlan,
    head: HeadRows,
    materialized: &mut Materialized,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<Vec<Value>, ExecutionError> {
    let split = deferred_split(plan);
    let deferred_ops = &plan.body[split..];
    let mut rows = head.rows;
    if !deferred_ops.is_empty() {
        let mut d_site = head.deferred_site;
        let mut d_stage = head.deferred_stage;
        rows = run_ops(
            deferred_ops,
            plan,
            rows,
            materialized,
            &mut d_site,
            &mut d_stage,
            cx,
            single_pass,
        )?;
    }
    let mut stage = head.projection_stage;
    apply_projection(plan, &mut rows, cx, &mut stage)
}

fn deferred_split(plan: &LogicalPlan) -> usize {
    plan.deferred_start
        .unwrap_or(plan.body.len())
        .min(plan.body.len())
}

/// Execute the positional body, producing the row environments the tail
/// clauses consume.
pub(super) fn run_body(
    plan: &LogicalPlan,
    initial: Env,
    materialized: &mut Materialized,
    site: &mut usize,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<Vec<Env>, ExecutionError> {
    let mut stage = 0usize;
    run_body_staged(
        plan,
        initial,
        materialized,
        site,
        &mut stage,
        cx,
        single_pass,
    )
}

fn run_body_staged(
    plan: &LogicalPlan,
    initial: Env,
    materialized: &mut Materialized,
    site: &mut usize,
    stage: &mut usize,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<Vec<Env>, ExecutionError> {
    run_ops(
        &plan.body,
        plan,
        vec![initial],
        materialized,
        site,
        stage,
        cx,
        single_pass,
    )
}

/// Execute a slice of body ops over a set of rows. The full body and the
/// deferred suffix both come through here, so the semantics of every op are
/// defined exactly once.
#[allow(clippy::too_many_arguments)]
fn run_ops(
    ops: &[PlanOp],
    plan: &LogicalPlan,
    initial_rows: Vec<Env>,
    materialized: &mut Materialized,
    site: &mut usize,
    stage: &mut usize,
    cx: &RunCx<'_>,
    single_pass: bool,
) -> Result<Vec<Env>, ExecutionError> {
    let mut rows = initial_rows;
    for op in ops {
        cx.deadline.check()?;
        match op {
            PlanOp::For { var, source } => {
                let this_site = *site;
                *site += 1;
                let hint = correlated_eq_hint(plan, var);
                rows = expand_for(
                    var,
                    source,
                    this_site,
                    rows,
                    materialized,
                    cx,
                    ExpandOpts {
                        single_pass,
                        correlated_eq: hint.as_ref(),
                    },
                )?;
                cx.record_stage(*stage, rows.len());
                *stage += 1;
            }
            PlanOp::Let(let_clause) => {
                for env in rows.iter_mut() {
                    cx.deadline.check()?;
                    let value = eval_expr(&let_clause.expr, env, &cx.eval())?;
                    env.insert(let_clause.name.clone(), value);
                }
                cx.record_stage(*stage, rows.len());
                *stage += 1;
            }
            PlanOp::LetSubquery {
                name,
                plan: subplan,
                correlated,
            } => {
                let sub_start = *site;
                let let_stage = *stage;
                let sub_stage_start = let_stage + 1;
                if *correlated {
                    // Per-row execution; every run consumes the same site
                    // and stage ids (stats aggregate across runs), so sites
                    // must survive: never single-pass.
                    for env in rows.iter_mut() {
                        let mut sub_site = sub_start;
                        let mut sub_stage = sub_stage_start;
                        let value = run_plan_staged(
                            subplan,
                            env.clone(),
                            materialized,
                            &mut sub_site,
                            &mut sub_stage,
                            cx,
                            false,
                        )?;
                        env.insert(name.clone(), Value::Array(value));
                    }
                } else {
                    // Runs once per enclosing execution — single-pass iff
                    // the enclosing body is.
                    let mut sub_site = sub_start;
                    let mut sub_stage = sub_stage_start;
                    let value = Value::Array(run_plan_staged(
                        subplan,
                        Env::new(),
                        materialized,
                        &mut sub_site,
                        &mut sub_stage,
                        cx,
                        single_pass,
                    )?);
                    for env in rows.iter_mut() {
                        env.insert(name.clone(), value.clone());
                    }
                }
                cx.record_stage(let_stage, rows.len());
                *site = sub_start + count_sites(subplan);
                *stage = sub_stage_start + stage_count(subplan);
            }
            PlanOp::Filter(filter) => {
                rows = std::mem::take(&mut rows)
                    .into_iter()
                    .filter_map(|env| {
                        match cx
                            .deadline
                            .check()
                            .and_then(|()| eval_expr(filter, &env, &cx.eval()))
                        {
                            Ok(Value::Bool(true)) => Some(Ok(env)),
                            // Null is falsy in filters so missing attributes
                            // exclude the row instead of failing the query.
                            Ok(Value::Bool(false)) | Ok(Value::Null) => None,
                            Ok(_) => Some(Err(ExecutionError::ExpectedBool)),
                            Err(err) => Some(Err(err)),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                cx.record_stage(*stage, rows.len());
                *stage += 1;
            }
        }
    }
    Ok(rows)
}

/// Convert a scan site to `IndexedDocs` keyed by `path`, if it is not already.
fn index_site(
    materialized: &mut Materialized,
    site: usize,
    path: &[String],
    collection: &str,
) -> Result<(), ExecutionError> {
    if let Some(SiteRows::IndexedDocs { path: existing, .. }) = materialized.get(&site)
        && existing == path
    {
        return Ok(());
    }
    let Some(entry) = materialized.remove(&site) else {
        return Err(ExecutionError::CollectionNotFound(collection.to_string()));
    };
    let docs = match entry {
        SiteRows::Docs(docs) | SiteRows::IndexedDocs { docs, .. } => docs,
        other => {
            materialized.insert(site, other);
            return Err(ExecutionError::CollectionNotFound(collection.to_string()));
        }
    };
    let mut buckets: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, doc) in docs.iter().enumerate() {
        if let Some(Value::String(key)) = doc_path_value(doc, path) {
            buckets.entry(key.clone()).or_default().push(i);
        }
    }
    materialized.insert(
        site,
        SiteRows::IndexedDocs {
            docs,
            path: path.to_vec(),
            buckets,
        },
    );
    Ok(())
}

/// `var.path == <expr>` (either side) among this plan's filters, where `<expr>`
/// does not itself reference `var` — i.e. a value fixed by the enclosing row.
///
/// Restricted to a top-level conjunct so the surviving FILTER still applies the
/// whole expression: the index only narrows candidates, it never decides.
fn correlated_eq_hint(plan: &LogicalPlan, var: &str) -> Option<(Vec<String>, crate::ast::Expr)> {
    for op in &plan.body {
        if let PlanOp::Filter(filter) = op
            && let Some(hint) = eq_conjunct(filter, var)
        {
            return Some(hint);
        }
    }
    None
}

fn eq_conjunct(expr: &crate::ast::Expr, var: &str) -> Option<(Vec<String>, crate::ast::Expr)> {
    use crate::ast::{BinaryOp, Expr};
    match expr {
        Expr::Binary {
            left,
            op: BinaryOp::And,
            right,
        } => eq_conjunct(left, var).or_else(|| eq_conjunct(right, var)),
        Expr::Binary {
            left,
            op: BinaryOp::Eq,
            right,
        } => scan_path(left, var)
            .filter(|_| !mentions_var(right, var))
            .map(|path| (path, (**right).clone()))
            .or_else(|| {
                scan_path(right, var)
                    .filter(|_| !mentions_var(left, var))
                    .map(|path| (path, (**left).clone()))
            }),
        _ => None,
    }
}

fn scan_path(expr: &crate::ast::Expr, var: &str) -> Option<Vec<String>> {
    let crate::ast::Expr::Identifier(parts) = expr else {
        return None;
    };
    (parts.len() >= 2 && parts[0] == var).then(|| parts[1..].to_vec())
}

/// Conservative: any mention of `var` anywhere disqualifies the expression as a
/// row-fixed key. Walking every variant would be exhaustive and brittle, so this
/// checks the rendered form — a false positive only costs the optimization.
fn mentions_var(expr: &crate::ast::Expr, var: &str) -> bool {
    fn walk(expr: &crate::ast::Expr, var: &str, found: &mut bool) {
        use crate::ast::Expr;
        if *found {
            return;
        }
        match expr {
            Expr::Identifier(parts) => {
                if parts.first().is_some_and(|p| p == var) {
                    *found = true;
                }
            }
            Expr::Binary { left, right, .. } => {
                walk(left, var, found);
                walk(right, var, found);
            }
            Expr::Unary { expr, .. } => walk(expr, var, found),
            Expr::FunctionCall { args, .. } => args.iter().for_each(|a| walk(a, var, found)),
            Expr::Array(items) => items.iter().for_each(|i| walk(i, var, found)),
            Expr::Object(fields) => fields.iter().for_each(|f| walk(&f.value, var, found)),
            // A subquery may close over anything; never treat it as row-fixed.
            Expr::Subquery(_) => *found = true,
            _ => {}
        }
    }
    let mut found = false;
    walk(expr, var, &mut found);
    found
}

/// How one `FOR` site may be expanded.
struct ExpandOpts<'a> {
    /// The site may be consumed by move (body runs exactly once).
    single_pass: bool,
    /// `var.path == <row-fixed expr>` to serve from a hash index (D1b).
    correlated_eq: Option<&'a (Vec<String>, crate::ast::Expr)>,
}

fn expand_for(
    var: &str,
    source: &PlanSource,
    this_site: usize,
    rows: Vec<Env>,
    materialized: &mut Materialized,
    cx: &RunCx<'_>,
    opts: ExpandOpts<'_>,
) -> Result<Vec<Env>, ExecutionError> {
    let ExpandOpts {
        single_pass,
        correlated_eq,
    } = opts;
    match source {
        PlanSource::CollectionScan { collection } => {
            // Single-pass bodies consume the site by move: the common
            // single-FOR scan binds each fetched document without cloning
            // it (measured ~25% on 10k-row scans; see benchmarks.md).
            if single_pass && rows.len() == 1 {
                let Some(SiteRows::Docs(docs)) = materialized.remove(&this_site) else {
                    return Err(ExecutionError::CollectionNotFound(collection.clone()));
                };
                let base = &rows[0];
                return Ok(docs
                    .into_iter()
                    .map(|doc| {
                        let mut next = base.clone();
                        next.insert(var.to_string(), doc);
                        next
                    })
                    .collect());
            }
            // A correlated equality against this site: index it once, then
            // serve each outer row by hash lookup instead of expanding the
            // whole site and letting FILTER throw it away.
            if let Some((path, expr)) = correlated_eq {
                index_site(materialized, this_site, path, collection)?;
                let Some(SiteRows::IndexedDocs { docs, buckets, .. }) =
                    materialized.get(&this_site)
                else {
                    return Err(ExecutionError::CollectionNotFound(collection.clone()));
                };
                let mut out = Vec::new();
                for env in &rows {
                    cx.deadline.check()?;
                    match eval_expr(expr, env, &cx.eval())? {
                        Value::String(key) => {
                            for &i in buckets.get(&key).map(Vec::as_slice).unwrap_or(&[]) {
                                let mut next = env.clone();
                                next.insert(var.to_string(), docs[i].clone());
                                out.push(next);
                            }
                        }
                        // Non-string key: the index cannot answer it, so fall
                        // back to the full expansion for this row only.
                        _ => {
                            for doc in docs {
                                let mut next = env.clone();
                                next.insert(var.to_string(), doc.clone());
                                out.push(next);
                            }
                        }
                    }
                }
                return Ok(out);
            }
            let Some(docs) = materialized.get(&this_site).and_then(SiteRows::docs) else {
                return Err(ExecutionError::CollectionNotFound(collection.clone()));
            };
            let mut out = Vec::with_capacity(rows.len().saturating_mul(docs.len()));
            for env in &rows {
                for doc in docs {
                    let mut next = env.clone();
                    next.insert(var.to_string(), doc.clone());
                    out.push(next);
                }
            }
            Ok(out)
        }
        PlanSource::Traversal {
            edge_var,
            path_var,
            start,
            ..
        } if matches!(
            materialized.get(&this_site),
            Some(SiteRows::CorrelatedTraversal { .. })
        ) =>
        {
            let Some(SiteRows::CorrelatedTraversal {
                opts,
                bind_edge,
                bind_path,
            }) = materialized.get(&this_site)
            else {
                unreachable!("guarded by the match arm")
            };
            let Some(source) = cx.traversals else {
                return Err(ExecutionError::Backend(
                    "a correlated traversal needs a traversal source".into(),
                ));
            };
            let mut out = Vec::new();
            for env in &rows {
                cx.deadline.check()?;
                // A start that is not an identifier string yields nothing for
                // this row, rather than failing the query: on a real graph an
                // edge endpoint can be absent, and one bad row should not take
                // the whole result with it.
                let Some(start_id) = eval_expr(start, env, &cx.eval())?
                    .as_str()
                    .map(str::to_string)
                else {
                    continue;
                };
                let Some(hits) = source.get(this_site, &start_id, opts) else {
                    source.miss(this_site, &start_id);
                    continue;
                };
                cx.charge(hits.len())?;
                for hit in hits {
                    let mut next = env.clone();
                    next.insert(var.to_string(), hit.vertex);
                    if *bind_edge {
                        next.insert(edge_var.to_string(), hit.edge);
                    }
                    if *bind_path {
                        next.insert(path_var.to_string(), hit.path);
                    }
                    out.push(next);
                }
            }
            Ok(out)
        }
        PlanSource::VectorSearch { .. } | PlanSource::Traversal { .. } => {
            // An uncorrelated traversal (and every VECTOR_SEARCH) is
            // materialized before the run. In the first-FOR case `rows` is the
            // single synthetic env, and the site moves out without a clone.
            let outermost = rows.len() == 1 && rows[0].is_empty();
            if outermost && single_pass {
                let Some(SiteRows::Envs(envs)) = materialized.remove(&this_site) else {
                    return Err(ExecutionError::Backend(
                        "special source was not materialized".into(),
                    ));
                };
                return Ok(envs);
            }
            let Some(SiteRows::Envs(envs)) = materialized.get(&this_site) else {
                return Err(ExecutionError::Backend(
                    "special source was not materialized".into(),
                ));
            };
            if outermost {
                return Ok(envs.clone());
            }
            // Since D2 an uncorrelated traversal may also sit in a non-first
            // FOR, where it joins against the rows above it. Merging rather
            // than replacing is what keeps those outer bindings visible.
            let mut out = Vec::with_capacity(rows.len().saturating_mul(envs.len()));
            for env in &rows {
                cx.deadline.check()?;
                for site_env in envs {
                    let mut next = env.clone();
                    next.extend(site_env.iter().map(|(k, v)| (k.clone(), v.clone())));
                    out.push(next);
                }
            }
            Ok(out)
        }
        PlanSource::VarRef(name) => {
            let mut out = Vec::new();
            for env in &rows {
                let value = env.get(name).cloned().unwrap_or(Value::Null);
                expand_array(var, value, env, &mut out, cx)?;
            }
            Ok(out)
        }
        PlanSource::Expression(expr) => {
            let mut out = Vec::new();
            for env in &rows {
                let value = eval_expr(expr, env, &cx.eval())?;
                expand_array(var, value, env, &mut out, cx)?;
            }
            Ok(out)
        }
    }
}

/// Iterate an array value into per-element environments. Null iterates as
/// empty (missing fields are common in documents); any other non-array is
/// an error rather than a silent skip.
fn expand_array(
    var: &str,
    value: Value,
    base: &Env,
    out: &mut Vec<Env>,
    cx: &RunCx<'_>,
) -> Result<(), ExecutionError> {
    match value {
        Value::Array(items) => {
            cx.charge(items.len())?;
            for item in items {
                let mut next = base.clone();
                next.insert(var.to_string(), item);
                out.push(next);
            }
            Ok(())
        }
        Value::Null => Ok(()),
        _ => Err(ExecutionError::ExpectedArraySource),
    }
}
