//! Mutations run without the backend read retry context. Reject every dynamic
//! backend read before materialization or the first write, including reads in
//! nested bodies, unused branches, and RETURN. Replaying mutations to resolve
//! these dependencies would repeat committed writes.

use super::ValidationError;
use crate::ast::{BodyClause, Expr, ForClause, MutationClause, Query};

pub(super) fn validate_backend_reads(query: &Query) -> Result<(), ValidationError> {
    for clause in &query.body {
        match clause {
            BodyClause::For(ForClause::Collection { .. }) => {}
            BodyClause::For(ForClause::Expression { expr, .. })
            | BodyClause::Let(crate::ast::LetClause { expr, .. })
            | BodyClause::Filter(expr) => validate_expr(expr, false)?,
            BodyClause::For(ForClause::VectorSearch { vector, .. }) => {
                validate_expr(vector, false)?;
            }
            BodyClause::For(ForClause::Traversal { start, .. }) => {
                validate_expr(start, true)?;
            }
        }
    }
    // These clauses can occur in a mutation's read subqueries.
    if let Some(collect) = &query.collect {
        for binding in &collect.bindings {
            validate_expr(&binding.expr, false)?;
        }
        for aggregate in &collect.aggregates {
            validate_expr(&aggregate.expr, false)?;
        }
        if let Some(into) = &collect.into
            && let Some(projection) = &into.projection
        {
            validate_expr(projection, false)?;
        }
    }
    if let Some(sort) = &query.sort {
        for key in &sort.keys {
            validate_expr(&key.expr, false)?;
        }
    }
    if let Some(mutation) = &query.mutation {
        match mutation {
            MutationClause::Insert { doc, .. } => validate_expr(doc, false)?,
            MutationClause::Update { key, with, .. }
            | MutationClause::Replace { key, with, .. } => {
                validate_expr(key, false)?;
                validate_expr(with, false)?;
            }
            MutationClause::Remove { key, .. } => validate_expr(key, false)?,
            MutationClause::Upsert {
                search,
                insert,
                update,
                ..
            } => {
                validate_expr(search, false)?;
                validate_expr(insert, false)?;
                validate_expr(update, false)?;
            }
        }
    }
    if let Some(projection) = &query.return_expr {
        validate_expr(projection, false)?;
    }
    Ok(())
}

fn validate_expr(expr: &Expr, traversal_start: bool) -> Result<(), ValidationError> {
    match expr {
        Expr::Identifier(_) if traversal_start => {
            // Materialization treats any identifier in a start as correlated,
            // even a LET variable that happens to contain a constant string.
            return Err(ValidationError::CorrelatedTraversalInMutation);
        }
        Expr::Field { base, .. } | Expr::Unary { expr: base, .. } => {
            validate_expr(base, traversal_start)?;
        }
        Expr::Array(items) => {
            for item in items {
                validate_expr(item, traversal_start)?;
            }
        }
        Expr::Object(fields) => {
            for field in fields {
                validate_expr(&field.value, traversal_start)?;
            }
        }
        Expr::Binary { left, right, .. } => {
            validate_expr(left, traversal_start)?;
            validate_expr(right, traversal_start)?;
        }
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_expr(condition, traversal_start)?;
            validate_expr(then_branch, traversal_start)?;
            validate_expr(else_branch, traversal_start)?;
        }
        Expr::FunctionCall { name, args } => {
            if name.eq_ignore_ascii_case("DOCUMENT") {
                return Err(ValidationError::DocumentInMutation);
            }
            for arg in args {
                validate_expr(arg, traversal_start)?;
            }
        }
        Expr::Subquery(query) => validate_backend_reads(query)?,
        Expr::Identifier(_)
        | Expr::BindVar(_)
        | Expr::String(_)
        | Expr::Number(_)
        | Expr::Bool(_)
        | Expr::Null => {}
    }
    Ok(())
}
