mod clauses;
mod expr;

use std::collections::BTreeSet;

use pest::Parser;
use pest::error::LineColLocation;
use pest::iterators::Pair;
use pest_derive::Parser;

use crate::ast::*;

use clauses::{
    build_collect_clause, build_for_clause, build_let_clause, build_limit_clause,
    build_mutation_clause, build_sort_clause,
};
use expr::{build_expr, is_keyword_rule, single_inner};

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct CgqlParser;

/// CGQL parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    /// 1-based line and column of the failure, when known.
    pub line_col: Option<(usize, usize)>,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line_col: None,
        }
    }

    fn from_pest(error: pest::error::Error<Rule>) -> Self {
        let (line, column) = match error.line_col {
            LineColLocation::Pos((line, column)) => (line, column),
            LineColLocation::Span((line, column), _) => (line, column),
        };
        Self {
            message: error.variant.message().into_owned(),
            line_col: Some((line, column)),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line_col {
            Some((line, column)) => {
                write!(
                    f,
                    "parse error at line {line}, column {column}: {}",
                    self.message
                )
            }
            None => write!(f, "parse error: {}", self.message),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a CGQL query into an AST.
pub fn parse_query(input: &str) -> Result<Query, ParseError> {
    let mut pairs = CgqlParser::parse(Rule::query, input).map_err(ParseError::from_pest)?;
    let query_pair = pairs
        .next()
        .ok_or_else(|| ParseError::new("parser produced no query"))?;
    let mut query = build_query(query_pair)?;
    // Runs once over the finished AST, so the planner, validator and executor
    // only ever see subqueries in LET position — the shape decision_cgql_v2.md
    // approved.
    let mut counter = 0usize;
    desugar_subqueries(&mut query, &mut counter)?;
    query.bind_vars = collect_bind_vars(&query);
    Ok(query)
}

fn build_query(pair: Pair<'_, Rule>) -> Result<Query, ParseError> {
    let mut explain = false;
    let mut analyze = false;
    // query wraps an optional EXPLAIN [ANALYZE] prefix and either
    // read_query or mutation_query; iterate its clauses.
    let mut body = None;
    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::kw_explain => explain = true,
            Rule::kw_analyze => analyze = true,
            Rule::read_query | Rule::mutation_query => body = Some(part),
            _ => {}
        }
    }
    let body = body.ok_or_else(|| ParseError::new("missing query body"))?;
    let mut query = build_query_body(body)?;
    query.explain = explain;
    query.analyze = analyze;
    query.bind_vars = collect_bind_vars(&query);
    Ok(query)
}

/// Build a Query from a read_query or mutation_query pair — also the
/// entry point for parenthesized subqueries (their pair IS a read_query).
/// Subquery bind_vars stay empty; the top-level query collects them all.
pub(super) fn build_subquery(pair: Pair<'_, Rule>) -> Result<Expr, ParseError> {
    let body = pair
        .into_inner()
        .find(|p| p.as_rule() == Rule::read_query)
        .ok_or_else(|| ParseError::new("empty subquery"))?;
    Ok(Expr::Subquery(Box::new(build_query_body(body)?)))
}

/// Lift subqueries out of non-LET expression positions into synthetic LETs.
///
/// `decision_cgql_v2.md` restricted subqueries to LET values; this keeps that
/// restriction in the AST the planner and executor see, while letting authors
/// write `LENGTH((FOR …))` where they expect to. Purely syntactic: the rewrite
/// emits exactly the LET the author would have written by hand, so there is no
/// new execution form and correlation analysis is unchanged.
///
/// Synthetic names use `$` — not a legal identifier character — so a hoisted
/// binding can never collide with a user variable. One counter spans the entire
/// parsed query, including nested bodies: their scopes can see outer generated
/// bindings, so resetting the counter in a child would create shadowing.
fn desugar_subqueries(query: &mut Query, counter: &mut usize) -> Result<(), ParseError> {
    let mut body = Vec::with_capacity(query.body.len());
    for clause in std::mem::take(&mut query.body) {
        let mut hoisted = Vec::new();
        let mut clause = clause;
        match &mut clause {
            // A LET whose whole value is a subquery is already legal; only
            // subqueries NESTED inside it need lifting.
            BodyClause::Let(let_clause) => match &mut let_clause.expr {
                Expr::Subquery(inner) => desugar_subqueries(inner, counter)?,
                other => hoist_expr(other, counter, &mut hoisted)?,
            },
            BodyClause::Filter(expr) => hoist_expr(expr, counter, &mut hoisted)?,
            BodyClause::For(for_clause) => match for_clause {
                ForClause::Expression { expr, .. } => hoist_expr(expr, counter, &mut hoisted)?,
                ForClause::VectorSearch { vector, .. } => {
                    hoist_expr(vector, counter, &mut hoisted)?
                }
                ForClause::Traversal { start, .. } => hoist_expr(start, counter, &mut hoisted)?,
                ForClause::Collection { .. } => {}
            },
        }
        body.extend(hoisted);
        body.push(clause);
    }

    // Tail positions: a hoisted LET lands at the END of the body, which is only
    // equivalent while no COLLECT sits between. COLLECT drops row variables, so
    // hoisting past it would silently change what the expression sees.
    let mut tail = Vec::new();
    if let Some(collect) = &mut query.collect {
        for binding in &mut collect.bindings {
            hoist_expr(&mut binding.expr, counter, &mut tail)?;
        }
        for aggregate in &mut collect.aggregates {
            hoist_expr(&mut aggregate.expr, counter, &mut tail)?;
        }
    }
    let post_collect = query.collect.is_some();
    let mut after = Vec::new();
    if let Some(sort) = &mut query.sort {
        for key in &mut sort.keys {
            hoist_expr(&mut key.expr, counter, &mut after)?;
        }
    }
    if let Some(return_expr) = &mut query.return_expr {
        hoist_expr(return_expr, counter, &mut after)?;
    }
    if post_collect && !after.is_empty() {
        return Err(ParseError::new(
            "a subquery in SORT or RETURN after COLLECT cannot be lifted: COLLECT \
             drops row variables, so the subquery would not see what it reads. \
             Bind it to a LET before the COLLECT instead.",
        ));
    }
    body.extend(tail);
    body.extend(after);
    query.body = body;
    Ok(())
}

/// Replace every `Expr::Subquery` inside `expr` with a reference to a fresh
/// synthetic LET, appended to `out` in evaluation order (innermost first).
fn hoist_expr(
    expr: &mut Expr,
    counter: &mut usize,
    out: &mut Vec<BodyClause>,
) -> Result<(), ParseError> {
    match expr {
        Expr::Subquery(inner) => {
            desugar_subqueries(inner, counter)?;
            let name = format!("$sq{counter}");
            *counter += 1;
            let taken = std::mem::replace(expr, Expr::Identifier(vec![name.clone()]));
            out.push(BodyClause::Let(LetClause { name, expr: taken }));
        }
        Expr::Array(items) => {
            for item in items {
                hoist_expr(item, counter, out)?;
            }
        }
        Expr::Object(fields) => {
            for field in fields {
                hoist_expr(&mut field.value, counter, out)?;
            }
        }
        Expr::Field { base, .. } => hoist_expr(base, counter, out)?,
        Expr::Unary { expr, .. } => hoist_expr(expr, counter, out)?,
        Expr::Binary { left, right, .. } => {
            hoist_expr(left, counter, out)?;
            hoist_expr(right, counter, out)?;
        }
        Expr::FunctionCall { args, .. } => {
            for arg in args {
                hoist_expr(arg, counter, out)?;
            }
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            hoist_expr(condition, counter, out)?;
            hoist_expr(then_branch, counter, out)?;
            hoist_expr(else_branch, counter, out)?;
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn build_query_body(pair: Pair<'_, Rule>) -> Result<Query, ParseError> {
    let mut body = Vec::new();
    let mut collect = None;
    let mut sort = None;
    let mut limit = None;
    let mut distinct = false;
    let mut mutation = None;
    let mut return_expr = None;

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::for_clause => body.push(BodyClause::For(build_for_clause(child)?)),
            Rule::let_clause => body.push(BodyClause::Let(build_let_clause(child)?)),
            Rule::filter_clause => {
                body.push(BodyClause::Filter(build_expr(single_inner(
                    child,
                    "FILTER expression",
                )?)?));
            }
            Rule::collect_clause => collect = Some(build_collect_clause(child)?),
            Rule::mutation_clause => mutation = Some(build_mutation_clause(child)?),
            Rule::sort_clause => sort = Some(build_sort_clause(child)?),
            Rule::limit_clause => limit = Some(build_limit_clause(child)?),
            Rule::return_clause => {
                for part in child.into_inner() {
                    match part.as_rule() {
                        Rule::kw_distinct => distinct = true,
                        rule if is_keyword_rule(rule) => {}
                        _ => return_expr = Some(build_expr(part)?),
                    }
                }
            }
            Rule::EOI => {}
            _ => {}
        }
    }

    Ok(Query {
        explain: false,
        analyze: false,
        body,
        mutation,
        collect,
        sort,
        limit,
        distinct,
        return_expr,
        bind_vars: Vec::new(),
    })
}

fn collect_bind_vars(query: &Query) -> Vec<String> {
    let mut vars = BTreeSet::new();
    collect_bind_vars_into(query, &mut vars);
    vars.into_iter().collect()
}

fn collect_bind_vars_into(query: &Query, vars: &mut BTreeSet<String>) {
    for clause in &query.body {
        match clause {
            BodyClause::For(for_clause) => collect_bind_vars_for(for_clause, vars),
            BodyClause::Let(let_clause) => collect_bind_vars_expr(&let_clause.expr, vars),
            BodyClause::Filter(filter) => collect_bind_vars_expr(filter, vars),
        }
    }
    if let Some(mutation) = &query.mutation {
        match mutation {
            MutationClause::Insert { doc, .. } => collect_bind_vars_expr(doc, vars),
            MutationClause::Update { key, with, .. }
            | MutationClause::Replace { key, with, .. } => {
                collect_bind_vars_expr(key, vars);
                collect_bind_vars_expr(with, vars);
            }
            MutationClause::Remove { key, .. } => collect_bind_vars_expr(key, vars),
            MutationClause::Upsert {
                search,
                insert,
                update,
                ..
            } => {
                collect_bind_vars_expr(search, vars);
                collect_bind_vars_expr(insert, vars);
                collect_bind_vars_expr(update, vars);
            }
        }
    }
    if let Some(collect) = &query.collect {
        for binding in &collect.bindings {
            collect_bind_vars_expr(&binding.expr, vars);
        }
        for aggregate in &collect.aggregates {
            collect_bind_vars_expr(&aggregate.expr, vars);
        }
        if let Some(into) = &collect.into
            && let Some(projection) = &into.projection
        {
            collect_bind_vars_expr(projection, vars);
        }
    }
    if let Some(sort) = &query.sort {
        for key in &sort.keys {
            collect_bind_vars_expr(&key.expr, vars);
        }
    }
    if let Some(return_expr) = &query.return_expr {
        collect_bind_vars_expr(return_expr, vars);
    }
}

fn collect_bind_vars_for(for_clause: &ForClause, vars: &mut BTreeSet<String>) {
    match for_clause {
        ForClause::Collection { .. } => {}
        ForClause::Expression { expr, .. } => collect_bind_vars_expr(expr, vars),
        ForClause::VectorSearch { vector, .. } => collect_bind_vars_expr(vector, vars),
        ForClause::Traversal { start, .. } => collect_bind_vars_expr(start, vars),
    }
}

fn collect_bind_vars_expr(expr: &Expr, vars: &mut BTreeSet<String>) {
    match expr {
        Expr::BindVar(name) => {
            vars.insert(name.clone());
        }
        Expr::Subquery(query) => collect_bind_vars_into(query, vars),
        Expr::Field { base, .. } => collect_bind_vars_expr(base, vars),
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_bind_vars_expr(condition, vars);
            collect_bind_vars_expr(then_branch, vars);
            collect_bind_vars_expr(else_branch, vars);
        }
        Expr::Array(items) => {
            for item in items {
                collect_bind_vars_expr(item, vars);
            }
        }
        Expr::Object(fields) => {
            for field in fields {
                collect_bind_vars_expr(&field.value, vars);
            }
        }
        Expr::Unary { expr, .. } => collect_bind_vars_expr(expr, vars),
        Expr::Binary { left, right, .. } => {
            collect_bind_vars_expr(left, vars);
            collect_bind_vars_expr(right, vars);
        }
        Expr::FunctionCall { args, .. } => {
            for arg in args {
                collect_bind_vars_expr(arg, vars);
            }
        }
        Expr::Identifier(_) | Expr::String(_) | Expr::Number(_) | Expr::Bool(_) | Expr::Null => {}
    }
}
