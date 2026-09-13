//! Backend reads share one execution and resolution path with EXPLAIN ANALYZE.
//! Query accounting survives every head/tail retry; only logical stage counts
//! from unresolved rounds are discarded.

use std::collections::HashMap;

use cognigraph_core::GraphBackend;
use serde_json::Value;

use super::run::RunCx;
use super::{Env, ExecutionError, backend_execution_error, eval, materialize, run};
use crate::planner::LogicalPlan;

pub(super) async fn execute_read(
    plan: &LogicalPlan,
    backend: &dyn GraphBackend,
    cx: &RunCx<'_>,
) -> Result<Vec<Value>, ExecutionError> {
    let mut materialized = materialize::materialize_backend(plan, backend, cx).await?;
    if plan_reads_documents(plan) {
        return run_with_documents(plan, backend, cx, materialized).await;
    }
    let mut site = 0;
    run::run_plan(plan, Env::new(), &mut materialized, &mut site, cx, true)
}

/// Ids `DOCUMENT()` asked for but could not resolve, recorded during a run.
#[derive(Default)]
struct PendingDocuments {
    resolved: HashMap<String, Value>,
    missing: std::sync::Mutex<std::collections::BTreeSet<String>>,
}

impl eval::DocumentSource for PendingDocuments {
    fn get(&self, id: &str) -> Option<Value> {
        self.resolved.get(id).cloned()
    }
    fn miss(&self, id: &str) {
        if let Ok(mut missing) = self.missing.lock() {
            missing.insert(id.to_string());
        }
    }
}

/// Traversals a correlated `FOR` asked for but could not resolve (D2).
///
/// Keyed by site and start vertex: the depth, direction and edge collection are
/// fixed per site, so the start is all that varies. Distinct starts dedupe
/// naturally, which is what makes one traversal per *outer value* rather than
/// per outer row.
#[derive(Default)]
struct PendingTraversals {
    resolved: HashMap<(usize, String), Vec<run::TraversalHit>>,
    missing: std::sync::Mutex<std::collections::BTreeSet<(usize, String)>>,
}

impl run::TraversalSource for PendingTraversals {
    fn get(
        &self,
        site: usize,
        start: &str,
        _opts: &cognigraph_core::TraversalOpts,
    ) -> Option<Vec<run::TraversalHit>> {
        self.resolved.get(&(site, start.to_string())).cloned()
    }
    fn miss(&self, site: usize, start: &str) {
        if let Ok(mut missing) = self.missing.lock() {
            missing.insert((site, start.to_string()));
        }
    }
}

/// How many fetch-and-retry rounds a run may take.
///
/// One round resolves ordinary use. A second covers `DOCUMENT(DOCUMENT(x).ref)`
/// or a traversal starting from a vertex a previous traversal produced. Beyond
/// that the ids are being computed from fetched data without converging, and
/// looping forever on user input is not an option.
const MAX_DOCUMENT_ROUNDS: usize = 4;

/// Run a plan that reads from the backend mid-row: `DOCUMENT()` calls, a
/// correlated traversal, or both.
///
/// Row evaluation is synchronous by design (everything is fetched before it
/// starts), so a read discovered mid-row cannot be served inline. Instead the
/// run records what it could not resolve, those are fetched in one batch, and
/// the run repeats with them available. Reads are side-effect free, so repeating
/// is safe; only the set of resolved values changes between rounds.
async fn run_with_documents(
    plan: &LogicalPlan,
    backend: &dyn GraphBackend,
    shared: &RunCx<'_>,
    mut materialized: run::Materialized,
) -> Result<Vec<Value>, ExecutionError> {
    let mut docs = PendingDocuments::default();
    let mut hops = PendingTraversals::default();

    // The plan splits at the deferral boundary: the head (main body, COLLECT,
    // SORT, LIMIT) decides which rows survive; the tail (deferred LETs and the
    // projection) decorates them. Retrying repeats only the phase that missed,
    // and whether the head CAN miss is known statically — when it cannot, it
    // runs exactly once and pays no defensive clone of the sites.
    let (head, mut sites) = if head_reads_backend(plan) {
        let mut settled = None;
        for _ in 0..MAX_DOCUMENT_ROUNDS {
            // Sites are consumed by move on a single-pass run, so each head
            // round works on its own copy.
            shared.deadline.check()?;
            let checkpoint = shared.stage_checkpoint();
            let mut round = materialized.clone();
            let cx = shared.clone().with_documents(&docs).with_traversals(&hops);
            let head = run::run_plan_head(plan, &mut round, &cx, true)?;
            drop(cx);
            if !fetch_unresolved(backend, &mut docs, &mut hops, &materialized, shared).await? {
                settled = Some((head, round));
                break;
            }
            shared.discard_speculative_stages(checkpoint);
        }
        let Some(settled) = settled else {
            return Err(non_convergence());
        };
        settled
    } else {
        let cx = shared.clone().with_documents(&docs).with_traversals(&hops);
        let head = run::run_plan_head(plan, &mut materialized, &cx, true)?;
        drop(cx);
        (head, materialized)
    };

    // Tail rounds re-run only the deferred suffix and the projection over the
    // surviving rows. `single_pass: false` borrows the deferred sites instead
    // of consuming them, so every round works off the same map — the per-round
    // cost is a clone of the surviving rows, not of the materialized sites.
    for _ in 0..MAX_DOCUMENT_ROUNDS {
        shared.deadline.check()?;
        let checkpoint = shared.stage_checkpoint();
        let cx = shared.clone().with_documents(&docs).with_traversals(&hops);
        let out = run::run_plan_deferred_tail(plan, head.clone(), &mut sites, &cx, false)?;
        drop(cx);
        if !fetch_unresolved(backend, &mut docs, &mut hops, &sites, shared).await? {
            return Ok(out);
        }
        shared.discard_speculative_stages(checkpoint);
    }
    Err(non_convergence())
}

fn non_convergence() -> ExecutionError {
    ExecutionError::Backend(
        "a backend read did not resolve within the allowed rounds: an id or start \
         vertex is being derived from fetched data without converging"
            .into(),
    )
}

/// Fetch everything the last run recorded as missing. Returns whether there
/// was anything to fetch — false means the run is complete as it stands.
async fn fetch_unresolved(
    backend: &dyn GraphBackend,
    docs: &mut PendingDocuments,
    hops: &mut PendingTraversals,
    sites: &run::Materialized,
    cx: &RunCx<'_>,
) -> Result<bool, ExecutionError> {
    let wanted_docs: Vec<String> = docs
        .missing
        .lock()
        .map(|missing| missing.iter().cloned().collect())
        .unwrap_or_default();
    let unresolved_docs: Vec<String> = wanted_docs
        .into_iter()
        .filter(|id| !docs.resolved.contains_key(id))
        .collect();
    let wanted_hops: Vec<(usize, String)> = hops
        .missing
        .lock()
        .map(|missing| missing.iter().cloned().collect())
        .unwrap_or_default();
    let unresolved_hops: Vec<(usize, String)> = wanted_hops
        .into_iter()
        .filter(|key| !hops.resolved.contains_key(key))
        .collect();
    if unresolved_docs.is_empty() && unresolved_hops.is_empty() {
        return Ok(false);
    }

    for id in unresolved_docs {
        cx.deadline.check()?;
        let Some((collection, key)) = id.split_once('/') else {
            // Not an identifier: record it as unresolvable so the next round
            // does not ask again, and let DOCUMENT() answer null.
            docs.resolved.insert(id, Value::Null);
            continue;
        };
        // One unit per distinct backend lookup, including an absent document.
        // Charge before sending it so the cap bounds requests as well as hits.
        cx.charge(1)?;
        let started = std::time::Instant::now();
        let fetched = backend
            .get_document(collection, key)
            .await
            .map_err(backend_execution_error)?
            .unwrap_or(Value::Null);
        cx.deadline.check()?;
        cx.record_document_fetch(started.elapsed().as_secs_f64() * 1000.0);
        docs.resolved.insert(id, fetched);
    }
    for (site_id, start) in unresolved_hops {
        cx.deadline.check()?;
        let Some(run::SiteRows::CorrelatedTraversal {
            opts, bind_path, ..
        }) = sites.get(&site_id)
        else {
            return Err(ExecutionError::Backend(
                "correlated traversal site lost its parameters".into(),
            ));
        };
        // A start vertex that does not exist traverses to nothing, which is
        // the same answer the backend gives for a real vertex with no edges.
        let started = std::time::Instant::now();
        let paths = backend
            .traverse(&start, opts)
            .await
            .map_err(backend_execution_error)?;
        cx.deadline.check()?;
        cx.charge(paths.len())?;
        cx.record_site(
            site_id,
            paths.len(),
            started.elapsed().as_secs_f64() * 1000.0,
        );
        let bind_path = *bind_path;
        hops.resolved.insert(
            (site_id, start),
            paths
                .into_iter()
                .map(|path| hit_of_path(path, bind_path))
                .collect(),
        );
    }
    Ok(true)
}

/// Shape one backend traversal path into the variables a `FOR` binds.
///
/// Serializing the path is the most expensive part — it copies every vertex and
/// edge along the way — so it is skipped entirely when the query named no path
/// variable, which is the majority of traversals.
fn hit_of_path(path: cognigraph_core::TraversalPath, bind_path: bool) -> run::TraversalHit {
    let vertex = path.vertices.last().cloned().unwrap_or(Value::Null);
    let edge = path.edges.last().cloned().unwrap_or(Value::Null);
    let path_value = if bind_path {
        serde_json::to_value(path).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    run::TraversalHit {
        vertex,
        edge,
        path: path_value,
    }
}

/// Does this expression reach the backend mid-row — a `DOCUMENT()` call
/// anywhere inside it?
fn expr_reads_backend(expr: &crate::ast::Expr) -> bool {
    use crate::ast::Expr;
    match expr {
        Expr::FunctionCall { name, args } => {
            name.eq_ignore_ascii_case("DOCUMENT") || args.iter().any(expr_reads_backend)
        }
        // `DOCUMENT(x).name` is a Field over the call, so this arm is what
        // makes the common form detectable at all.
        Expr::Field { base, .. } => expr_reads_backend(base),
        Expr::Array(items) => items.iter().any(expr_reads_backend),
        Expr::Object(fields) => fields.iter().any(|f| expr_reads_backend(&f.value)),
        Expr::Unary { expr, .. } => expr_reads_backend(expr),
        Expr::Binary { left, right, .. } => expr_reads_backend(left) || expr_reads_backend(right),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_reads_backend(condition)
                || expr_reads_backend(then_branch)
                || expr_reads_backend(else_branch)
        }
        _ => false,
    }
}

/// Does this op reach the backend mid-row — `DOCUMENT()` in any of its
/// expressions, or a correlated traversal source?
fn op_reads_backend(op: &crate::planner::PlanOp) -> bool {
    use crate::planner::{PlanOp, PlanSource};
    match op {
        PlanOp::Filter(expr) => expr_reads_backend(expr),
        PlanOp::Let(clause) => expr_reads_backend(&clause.expr),
        PlanOp::LetSubquery { plan, .. } => plan_reads_documents(plan),
        // A correlated traversal resolves per row, through the same loop.
        PlanOp::For {
            source: PlanSource::Traversal { start, .. },
            ..
        } => materialize::is_correlated_start(start) || expr_reads_backend(start),
        PlanOp::For {
            source: PlanSource::Expression(expr),
            ..
        } => expr_reads_backend(expr),
        _ => false,
    }
}

/// True when the plan needs a backend read mid-row, and so must go through the
/// fetch-and-retry loop: a `DOCUMENT()` call anywhere, or a correlated traversal.
fn plan_reads_documents(plan: &LogicalPlan) -> bool {
    plan.body.iter().any(op_reads_backend)
        || plan
            .sort
            .as_ref()
            .is_some_and(|sort| sort.keys.iter().any(|key| expr_reads_backend(&key.expr)))
        || plan.projection.as_ref().is_some_and(expr_reads_backend)
        || plan.collect.as_ref().is_some_and(|collect| {
            collect
                .into
                .as_ref()
                .and_then(|into| into.projection.as_ref())
                .is_some_and(expr_reads_backend)
                || collect.bindings.iter().any(|b| expr_reads_backend(&b.expr))
                || collect
                    .aggregates
                    .iter()
                    .any(|a| expr_reads_backend(&a.expr))
        })
}

/// True when the HEAD of the plan — the ops before the deferral split, plus
/// COLLECT and SORT — can record a miss. When it cannot, the retry loop runs
/// the head exactly once, with no defensive clone of the sites; every miss
/// then belongs to the deferred suffix or the projection, which retry cheaply
/// over the surviving rows.
fn head_reads_backend(plan: &LogicalPlan) -> bool {
    let split = plan
        .deferred_start
        .unwrap_or(plan.body.len())
        .min(plan.body.len());
    plan.body[..split].iter().any(op_reads_backend)
        || plan
            .sort
            .as_ref()
            .is_some_and(|sort| sort.keys.iter().any(|key| expr_reads_backend(&key.expr)))
        || plan.collect.as_ref().is_some_and(|collect| {
            collect
                .into
                .as_ref()
                .and_then(|into| into.projection.as_ref())
                .is_some_and(expr_reads_backend)
                || collect.bindings.iter().any(|b| expr_reads_backend(&b.expr))
                || collect
                    .aggregates
                    .iter()
                    .any(|a| expr_reads_backend(&a.expr))
        })
}
