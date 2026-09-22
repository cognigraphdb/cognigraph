//! Materialization: the engine-specific phase. Every For site (subquery
//! plans included, in pre-order) fetches its rows ONCE — decision 4's
//! materialize-once rule — with per-site projection/filter/limit pushdown
//! for backend collection scans. Pushed predicates are literal/bind-only,
//! so a site's materialization is valid for every (re-)execution of the
//! plan around it, correlated subqueries included.

use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};

use serde_json::{Value, json};

use crate::ast::{Direction, Expr, MutationClause};
use crate::planner::{LogicalPlan, PlanOp, PlanSource};
use cognigraph_core::{
    Direction as CoreDirection, FieldPredicate, GraphBackend, PredicateOp, TraversalOpts,
    VectorSearchOpts,
};

use super::backend_execution_error;
use super::eval::{EvalCtx, eval_expr};
use super::run::{Materialized, RunCx, SiteRows, TraversalHit};
use super::{BindVars, Env, ExecutionError, InMemoryDataset};
use crate::ast::LimitClause;

// ---------------------------------------------------------------------------
// Pushdown analysis (shared by materialization and EXPLAIN)
// ---------------------------------------------------------------------------

/// One pushable comparison, with bind variables left symbolic so the same
/// analysis serves both execution and EXPLAIN.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PushPredicate {
    pub path: Vec<String>,
    pub op: PredicateOp,
    pub value: PushValue,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum PushValue {
    Literal(Value),
    Bind(String),
}

/// What one collection-scan site can delegate to the backend.
#[derive(Debug, Clone)]
pub(super) struct ScanPushdown {
    /// Projection pushdown target, or None for full documents.
    pub fields: Option<Vec<String>>,
    /// Filter conjuncts the backend applies during the scan.
    pub predicates: Vec<PushPredicate>,
    /// True when every same-plan FILTER decomposed entirely into pushed
    /// conjuncts (the engine still re-applies them as a correctness belt).
    pub all_filters_pushed: bool,
    /// LIMIT pushdown (offset + count, resolved at run time): only for a
    /// plan whose body has exactly this one For, with no SORT/COLLECT and no
    /// residual filters.
    pub fetch_limit: Option<LimitClause>,
}

/// Analyze pushdown for the For op at `for_index` in `plan.body`.
///
/// Predicates come ONLY from this plan's own body filters: a filter inside
/// a nested subquery that references this var affects the subquery's rows,
/// not this source's — hoisting it here would delete outer rows. Projection
/// fields, by contrast, must include every access from nested plans too.
pub(super) fn analyze_scan(plan: &LogicalPlan, for_index: usize, var: &str) -> ScanPushdown {
    let fields = referenced_attributes(plan, var);
    let mut predicates = Vec::new();
    let mut all_filters_pushed = true;
    let mut for_count = 0usize;
    for op in &plan.body {
        match op {
            PlanOp::For { .. } => for_count += 1,
            PlanOp::Filter(filter) if !push_conjuncts(filter, var, &mut predicates) => {
                all_filters_pushed = false;
            }
            _ => {}
        }
    }
    let single_for = for_count == 1 && matches!(&plan.body[for_index], PlanOp::For { .. });
    let fetch_limit = match &plan.limit {
        Some(limit)
            if single_for
                && plan.sort.is_none()
                && plan.collect.is_none()
                && all_filters_pushed =>
        {
            Some(limit.clone())
        }
        _ => None,
    };
    ScanPushdown {
        fields,
        predicates,
        all_filters_pushed,
        fetch_limit,
    }
}

/// Decompose a filter into AND conjuncts, pushing every pushable one.
/// Returns true only if the WHOLE expression was pushed.
fn push_conjuncts(expr: &Expr, var: &str, out: &mut Vec<PushPredicate>) -> bool {
    if let Expr::Binary {
        left,
        op: crate::ast::BinaryOp::And,
        right,
    } = expr
    {
        // Pushable conjuncts can never error, so pushing one side while the
        // other stays residual is safe: the engine re-applies the full
        // expression to the surviving rows.
        let left_pushed = push_conjuncts(left, var, out);
        let right_pushed = push_conjuncts(right, var, out);
        return left_pushed && right_pushed;
    }
    if let Some(predicate) = as_push_predicate(expr, var) {
        out.push(predicate);
        return true;
    }
    false
}

/// `var.path <op> literal-or-bind` (either side), for ops whose evaluation
/// can never error and whose semantics `FieldPredicate::matches` mirrors.
fn as_push_predicate(expr: &Expr, var: &str) -> Option<PushPredicate> {
    use crate::ast::BinaryOp;
    let Expr::Binary { left, op, right } = expr else {
        return None;
    };
    // `var.path IN <literal-array-or-bind>` pushes as a membership
    // predicate (the flipped form — literal IN var.path — is array-contains
    // on a document field, a different operation, and stays in-engine).
    if matches!(op, BinaryOp::In)
        && let Some(path) = var_path(left, var)
        && let Some(value) = in_value(right)
    {
        return Some(PushPredicate {
            path,
            op: PredicateOp::In,
            value,
        });
    }
    let op = match op {
        BinaryOp::Eq => PredicateOp::Eq,
        BinaryOp::Ne => PredicateOp::Ne,
        BinaryOp::Lt => PredicateOp::Lt,
        BinaryOp::Le => PredicateOp::Le,
        BinaryOp::Gt => PredicateOp::Gt,
        BinaryOp::Ge => PredicateOp::Ge,
        _ => return None,
    };
    if let (Some(path), Some(value)) = (var_path(left, var), push_value(right)) {
        return Some(PushPredicate { path, op, value });
    }
    if let (Some(path), Some(value)) = (var_path(right, var), push_value(left)) {
        let flipped = match op {
            PredicateOp::Lt => PredicateOp::Gt,
            PredicateOp::Le => PredicateOp::Ge,
            PredicateOp::Gt => PredicateOp::Lt,
            PredicateOp::Ge => PredicateOp::Le,
            symmetric => symmetric,
        };
        return Some(PushPredicate {
            path,
            op: flipped,
            value,
        });
    }
    None
}

fn var_path(expr: &Expr, var: &str) -> Option<Vec<String>> {
    let Expr::Identifier(parts) = expr else {
        return None;
    };
    (parts.len() >= 2 && parts[0] == var).then(|| parts[1..].to_vec())
}

/// RHS of a pushable IN: a fully-literal array or a bind variable. A bind
/// resolving to a non-array matches nothing — exactly like the evaluator.
fn in_value(expr: &Expr) -> Option<PushValue> {
    match expr {
        Expr::BindVar(name) => Some(PushValue::Bind(name.clone())),
        Expr::Array(items) => {
            let values: Option<Vec<Value>> = items
                .iter()
                .map(|item| match push_value(item)? {
                    PushValue::Literal(value) => Some(value),
                    PushValue::Bind(_) => None,
                })
                .collect();
            values.map(|v| PushValue::Literal(Value::Array(v)))
        }
        _ => None,
    }
}

fn push_value(expr: &Expr) -> Option<PushValue> {
    match expr {
        Expr::String(v) => Some(PushValue::Literal(Value::String(v.clone()))),
        Expr::Number(v) => Some(PushValue::Literal(json!(v))),
        Expr::Bool(v) => Some(PushValue::Literal(Value::Bool(*v))),
        Expr::Null => Some(PushValue::Literal(Value::Null)),
        Expr::BindVar(name) => Some(PushValue::Bind(name.clone())),
        _ => None,
    }
}

fn resolve_predicates(
    predicates: &[PushPredicate],
    bind_vars: &BindVars,
) -> Result<Vec<FieldPredicate>, ExecutionError> {
    predicates
        .iter()
        .map(|p| {
            let value = match &p.value {
                PushValue::Literal(value) => value.clone(),
                PushValue::Bind(name) => bind_vars
                    .get(name)
                    .cloned()
                    .ok_or_else(|| ExecutionError::BindVariableNotFound(name.clone()))?,
            };
            Ok(FieldPredicate {
                path: p.path.clone(),
                op: p.op,
                value,
            })
        })
        .collect()
}

/// A `var._score >= x` (or `> x`) conjunct anywhere in this plan's body
/// filters, pushable as the vector-search threshold: top-K-above-threshold
/// equals filtering top-K, and the engine re-applies the filter anyway
/// (strict `>` fetches the boundary row; the belt drops it).
pub(super) fn vector_threshold(plan: &LogicalPlan, var: &str) -> Option<PushValue> {
    for op in &plan.body {
        if let PlanOp::Filter(filter) = op
            && let Some(value) = threshold_conjunct(filter, var)
        {
            return Some(value);
        }
    }
    None
}

fn threshold_conjunct(expr: &Expr, var: &str) -> Option<PushValue> {
    if let Expr::Binary {
        left,
        op: crate::ast::BinaryOp::And,
        right,
    } = expr
    {
        return threshold_conjunct(left, var).or_else(|| threshold_conjunct(right, var));
    }
    let predicate = as_push_predicate(expr, var)?;
    (matches!(predicate.op, PredicateOp::Ge | PredicateOp::Gt)
        && predicate.path == ["_score".to_string()])
    .then_some(predicate.value)
}

/// A `edge_var.confidence >= x` conjunct, pushable as the traversal's
/// min_confidence ONLY for exact depth 1..1: at deeper ranges, pruning
/// during expansion also removes paths THROUGH weak edges, which a
/// post-FILTER does not — different semantics, so no general push.
pub(super) fn traversal_min_confidence(plan: &LogicalPlan, edge_var: &str) -> Option<PushValue> {
    for op in &plan.body {
        if let PlanOp::Filter(filter) = op
            && let Some(value) = confidence_conjunct(filter, edge_var)
        {
            return Some(value);
        }
    }
    None
}

fn confidence_conjunct(expr: &Expr, edge_var: &str) -> Option<PushValue> {
    if let Expr::Binary {
        left,
        op: crate::ast::BinaryOp::And,
        right,
    } = expr
    {
        return confidence_conjunct(left, edge_var)
            .or_else(|| confidence_conjunct(right, edge_var));
    }
    let predicate = as_push_predicate(expr, edge_var)?;
    (matches!(predicate.op, PredicateOp::Ge) && predicate.path == ["confidence".to_string()])
        .then_some(predicate.value)
}

pub(super) fn resolve_push_number(
    value: &PushValue,
    bind_vars: &BindVars,
) -> Result<Option<f64>, ExecutionError> {
    let resolved = match value {
        PushValue::Literal(value) => value.clone(),
        PushValue::Bind(name) => bind_vars
            .get(name)
            .cloned()
            .ok_or_else(|| ExecutionError::BindVariableNotFound(name.clone()))?,
    };
    Ok(resolved.as_f64())
}

/// The top-level attributes of `var` the plan touches — INCLUDING nested
/// subquery plans (a correlated subquery may read `var.attr`) — or `None`
/// when the whole document is needed.
pub(super) fn referenced_attributes(plan: &LogicalPlan, var: &str) -> Option<Vec<String>> {
    let mut attrs = std::collections::BTreeSet::new();
    let mut whole = false;
    collect_plan_attrs(plan, var, &mut attrs, &mut whole);
    (!whole).then(|| attrs.into_iter().collect())
}

fn collect_plan_attrs(
    plan: &LogicalPlan,
    var: &str,
    attrs: &mut std::collections::BTreeSet<String>,
    whole: &mut bool,
) {
    for op in &plan.body {
        match op {
            PlanOp::For { source, .. } => match source {
                PlanSource::Expression(expr) => collect_attrs(expr, var, attrs, whole),
                PlanSource::VarRef(name) if name == var => *whole = true,
                PlanSource::VectorSearch { vector, .. } => collect_attrs(vector, var, attrs, whole),
                PlanSource::Traversal { start, .. } => collect_attrs(start, var, attrs, whole),
                _ => {}
            },
            PlanOp::Let(let_clause) => collect_attrs(&let_clause.expr, var, attrs, whole),
            PlanOp::LetSubquery { plan, .. } => collect_plan_attrs(plan, var, attrs, whole),
            PlanOp::Filter(filter) => collect_attrs(filter, var, attrs, whole),
        }
    }
    if let Some(collect) = &plan.collect {
        for binding in &collect.bindings {
            collect_attrs(&binding.expr, var, attrs, whole);
        }
        for aggregate in &collect.aggregates {
            collect_attrs(&aggregate.expr, var, attrs, whole);
        }
        if let Some(into) = &collect.into {
            match &into.projection {
                Some(projection) => collect_attrs(projection, var, attrs, whole),
                // Bare INTO captures the whole per-row environment.
                None => *whole = true,
            }
        }
    }
    if let Some(sort) = &plan.sort {
        for key in &sort.keys {
            collect_attrs(&key.expr, var, attrs, whole);
        }
    }
    if let Some(projection) = &plan.projection {
        collect_attrs(projection, var, attrs, whole);
    }
    if let Some(mutation) = &plan.mutation {
        match mutation {
            MutationClause::Insert { doc, .. } => collect_attrs(doc, var, attrs, whole),
            MutationClause::Update { key, with, .. }
            | MutationClause::Replace { key, with, .. } => {
                collect_attrs(key, var, attrs, whole);
                collect_attrs(with, var, attrs, whole);
            }
            MutationClause::Remove { key, .. } => collect_attrs(key, var, attrs, whole),
            MutationClause::Upsert {
                search,
                insert,
                update,
                ..
            } => {
                collect_attrs(search, var, attrs, whole);
                collect_attrs(insert, var, attrs, whole);
                collect_attrs(update, var, attrs, whole);
            }
        }
    }
}

fn collect_attrs(
    expr: &Expr,
    var: &str,
    attrs: &mut std::collections::BTreeSet<String>,
    whole: &mut bool,
) {
    match expr {
        Expr::Identifier(parts) => {
            if parts.first().is_some_and(|root| root == var) {
                match parts.get(1) {
                    Some(attr) => {
                        attrs.insert(attr.clone());
                    }
                    None => *whole = true,
                }
            }
        }
        Expr::Field { base, .. } => collect_attrs(base, var, attrs, whole),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_attrs(condition, var, attrs, whole);
            collect_attrs(then_branch, var, attrs, whole);
            collect_attrs(else_branch, var, attrs, whole);
        }
        Expr::Array(items) => items
            .iter()
            .for_each(|item| collect_attrs(item, var, attrs, whole)),
        Expr::Object(fields) => fields
            .iter()
            .for_each(|field| collect_attrs(&field.value, var, attrs, whole)),
        Expr::Unary { expr, .. } => collect_attrs(expr, var, attrs, whole),
        Expr::Binary { left, right, .. } => {
            collect_attrs(left, var, attrs, whole);
            collect_attrs(right, var, attrs, whole);
        }
        Expr::FunctionCall { args, .. } => args
            .iter()
            .for_each(|arg| collect_attrs(arg, var, attrs, whole)),
        Expr::Subquery(_) => {
            // Subqueries are pre-planned into PlanOp::LetSubquery; an Expr
            // subquery inside a plan expression cannot occur.
        }
        Expr::BindVar(_) | Expr::String(_) | Expr::Number(_) | Expr::Bool(_) | Expr::Null => {}
    }
}

// ---------------------------------------------------------------------------
// Backend materialization (async)
// ---------------------------------------------------------------------------

pub(super) async fn materialize_backend(
    plan: &LogicalPlan,
    backend: &dyn GraphBackend,
    cx: &RunCx<'_>,
) -> Result<Materialized, ExecutionError> {
    let mut sites = Materialized::new();
    let mut site = 0usize;
    materialize_backend_plan(plan, backend, cx, &mut sites, &mut site).await?;
    Ok(sites)
}

async fn materialize_backend_plan(
    plan: &LogicalPlan,
    backend: &dyn GraphBackend,
    cx: &RunCx<'_>,
    sites: &mut Materialized,
    site: &mut usize,
) -> Result<(), ExecutionError> {
    for (index, op) in plan.body.iter().enumerate() {
        match op {
            PlanOp::For { var, source } => {
                let this_site = *site;
                *site += 1;
                match source {
                    PlanSource::CollectionScan { collection } => {
                        let pushdown = analyze_scan(plan, index, var);
                        let fetch_limit = pushdown
                            .fetch_limit
                            .as_ref()
                            .map(|l| super::limit::resolve_limit(l, cx.bind_vars, plan.max_limit))
                            .transpose()?
                            .map(super::limit::ResolvedLimit::fetch_len);
                        let fetch_started = std::time::Instant::now();
                        let rows = if pushdown.predicates.is_empty() {
                            match &pushdown.fields {
                                Some(fields) => backend
                                    .list_documents_projected(collection, fields, fetch_limit, None)
                                    .await
                                    .map_err(backend_execution_error)?,
                                None => backend
                                    .list_documents(collection, fetch_limit, None)
                                    .await
                                    .map_err(backend_execution_error)?,
                            }
                        } else {
                            let predicates =
                                resolve_predicates(&pushdown.predicates, cx.bind_vars)?;
                            backend
                                .list_documents_filtered(
                                    collection,
                                    &predicates,
                                    pushdown.fields.as_deref(),
                                    fetch_limit,
                                    None,
                                )
                                .await
                                .map_err(backend_execution_error)?
                        };
                        cx.charge(rows.len())?;
                        cx.record_site(
                            this_site,
                            rows.len(),
                            fetch_started.elapsed().as_secs_f64() * 1000.0,
                        );
                        sites.insert(this_site, SiteRows::Docs(rows));
                    }
                    PlanSource::VectorSearch { collection, vector } => {
                        let fetch_started = std::time::Instant::now();
                        let query_vector =
                            value_to_vector(&eval_expr(vector, &Env::new(), &cx.eval())?)?;
                        let search_limit = plan
                            .limit
                            .as_ref()
                            .map(|l| super::limit::resolve_limit(l, cx.bind_vars, plan.max_limit))
                            .transpose()?
                            .map_or(100, super::limit::ResolvedLimit::fetch_len);
                        let threshold = match vector_threshold(plan, var) {
                            Some(value) => resolve_push_number(&value, cx.bind_vars)?,
                            None => None,
                        };
                        let opts = VectorSearchOpts {
                            threshold,
                            limit: search_limit,
                            model_name: None,
                        };
                        let hits = backend
                            .vector_search(collection, &query_vector, &opts)
                            .await
                            .map_err(backend_execution_error)?;
                        cx.charge(hits.len())?;
                        cx.record_site(
                            this_site,
                            hits.len(),
                            fetch_started.elapsed().as_secs_f64() * 1000.0,
                        );
                        let envs = hits
                            .into_iter()
                            .map(|hit| {
                                let mut document = hit.document;
                                if let Value::Object(obj) = &mut document {
                                    obj.insert("_score".to_string(), json!(hit.score));
                                }
                                env_with(var, document)
                            })
                            .collect();
                        sites.insert(this_site, SiteRows::Envs(envs));
                    }
                    PlanSource::Traversal {
                        edge_var,
                        path_var,
                        min_depth,
                        max_depth,
                        direction,
                        start,
                        edge_collection,
                    } => {
                        let fetch_started = std::time::Instant::now();
                        let min_confidence = if *min_depth == 1 && *max_depth == 1 {
                            match traversal_min_confidence(plan, edge_var) {
                                Some(value) => resolve_push_number(&value, cx.bind_vars)?,
                                None => None,
                            }
                        } else {
                            None
                        };
                        let opts = TraversalOpts {
                            max_depth: *max_depth,
                            min_depth: *min_depth,
                            direction: to_core_direction(*direction),
                            edge_collection: edge_collection.clone(),
                            min_confidence,
                            path_decay: 0.8,
                        };
                        // A start that reads the enclosing row has no value yet
                        // — it is resolved per row during the run (D2).
                        if is_correlated_start(start) {
                            sites.insert(
                                this_site,
                                SiteRows::CorrelatedTraversal {
                                    opts,
                                    bind_edge: !is_synthetic_var(edge_var),
                                    bind_path: !is_synthetic_var(path_var),
                                },
                            );
                            continue;
                        }
                        let start_vertex = eval_expr(start, &Env::new(), &cx.eval())?
                            .as_str()
                            .map(str::to_string)
                            .ok_or_else(|| {
                                ExecutionError::IdentifierNotFound("traversal start".into())
                            })?;
                        let paths = backend
                            .traverse(&start_vertex, &opts)
                            .await
                            .map_err(backend_execution_error)?;
                        cx.charge(paths.len())?;
                        cx.record_site(
                            this_site,
                            paths.len(),
                            fetch_started.elapsed().as_secs_f64() * 1000.0,
                        );
                        let envs = paths
                            .into_iter()
                            .map(|path| {
                                let vertex = path.vertices.last().cloned().unwrap_or(Value::Null);
                                let edge = path.edges.last().cloned().unwrap_or(Value::Null);
                                let path_value = serde_json::to_value(path).unwrap_or(Value::Null);
                                HashMap::from([
                                    (var.clone(), vertex),
                                    (edge_var.clone(), edge),
                                    (path_var.clone(), path_value),
                                ])
                            })
                            .collect();
                        sites.insert(this_site, SiteRows::Envs(envs));
                    }
                    // Runtime-only sources: nothing to materialize.
                    PlanSource::VarRef(_) | PlanSource::Expression(_) => {}
                }
            }
            PlanOp::LetSubquery { plan: subplan, .. } => {
                Box::pin(materialize_backend_plan(subplan, backend, cx, sites, site)).await?;
            }
            _ => {}
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// In-memory materialization (sync)
// ---------------------------------------------------------------------------

pub(super) fn materialize_memory(
    plan: &LogicalPlan,
    dataset: &InMemoryDataset,
    cx: &RunCx<'_>,
) -> Result<Materialized, ExecutionError> {
    let mut sites = Materialized::new();
    let mut site = 0usize;
    materialize_memory_plan(plan, dataset, cx, &mut sites, &mut site)?;
    Ok(sites)
}

fn materialize_memory_plan(
    plan: &LogicalPlan,
    dataset: &InMemoryDataset,
    cx: &RunCx<'_>,
    sites: &mut Materialized,
    site: &mut usize,
) -> Result<(), ExecutionError> {
    for op in &plan.body {
        match op {
            PlanOp::For { var, source } => {
                let this_site = *site;
                *site += 1;
                match source {
                    PlanSource::CollectionScan { collection } => {
                        let fetch_started = std::time::Instant::now();
                        let rows: Vec<Value> = dataset.collection(collection)?.to_vec();
                        cx.charge(rows.len())?;
                        cx.record_site(
                            this_site,
                            rows.len(),
                            fetch_started.elapsed().as_secs_f64() * 1000.0,
                        );
                        sites.insert(this_site, SiteRows::Docs(rows));
                    }
                    PlanSource::VectorSearch { collection, vector } => {
                        let query_vector =
                            value_to_vector(&eval_expr(vector, &Env::new(), &cx.eval())?)?;
                        let mut scored = dataset
                            .collection(collection)?
                            .iter()
                            .cloned()
                            .filter_map(|row| {
                                let embedding = row.get("embedding").and_then(vector_from_value)?;
                                let score = cosine_similarity(&query_vector, &embedding)?;
                                let mut scored_row = row;
                                if let Value::Object(obj) = &mut scored_row {
                                    obj.insert("_score".to_string(), json!(score));
                                }
                                Some((score, env_with(var, scored_row)))
                            })
                            .collect::<Vec<_>>();
                        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
                        cx.charge(scored.len())?;
                        cx.record_site(this_site, scored.len(), 0.0);
                        sites.insert(
                            this_site,
                            SiteRows::Envs(scored.into_iter().map(|(_, env)| env).collect()),
                        );
                    }
                    PlanSource::Traversal {
                        edge_var,
                        path_var,
                        min_depth,
                        max_depth,
                        direction,
                        start,
                        edge_collection,
                    } => {
                        if is_correlated_start(start) {
                            sites.insert(
                                this_site,
                                SiteRows::CorrelatedTraversal {
                                    opts: TraversalOpts {
                                        max_depth: *max_depth,
                                        min_depth: *min_depth,
                                        direction: to_core_direction(*direction),
                                        edge_collection: edge_collection.clone(),
                                        min_confidence: None,
                                        path_decay: 0.8,
                                    },
                                    bind_edge: !is_synthetic_var(edge_var),
                                    bind_path: !is_synthetic_var(path_var),
                                },
                            );
                            continue;
                        }
                        let envs = traversal_rows(
                            TraversalPlanRef {
                                vertex_var: var,
                                edge_var,
                                path_var,
                                min_depth: *min_depth,
                                max_depth: *max_depth,
                                direction: *direction,
                                start,
                                edge_collection,
                            },
                            dataset,
                            cx.bind_vars,
                        )?;
                        cx.charge(envs.len())?;
                        cx.record_site(this_site, envs.len(), 0.0);
                        sites.insert(this_site, SiteRows::Envs(envs));
                    }
                    PlanSource::VarRef(_) | PlanSource::Expression(_) => {}
                }
            }
            PlanOp::LetSubquery { plan: subplan, .. } => {
                materialize_memory_plan(subplan, dataset, cx, sites, site)?;
            }
            _ => {}
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// In-memory traversal + vector helpers (moved from the v1 source module)
// ---------------------------------------------------------------------------

/// A variable the parser invented because the query did not name one. `$` is
/// not a legal identifier character, so nothing can read it.
pub(super) fn is_synthetic_var(name: &str) -> bool {
    name.starts_with('$')
}

/// Does this traversal start expression read the enclosing row?
///
/// An expression that names no variable — a literal or a bind variable — has
/// the same value for every row, so it is materialized once before the run, as
/// it always was. Anything that reaches for an identifier is correlated and can
/// only be resolved per row. Erring toward "correlated" is the safe direction:
/// it costs a per-row resolution, while erring the other way would evaluate the
/// start against an environment that does not have the variable.
pub(super) fn is_correlated_start(start: &Expr) -> bool {
    fn walk(expr: &Expr, found: &mut bool) {
        if *found {
            return;
        }
        match expr {
            Expr::Identifier(_) => *found = true,
            Expr::Unary { expr, .. } => walk(expr, found),
            Expr::Binary { left, right, .. } => {
                walk(left, found);
                walk(right, found);
            }
            Expr::FunctionCall { args, .. } => args.iter().for_each(|a| walk(a, found)),
            Expr::Field { base, .. } => walk(base, found),
            Expr::Array(items) => items.iter().for_each(|i| walk(i, found)),
            Expr::Object(fields) => fields.iter().for_each(|f| walk(&f.value, found)),
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                walk(condition, found);
                walk(then_branch, found);
                walk(else_branch, found);
            }
            // A subquery cannot appear here (grammar restricts it to LET
            // values), and the remaining variants are literals.
            _ => {}
        }
    }
    let mut found = false;
    walk(start, &mut found);
    found
}

struct TraversalPlanRef<'a> {
    vertex_var: &'a str,
    edge_var: &'a str,
    path_var: &'a str,
    min_depth: u32,
    max_depth: u32,
    direction: Direction,
    start: &'a Expr,
    edge_collection: &'a str,
}

fn traversal_rows(
    plan: TraversalPlanRef<'_>,
    dataset: &InMemoryDataset,
    bind_vars: &BindVars,
) -> Result<Vec<Env>, ExecutionError> {
    let start_id = eval_expr(plan.start, &Env::new(), &EvalCtx::new(bind_vars))?
        .as_str()
        .map(str::to_string)
        .ok_or(ExecutionError::IdentifierNotFound("traversal start".into()))?;
    let hits = traversal_hits(
        dataset,
        &start_id,
        plan.edge_collection,
        plan.direction,
        plan.min_depth,
        plan.max_depth,
    )?;
    Ok(hits
        .into_iter()
        .map(|hit| {
            let mut env = Env::new();
            env.insert(plan.vertex_var.to_string(), hit.vertex);
            env.insert(plan.edge_var.to_string(), hit.edge);
            env.insert(plan.path_var.to_string(), hit.path);
            env
        })
        .collect())
}

/// Breadth-first walk over an in-memory edge collection.
///
/// Shared by the materialized (uncorrelated) path and the per-row resolution a
/// correlated traversal needs, so both agree on depth, cycle and path
/// semantics by construction rather than by two implementations matching.
pub(super) fn traversal_hits(
    dataset: &InMemoryDataset,
    start_id: &str,
    edge_collection: &str,
    direction: Direction,
    min_depth: u32,
    max_depth: u32,
) -> Result<Vec<TraversalHit>, ExecutionError> {
    let plan = TraversalPlanRef {
        vertex_var: "",
        edge_var: "",
        path_var: "",
        min_depth,
        max_depth,
        direction,
        start: &Expr::Null,
        edge_collection,
    };
    let start_id = start_id.to_string();
    let edges = dataset.collection(plan.edge_collection)?;
    let mut output: Vec<TraversalHit> = Vec::new();
    let mut queue = VecDeque::from([TraversalState {
        current: start_id.clone(),
        vertices: vec![vertex_value(dataset, &start_id)],
        edges: Vec::new(),
    }]);

    while let Some(state) = queue.pop_front() {
        let depth = state.edges.len() as u32;
        if depth >= plan.min_depth && depth <= plan.max_depth {
            let vertex = state
                .vertices
                .last()
                .cloned()
                .unwrap_or_else(|| vertex_value(dataset, &state.current));
            let edge = state.edges.last().cloned().unwrap_or(Value::Null);
            let path = json!({
                "vertices": state.vertices,
                "edges": state.edges,
                "depth": depth
            });
            output.push(TraversalHit { vertex, edge, path });
        }

        if depth == plan.max_depth {
            continue;
        }

        for edge in matching_edges(edges, &state.current, plan.direction) {
            let Some(next_id) = next_vertex_id(edge, &state.current, plan.direction) else {
                continue;
            };
            if state
                .vertices
                .iter()
                .any(|vertex| vertex.get("_id").and_then(Value::as_str) == Some(next_id))
            {
                continue;
            }
            let mut next_vertices = state.vertices.clone();
            next_vertices.push(vertex_value(dataset, next_id));
            let mut next_edges = state.edges.clone();
            next_edges.push(edge.clone());
            queue.push_back(TraversalState {
                current: next_id.to_string(),
                vertices: next_vertices,
                edges: next_edges,
            });
        }
    }

    Ok(output)
}

#[derive(Debug, Clone)]
struct TraversalState {
    current: String,
    vertices: Vec<Value>,
    edges: Vec<Value>,
}

fn matching_edges<'a>(edges: &'a [Value], current: &str, direction: Direction) -> Vec<&'a Value> {
    edges
        .iter()
        .filter(|edge| match direction {
            Direction::Outbound => edge.get("_from").and_then(Value::as_str) == Some(current),
            Direction::Inbound => edge.get("_to").and_then(Value::as_str) == Some(current),
            Direction::Any => {
                edge.get("_from").and_then(Value::as_str) == Some(current)
                    || edge.get("_to").and_then(Value::as_str) == Some(current)
            }
        })
        .collect()
}

fn next_vertex_id<'a>(edge: &'a Value, current: &str, direction: Direction) -> Option<&'a str> {
    match direction {
        Direction::Outbound => edge.get("_to").and_then(Value::as_str),
        Direction::Inbound => edge.get("_from").and_then(Value::as_str),
        Direction::Any => {
            let from = edge.get("_from").and_then(Value::as_str)?;
            let to = edge.get("_to").and_then(Value::as_str)?;
            if from == current {
                Some(to)
            } else {
                Some(from)
            }
        }
    }
}

fn to_core_direction(direction: Direction) -> CoreDirection {
    match direction {
        Direction::Outbound => CoreDirection::Outbound,
        Direction::Inbound => CoreDirection::Inbound,
        Direction::Any => CoreDirection::Any,
    }
}

fn vertex_value(dataset: &InMemoryDataset, id: &str) -> Value {
    dataset
        .vertex_by_id(id)
        .unwrap_or_else(|| json!({ "_id": id }))
}

fn env_with(var: &str, value: Value) -> Env {
    HashMap::from([(var.to_string(), value)])
}

fn value_to_vector(value: &Value) -> Result<Vec<f64>, ExecutionError> {
    vector_from_value(value).ok_or(ExecutionError::ExpectedVector)
}

fn vector_from_value(value: &Value) -> Option<Vec<f64>> {
    value
        .as_array()?
        .iter()
        .map(Value::as_f64)
        .collect::<Option<Vec<_>>>()
}

fn cosine_similarity(left: &[f64], right: &[f64]) -> Option<f64> {
    if left.is_empty() || left.len() != right.len() {
        return None;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|v| v * v).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|v| v * v).sum::<f64>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        return None;
    }
    Some(dot / (left_norm * right_norm))
}
