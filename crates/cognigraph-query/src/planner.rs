use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ast::*;
use crate::parser::{ParseError, parse_query};
use crate::validation::{
    ValidationError, ValidationOptions, validate_query, validate_query_with_options,
};

/// A non-executing logical plan for a validated CGQL query. The body is an
/// ordered operator pipeline (v2 positional semantics); COLLECT/SORT/LIMIT
/// remain tail clauses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogicalPlan {
    /// EXPLAIN prefix: describe the plan instead of executing it.
    #[serde(default)]
    pub explain: bool,
    /// EXPLAIN ANALYZE: execute and report statistics instead of rows.
    #[serde(default)]
    pub analyze: bool,
    pub body: Vec<PlanOp>,
    pub collect: Option<CollectClause>,
    pub sort: Option<SortClause>,
    pub limit: Option<LimitClause>,
    pub distinct: bool,
    pub mutation: Option<MutationClause>,
    pub projection: Option<Expr>,
    pub bind_vars: Vec<String>,
    /// Index into `body` where the deferred suffix begins (move-calculations-
    /// down): ops from here on are referenced only by the projection, so the
    /// runner executes them after SORT and LIMIT, on the surviving rows only.
    /// None means nothing was deferred and the body runs in one piece.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deferred_start: Option<usize>,
}

impl LogicalPlan {
    /// First FOR source of the body (v1-shaped convenience view).
    pub fn first_source(&self) -> Option<PlanSource> {
        self.body.iter().find_map(|op| match op {
            PlanOp::For { source, .. } => Some(source.clone()),
            _ => None,
        })
    }

    /// All FILTER expressions of the body, in order.
    pub fn filters(&self) -> Vec<Expr> {
        self.body
            .iter()
            .filter_map(|op| match op {
                PlanOp::Filter(filter) => Some(filter.clone()),
                _ => None,
            })
            .collect()
    }
}

/// One pipeline operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanOp {
    For {
        var: String,
        source: PlanSource,
    },
    Let(LetClause),
    /// A LET whose value is a subquery, pre-planned. Uncorrelated
    /// subqueries execute once per query; correlated ones per row.
    LetSubquery {
        name: String,
        plan: Box<LogicalPlan>,
        correlated: bool,
    },
    Filter(Expr),
}

/// Row source for a FOR operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanSource {
    /// Backend collection scan — materialized once per site, with that
    /// site's own projection/filter/limit pushdown.
    CollectionScan {
        collection: String,
    },
    /// Iteration over an in-scope array variable.
    VarRef(String),
    /// Iteration over an array-valued expression (evaluated per row).
    Expression(Expr),
    VectorSearch {
        collection: String,
        vector: Expr,
    },
    /// Binds vertex/edge/path — the For op's `var` is the vertex variable.
    Traversal {
        edge_var: String,
        path_var: String,
        min_depth: u32,
        max_depth: u32,
        direction: Direction,
        start: Expr,
        edge_collection: String,
    },
}

/// Planning error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlanError {
    #[error(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    Validation(#[from] ValidationError),
}

/// Validate and plan a parsed CGQL query using default validation options.
pub fn plan_query(query: &Query) -> Result<LogicalPlan, PlanError> {
    validate_query(query)?;
    Ok(build_plan(query))
}

/// Validate and plan a parsed CGQL query using explicit validation options.
pub fn plan_query_with_options(
    query: &Query,
    options: ValidationOptions,
) -> Result<LogicalPlan, PlanError> {
    validate_query_with_options(query, options)?;
    Ok(build_plan(query))
}

/// Parse, validate, and plan a CGQL query using default validation options.
pub fn parse_and_plan(input: &str) -> Result<LogicalPlan, PlanError> {
    let query = parse_query(input)?;
    plan_query(&query)
}

/// Parse, validate, and plan a CGQL query using explicit validation options.
pub fn parse_and_plan_with_options(
    input: &str,
    options: ValidationOptions,
) -> Result<LogicalPlan, PlanError> {
    let query = parse_query(input)?;
    plan_query_with_options(&query, options)
}

fn build_plan(query: &Query) -> LogicalPlan {
    build_plan_scoped(query, &BTreeSet::new())
}

fn build_plan_scoped(query: &Query, outer_scope: &BTreeSet<String>) -> LogicalPlan {
    let mut scope = outer_scope.clone();
    let mut body = Vec::new();
    for clause in &query.body {
        match clause {
            BodyClause::For(for_clause) => body.push(build_for_op(for_clause, &mut scope)),
            BodyClause::Let(let_clause) => match &let_clause.expr {
                Expr::Subquery(subquery) => {
                    // Correlated iff the subquery reads any variable bound
                    // in the enclosing scopes at this point.
                    let free = free_var_roots(subquery);
                    let correlated = free.iter().any(|root| scope.contains(root));
                    let plan = build_plan_scoped(subquery, &scope);
                    scope.insert(let_clause.name.clone());
                    body.push(PlanOp::LetSubquery {
                        name: let_clause.name.clone(),
                        plan: Box::new(plan),
                        correlated,
                    });
                }
                _ => {
                    scope.insert(let_clause.name.clone());
                    body.push(PlanOp::Let(let_clause.clone()));
                }
            },
            BodyClause::Filter(filter) => body.push(PlanOp::Filter(filter.clone())),
        }
    }
    let mut plan = LogicalPlan {
        explain: query.explain,
        analyze: query.analyze,
        body,
        collect: query.collect.clone(),
        sort: query.sort.clone(),
        limit: query.limit.clone(),
        distinct: query.distinct,
        mutation: query.mutation.clone(),
        projection: query.return_expr.clone(),
        bind_vars: query.bind_vars.clone(),
        deferred_start: None,
    };
    defer_return_only_lets(&mut plan);
    plan
}

/// Move-calculations-down: a `LET` whose variable feeds nothing but the
/// projection is moved past SORT and LIMIT, so it runs for the rows that
/// survive rather than for every row that might have.
///
/// The person-360 evaluation query is the motivating shape: one subquery
/// feeds the FILTER and the SORT, three more only decorate the RETURN — and
/// they were being computed for 651 rows to return 5.
///
/// Safety argument, not assumption:
/// - A `LET` binds a pure value; it never changes row count or order, so
///   running it after SORT/LIMIT yields the same value for every surviving
///   row. (Evaluation that would have errored on a discarded row no longer
///   runs at all — strictly fewer errors, the same trade AQL makes.)
/// - `COLLECT` consumes row variables, so nothing defers past one; a plan
///   with COLLECT is left untouched. Mutations read their row, likewise.
/// - Blocking is over-approximated: any identifier root read by a
///   non-deferred op or a sort key blocks that name, shadowing included.
///   Over-blocking costs the optimization, never correctness.
fn defer_return_only_lets(plan: &mut LogicalPlan) {
    if plan.collect.is_some() || plan.mutation.is_some() {
        return;
    }
    if plan.sort.is_none() && plan.limit.is_none() {
        return;
    }
    let mut blocked = BTreeSet::new();
    if let Some(sort) = &plan.sort {
        for key in &sort.keys {
            expr_reads_into(&key.expr, &mut blocked);
        }
    }
    let mut defer = vec![false; plan.body.len()];
    for i in (0..plan.body.len()).rev() {
        let deferrable = match &plan.body[i] {
            PlanOp::Let(clause) => !blocked.contains(&clause.name),
            PlanOp::LetSubquery { name, .. } => !blocked.contains(name),
            // FOR changes cardinality and FILTER prunes; both stay put.
            _ => false,
        };
        if deferrable {
            defer[i] = true;
        } else {
            op_reads_into(&plan.body[i], &mut blocked);
        }
    }
    if !defer.iter().any(|d| *d) {
        return;
    }
    let mut main = Vec::new();
    let mut tail = Vec::new();
    for (op, deferred) in plan.body.drain(..).zip(defer) {
        if deferred {
            tail.push(op);
        } else {
            main.push(op);
        }
    }
    plan.deferred_start = Some(main.len());
    main.extend(tail);
    plan.body = main;
}

/// Identifier roots an expression reads, over-approximated (no scope
/// tracking): used only to BLOCK deferral, where more names is the safe
/// direction.
fn expr_reads_into(expr: &Expr, out: &mut BTreeSet<String>) {
    match expr {
        Expr::Identifier(parts) => {
            if let Some(root) = parts.first() {
                out.insert(root.clone());
            }
        }
        Expr::Array(items) => items.iter().for_each(|item| expr_reads_into(item, out)),
        Expr::Object(fields) => fields
            .iter()
            .for_each(|field| expr_reads_into(&field.value, out)),
        Expr::Unary { expr, .. } => expr_reads_into(expr, out),
        Expr::Binary { left, right, .. } => {
            expr_reads_into(left, out);
            expr_reads_into(right, out);
        }
        Expr::FunctionCall { args, .. } => args.iter().for_each(|arg| expr_reads_into(arg, out)),
        Expr::Field { base, .. } => expr_reads_into(base, out),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_reads_into(condition, out);
            expr_reads_into(then_branch, out);
            expr_reads_into(else_branch, out);
        }
        Expr::Subquery(subquery) => out.extend(free_var_roots(subquery)),
        Expr::BindVar(_) | Expr::String(_) | Expr::Number(_) | Expr::Bool(_) | Expr::Null => {}
    }
}

/// Identifier roots a plan op reads, over-approximated the same way. A
/// subquery contributes every root it mentions anywhere, its own bindings
/// included — shadowed outer names then merely fail to defer.
fn op_reads_into(op: &PlanOp, out: &mut BTreeSet<String>) {
    match op {
        PlanOp::For { source, .. } => match source {
            PlanSource::VarRef(name) => {
                out.insert(name.clone());
            }
            PlanSource::Expression(expr)
            | PlanSource::Traversal { start: expr, .. }
            | PlanSource::VectorSearch { vector: expr, .. } => expr_reads_into(expr, out),
            PlanSource::CollectionScan { .. } => {}
        },
        PlanOp::Let(clause) => expr_reads_into(&clause.expr, out),
        PlanOp::LetSubquery { plan, .. } => plan_reads_into(plan, out),
        PlanOp::Filter(expr) => expr_reads_into(expr, out),
    }
}

fn plan_reads_into(plan: &LogicalPlan, out: &mut BTreeSet<String>) {
    for op in &plan.body {
        op_reads_into(op, out);
    }
    if let Some(sort) = &plan.sort {
        for key in &sort.keys {
            expr_reads_into(&key.expr, out);
        }
    }
    if let Some(collect) = &plan.collect {
        for binding in &collect.bindings {
            expr_reads_into(&binding.expr, out);
        }
        for aggregate in &collect.aggregates {
            expr_reads_into(&aggregate.expr, out);
        }
        if let Some(into) = &collect.into
            && let Some(projection) = &into.projection
        {
            expr_reads_into(projection, out);
        }
    }
    if let Some(projection) = &plan.projection {
        expr_reads_into(projection, out);
    }
}

fn build_for_op(for_clause: &ForClause, scope: &mut BTreeSet<String>) -> PlanOp {
    match for_clause {
        ForClause::Collection { var, collection } => {
            // Decision 1: bare names resolve as in-scope variables first,
            // then as collections. Validation rejects shadowing, so this
            // is deterministic.
            let source = if scope.contains(collection) {
                PlanSource::VarRef(collection.clone())
            } else {
                PlanSource::CollectionScan {
                    collection: collection.clone(),
                }
            };
            scope.insert(var.clone());
            PlanOp::For {
                var: var.clone(),
                source,
            }
        }
        ForClause::Expression { var, expr } => {
            scope.insert(var.clone());
            PlanOp::For {
                var: var.clone(),
                source: PlanSource::Expression(expr.clone()),
            }
        }
        ForClause::VectorSearch {
            var,
            collection,
            vector,
        } => {
            scope.insert(var.clone());
            PlanOp::For {
                var: var.clone(),
                source: PlanSource::VectorSearch {
                    collection: collection.clone(),
                    vector: vector.clone(),
                },
            }
        }
        ForClause::Traversal {
            vertex_var,
            edge_var,
            path_var,
            min_depth,
            max_depth,
            direction,
            start,
            edge_collection,
        } => {
            scope.insert(vertex_var.clone());
            scope.insert(edge_var.clone());
            scope.insert(path_var.clone());
            PlanOp::For {
                var: vertex_var.clone(),
                source: PlanSource::Traversal {
                    edge_var: edge_var.clone(),
                    path_var: path_var.clone(),
                    min_depth: *min_depth,
                    max_depth: *max_depth,
                    direction: *direction,
                    start: start.clone(),
                    edge_collection: edge_collection.clone(),
                },
            }
        }
    }
}

/// Identifier roots a query reads that it does not bind itself — used for
/// correlation detection. Binding is over-approximated order-insensitively;
/// use-before-definition inside the subquery is a validation error anyway.
fn free_var_roots(query: &Query) -> BTreeSet<String> {
    let mut bound = BTreeSet::new();
    for clause in &query.body {
        match clause {
            BodyClause::For(ForClause::Collection { var, .. })
            | BodyClause::For(ForClause::Expression { var, .. })
            | BodyClause::For(ForClause::VectorSearch { var, .. }) => {
                bound.insert(var.clone());
            }
            BodyClause::For(ForClause::Traversal {
                vertex_var,
                edge_var,
                path_var,
                ..
            }) => {
                bound.insert(vertex_var.clone());
                bound.insert(edge_var.clone());
                bound.insert(path_var.clone());
            }
            BodyClause::Let(let_clause) => {
                bound.insert(let_clause.name.clone());
            }
            BodyClause::Filter(_) => {}
        }
    }
    if let Some(collect) = &query.collect {
        for binding in &collect.bindings {
            bound.insert(binding.name.clone());
        }
        for aggregate in &collect.aggregates {
            bound.insert(aggregate.name.clone());
        }
        if let Some(into) = &collect.into {
            bound.insert(into.name.clone());
        }
        if let Some(count_var) = &collect.count_into {
            bound.insert(count_var.clone());
        }
    }

    let mut free = BTreeSet::new();
    for clause in &query.body {
        match clause {
            BodyClause::For(ForClause::Expression { expr, .. }) => {
                collect_free(expr, &bound, &mut free)
            }
            BodyClause::For(ForClause::VectorSearch { vector, .. }) => {
                collect_free(vector, &bound, &mut free)
            }
            BodyClause::For(ForClause::Traversal { start, .. }) => {
                collect_free(start, &bound, &mut free)
            }
            BodyClause::For(ForClause::Collection { collection, .. }) => {
                // A bare source name may itself be an outer variable.
                if !bound.contains(collection) {
                    free.insert(collection.clone());
                }
            }
            BodyClause::Let(let_clause) => collect_free(&let_clause.expr, &bound, &mut free),
            BodyClause::Filter(filter) => collect_free(filter, &bound, &mut free),
        }
    }
    if let Some(collect) = &query.collect {
        for binding in &collect.bindings {
            collect_free(&binding.expr, &bound, &mut free);
        }
        for aggregate in &collect.aggregates {
            collect_free(&aggregate.expr, &bound, &mut free);
        }
        if let Some(into) = &collect.into
            && let Some(projection) = &into.projection
        {
            collect_free(projection, &bound, &mut free);
        }
    }
    if let Some(sort) = &query.sort {
        for key in &sort.keys {
            collect_free(&key.expr, &bound, &mut free);
        }
    }
    if let Some(return_expr) = &query.return_expr {
        collect_free(return_expr, &bound, &mut free);
    }
    free
}

fn collect_free(expr: &Expr, bound: &BTreeSet<String>, free: &mut BTreeSet<String>) {
    match expr {
        Expr::Identifier(parts) => {
            if let Some(root) = parts.first()
                && !bound.contains(root)
            {
                free.insert(root.clone());
            }
        }
        Expr::Field { base, .. } => collect_free(base, bound, free),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_free(condition, bound, free);
            collect_free(then_branch, bound, free);
            collect_free(else_branch, bound, free);
        }
        Expr::Array(items) => items.iter().for_each(|e| collect_free(e, bound, free)),
        Expr::Object(fields) => fields
            .iter()
            .for_each(|f| collect_free(&f.value, bound, free)),
        Expr::Unary { expr, .. } => collect_free(expr, bound, free),
        Expr::Binary { left, right, .. } => {
            collect_free(left, bound, free);
            collect_free(right, bound, free);
        }
        Expr::FunctionCall { args, .. } => args.iter().for_each(|a| collect_free(a, bound, free)),
        Expr::Subquery(nested) => {
            // Nested free vars minus what this level binds.
            for root in free_var_roots(nested) {
                if !bound.contains(&root) {
                    free.insert(root);
                }
            }
        }
        Expr::BindVar(_) | Expr::String(_) | Expr::Number(_) | Expr::Bool(_) | Expr::Null => {}
    }
}
