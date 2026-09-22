use std::collections::BTreeSet;

use thiserror::Error;

use crate::ast::*;
use crate::functions;

mod mutation;

const DEFAULT_MAX_LIMIT: u64 = 10_000;
const DEFAULT_MAX_TRAVERSAL_DEPTH: u32 = 10;

/// Semantic validation options for parsed CGQL queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationOptions {
    pub max_limit: u64,
    pub max_traversal_depth: u32,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            max_limit: DEFAULT_MAX_LIMIT,
            max_traversal_depth: DEFAULT_MAX_TRAVERSAL_DEPTH,
        }
    }
}

/// Semantic validation error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("variable `{0}` is declared more than once")]
    DuplicateVariable(String),

    #[error("variable name `{0}` is reserved")]
    ReservedVariableName(String),

    #[error("identifier `{0}` is not in scope")]
    UnknownIdentifier(String),

    #[error("collection name `{0}` is reserved")]
    ReservedCollectionName(String),

    #[error("function name `{0}` is reserved")]
    ReservedFunctionName(String),

    #[error("object field `{0}` is declared more than once")]
    DuplicateObjectField(String),

    #[error("LIMIT count must be greater than zero")]
    ZeroLimit,

    #[error("LIMIT count {count} exceeds configured maximum {max}")]
    LimitTooLarge { count: u64, max: u64 },

    /// A bound LIMIT operand resolved to something other than a non-negative
    /// integer JSON number (raised before execution, CG-85).
    #[error("LIMIT {role} bind variable `@{name}` must be a non-negative integer")]
    LimitBindNotInteger { role: String, name: String },

    #[error("traversal maximum depth {depth} exceeds configured maximum {max}")]
    TraversalDepthTooLarge { depth: u32, max: u32 },

    #[error("VECTOR_SEARCH vector expression must be a bind variable or array literal")]
    InvalidVectorSearchInput,

    #[error("mutations cannot be combined with COLLECT")]
    MutationWithCollect,

    #[error("DOCUMENT() is not supported in mutation queries")]
    DocumentInMutation,

    #[error("correlated traversal starts are not supported in mutation queries")]
    CorrelatedTraversalInMutation,

    #[error("invalid COLLATE locale `{0}`")]
    InvalidCollation(String),

    #[error("unknown function `{0}`")]
    UnknownFunction(String),

    #[error("function `{name}` expects {expected} argument(s), got {got}")]
    FunctionArity {
        name: String,
        expected: String,
        got: usize,
    },

    #[error("subqueries nest deeper than the maximum of {0}")]
    SubqueryTooDeep(usize),

    #[error("VECTOR_SEARCH is only allowed as the first FOR of the outermost query")]
    SpecialSourceNotFirst,

    #[error("subqueries are only allowed as LET values")]
    SubqueryPosition,

    #[error("EXPLAIN ANALYZE does not support mutations")]
    AnalyzeMutation,
}

/// Decision 3 (decision_cgql_v2.md): a fixed nesting cap keeps budget
/// analysis and planning bounded.
const MAX_SUBQUERY_DEPTH: usize = 4;

/// Validate a parsed CGQL query using default semantic limits.
pub fn validate_query(query: &Query) -> Result<(), ValidationError> {
    validate_query_with_options(query, ValidationOptions::default())
}

/// Validate a parsed CGQL query using explicit semantic limits.
pub fn validate_query_with_options(
    query: &Query,
    options: ValidationOptions,
) -> Result<(), ValidationError> {
    validate_query_at_depth(query, options, 0)?;
    if query.mutation.is_some() {
        mutation::validate_backend_reads(query)?;
    }
    Ok(())
}

/// v2 positional semantics: the body is walked in written order and every
/// clause validates against exactly the variables bound above it — a
/// FILTER referencing a later LET is a use-before-definition error.
fn validate_query_at_depth(
    query: &Query,
    options: ValidationOptions,
    depth: usize,
) -> Result<(), ValidationError> {
    if depth > MAX_SUBQUERY_DEPTH {
        return Err(ValidationError::SubqueryTooDeep(MAX_SUBQUERY_DEPTH));
    }
    // EXPLAIN ANALYZE executes; running a mutation under an analysis
    // prefix is a footgun we refuse (unlike Postgres).
    if query.analyze && query.mutation.is_some() {
        return Err(ValidationError::AnalyzeMutation);
    }
    let mut scope = BTreeSet::new();
    for (position, clause) in query.body.iter().enumerate() {
        match clause {
            BodyClause::For(for_clause) => {
                // A traversal may now start from the enclosing row (D2), so it
                // is allowed anywhere. VECTOR_SEARCH is not: its cost is a
                // whole index probe, and running one per outer row is a
                // different decision from making traversal correlated.
                if matches!(for_clause, ForClause::VectorSearch { .. })
                    && (depth > 0 || position != 0)
                {
                    return Err(ValidationError::SpecialSourceNotFirst);
                }
                validate_for_clause(for_clause, options, &mut scope)?;
            }
            BodyClause::Let(let_clause) => {
                match &let_clause.expr {
                    Expr::Subquery(subquery) => {
                        // Correlated subqueries see the outer scope; their
                        // own bindings do not leak back out.
                        validate_subquery(subquery, options, depth + 1, &scope)?;
                    }
                    expr => validate_expr(expr, &scope)?,
                }
                insert_var(&mut scope, &let_clause.name)?;
            }
            BodyClause::Filter(filter) => validate_expr(filter, &scope)?,
        }
    }
    validate_query_tail(query, options, scope)
}

/// Everything after the body: mutation, COLLECT, SORT, LIMIT, RETURN.
fn validate_query_tail(
    query: &Query,
    options: ValidationOptions,
    mut scope: BTreeSet<String>,
) -> Result<(), ValidationError> {
    if let Some(mutation) = &query.mutation {
        if query.collect.is_some() {
            return Err(ValidationError::MutationWithCollect);
        }
        let (key, with) = match mutation {
            MutationClause::Insert { doc, .. } => (None, Some(doc)),
            MutationClause::Update { key, with, .. }
            | MutationClause::Replace { key, with, .. } => (Some(key), Some(with)),
            MutationClause::Remove { key, .. } => (Some(key), None),
            MutationClause::Upsert {
                search,
                insert,
                update,
                ..
            } => {
                validate_expr(search, &scope)?;
                validate_expr(insert, &scope)?;
                validate_expr(update, &scope)?;
                (None, None)
            }
        };
        if let Some(key) = key {
            validate_expr(key, &scope)?;
        }
        if let Some(with) = with {
            validate_expr(with, &scope)?;
        }
        // NEW/OLD become available to RETURN after a mutation.
        scope.insert("NEW".to_string());
        scope.insert("OLD".to_string());
    }
    if let Some(collect) = &query.collect {
        let mut group_scope = BTreeSet::new();
        for binding in &collect.bindings {
            validate_expr(&binding.expr, &scope)?;
            insert_var(&mut group_scope, &binding.name)?;
        }
        for aggregate in &collect.aggregates {
            if !matches!(
                aggregate.function.to_ascii_uppercase().as_str(),
                "SUM" | "MIN" | "MAX" | "AVG"
            ) {
                return Err(ValidationError::UnknownFunction(aggregate.function.clone()));
            }
            validate_expr(&aggregate.expr, &scope)?;
            insert_var(&mut group_scope, &aggregate.name)?;
        }
        if let Some(into) = &collect.into {
            if let Some(projection) = &into.projection {
                validate_expr(projection, &scope)?;
            }
            insert_var(&mut group_scope, &into.name)?;
        }
        if let Some(count_var) = &collect.count_into {
            insert_var(&mut group_scope, count_var)?;
        }
        // After COLLECT only the group bindings are in scope.
        scope = group_scope;
    }
    if let Some(sort) = &query.sort {
        for key in &sort.keys {
            validate_expr(&key.expr, &scope)?;
        }
        if let Some(locale) = &sort.collation
            && locale.parse::<icu::locale::Locale>().is_err()
        {
            return Err(ValidationError::InvalidCollation(locale.clone()));
        }
    }
    if let Some(limit) = &query.limit {
        validate_limit(limit, options)?;
    }
    if let Some(return_expr) = &query.return_expr {
        validate_expr(return_expr, &scope)?;
    }

    Ok(())
}

/// Validate a subquery body with the outer scope visible (correlation),
/// on a scope copy so inner bindings stay inner.
fn validate_subquery(
    subquery: &Query,
    options: ValidationOptions,
    depth: usize,
    outer_scope: &BTreeSet<String>,
) -> Result<(), ValidationError> {
    let mut scope = outer_scope.clone();
    for (position, clause) in subquery.body.iter().enumerate() {
        match clause {
            BodyClause::For(for_clause) => {
                if matches!(for_clause, ForClause::VectorSearch { .. }) {
                    let _ = position;
                    return Err(ValidationError::SpecialSourceNotFirst);
                }
                validate_for_clause(for_clause, options, &mut scope)?;
            }
            BodyClause::Let(let_clause) => {
                match &let_clause.expr {
                    Expr::Subquery(nested) => {
                        if depth + 1 > MAX_SUBQUERY_DEPTH {
                            return Err(ValidationError::SubqueryTooDeep(MAX_SUBQUERY_DEPTH));
                        }
                        validate_subquery(nested, options, depth + 1, &scope)?;
                    }
                    expr => validate_expr(expr, &scope)?,
                }
                insert_var(&mut scope, &let_clause.name)?;
            }
            BodyClause::Filter(filter) => validate_expr(filter, &scope)?,
        }
    }
    validate_query_tail(subquery, options, scope)
}

fn validate_for_clause(
    for_clause: &ForClause,
    options: ValidationOptions,
    scope: &mut BTreeSet<String>,
) -> Result<(), ValidationError> {
    match for_clause {
        ForClause::Collection { var, collection } => {
            // A bare name that is an in-scope variable iterates that
            // variable (resolved at plan time); otherwise it must be a
            // legal collection name.
            if !scope.contains(collection) {
                validate_collection_name(collection)?;
            }
            insert_var(scope, var)?;
        }
        ForClause::Expression { var, expr } => {
            validate_expr(expr, scope)?;
            insert_var(scope, var)?;
        }
        ForClause::VectorSearch {
            var,
            collection,
            vector,
        } => {
            validate_collection_name(collection)?;
            validate_vector_input(vector)?;
            insert_var(scope, var)?;
        }
        ForClause::Traversal {
            vertex_var,
            edge_var,
            path_var,
            max_depth,
            start,
            edge_collection,
            ..
        } => {
            validate_collection_name(edge_collection)?;
            if *max_depth > options.max_traversal_depth {
                return Err(ValidationError::TraversalDepthTooLarge {
                    depth: *max_depth,
                    max: options.max_traversal_depth,
                });
            }
            // Since D2 the start is an ordinary expression, validated against
            // the variables bound above it — so it may read the enclosing row.
            // Its own variables are inserted after, so it cannot name itself.
            validate_expr(start, scope)?;
            validate_traversal_start(start)?;
            insert_var(scope, vertex_var)?;
            insert_var(scope, edge_var)?;
            insert_var(scope, path_var)?;
        }
    }

    Ok(())
}

fn validate_collection_name(name: &str) -> Result<(), ValidationError> {
    if is_reserved_word(name) {
        Err(ValidationError::ReservedCollectionName(name.to_string()))
    } else {
        Ok(())
    }
}

fn validate_vector_input(expr: &Expr) -> Result<(), ValidationError> {
    match expr {
        Expr::BindVar(_) => Ok(()),
        Expr::Array(items) if items.iter().all(is_numeric_literal_expr) => Ok(()),
        _ => Err(ValidationError::InvalidVectorSearchInput),
    }
}

fn is_numeric_literal_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::Number(_))
        || matches!(
            expr,
            Expr::Unary {
                op: UnaryOp::Neg,
                expr
            } if matches!(expr.as_ref(), Expr::Number(_))
        )
}

/// Reject a start vertex that cannot be an identifier no matter what the data
/// says. Anything that could evaluate to a string — a variable, a field, a bind
/// variable, a call — is left to the run; a numeric or boolean literal is a
/// mistake the query author can be told about now rather than at runtime.
fn validate_traversal_start(expr: &Expr) -> Result<(), ValidationError> {
    match expr {
        Expr::Number(_) | Expr::Bool(_) | Expr::Null | Expr::Array(_) | Expr::Object(_) => Err(
            ValidationError::UnknownIdentifier("traversal start".to_string()),
        ),
        _ => Ok(()),
    }
}

/// Literal operands are checked here; bound operands are checked with the
/// same errors once their values are known, before any storage access.
fn validate_limit(limit: &LimitClause, options: ValidationOptions) -> Result<(), ValidationError> {
    if let crate::ast::LimitValue::Literal(count) = limit.count {
        if count == 0 {
            return Err(ValidationError::ZeroLimit);
        }
        if count > options.max_limit {
            return Err(ValidationError::LimitTooLarge {
                count,
                max: options.max_limit,
            });
        }
    }
    Ok(())
}

fn insert_var(scope: &mut BTreeSet<String>, var: &str) -> Result<(), ValidationError> {
    if is_reserved_word(var) {
        return Err(ValidationError::ReservedVariableName(var.to_string()));
    }
    if !scope.insert(var.to_string()) {
        Err(ValidationError::DuplicateVariable(var.to_string()))
    } else {
        Ok(())
    }
}

fn validate_expr(expr: &Expr, scope: &BTreeSet<String>) -> Result<(), ValidationError> {
    match expr {
        Expr::Identifier(parts) => {
            if let Some(root) = parts.first()
                && !scope.contains(root)
            {
                return Err(ValidationError::UnknownIdentifier(root.clone()));
            }
        }
        Expr::Field { base, .. } => validate_expr(base, scope)?,
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            validate_expr(condition, scope)?;
            validate_expr(then_branch, scope)?;
            validate_expr(else_branch, scope)?;
        }
        Expr::Array(items) => {
            for item in items {
                validate_expr(item, scope)?;
            }
        }
        Expr::Object(fields) => {
            let mut names = BTreeSet::new();
            for field in fields {
                if !names.insert(field.name.clone()) {
                    return Err(ValidationError::DuplicateObjectField(field.name.clone()));
                }
                validate_expr(&field.value, scope)?;
            }
        }
        Expr::Unary { expr, .. } => validate_expr(expr, scope)?,
        Expr::Binary { left, right, .. } => {
            validate_expr(left, scope)?;
            validate_expr(right, scope)?;
        }
        Expr::FunctionCall { name, args } => {
            if is_reserved_function_name(name) {
                return Err(ValidationError::ReservedFunctionName(name.clone()));
            }
            let Some(def) = functions::lookup(name) else {
                return Err(ValidationError::UnknownFunction(name.clone()));
            };
            if args.len() < def.min_args || def.max_args.is_some_and(|max| args.len() > max) {
                return Err(ValidationError::FunctionArity {
                    name: def.name.to_string(),
                    expected: def.arity_description(),
                    got: args.len(),
                });
            }
            for arg in args {
                validate_expr(arg, scope)?;
            }
        }
        // Grammar restricts subqueries to LET values, where the body walk
        // intercepts them before this function runs; reaching one here
        // means an unsupported position.
        Expr::Subquery(_) => return Err(ValidationError::SubqueryPosition),
        Expr::BindVar(_) | Expr::String(_) | Expr::Number(_) | Expr::Bool(_) | Expr::Null => {}
    }

    Ok(())
}

fn is_reserved_function_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("VECTOR_SEARCH")
}

fn is_reserved_word(word: &str) -> bool {
    matches!(
        word.to_ascii_uppercase().as_str(),
        "AGGREGATE"
            | "ANALYZE"
            | "AND"
            | "ANY"
            | "ASC"
            | "DESC"
            | "COLLATE"
            | "COLLECT"
            | "COUNT"
            | "DISTINCT"
            | "EXPLAIN"
            | "INTO"
            | "LET"
            | "WITH"
            | "FILTER"
            | "FOR"
            | "IN"
            | "INBOUND"
            | "LIMIT"
            | "NOT"
            | "NULL"
            | "OR"
            | "OUTBOUND"
            | "RETURN"
            | "SORT"
            | "TRUE"
            | "FALSE"
            | "VECTOR_SEARCH"
    )
}
