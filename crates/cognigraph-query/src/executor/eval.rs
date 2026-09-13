use std::cmp::Ordering;

use serde_json::{Map, Value, json};

use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::functions;

use super::{BindVars, Env, ExecutionError};

/// Documents `DOCUMENT()` may read.
///
/// Deliberately SYNCHRONOUS. Row evaluation runs after all fetching (see
/// `materialize.rs`), and making `eval_expr` async to allow an inline fetch
/// would make every expression node an await point for one function. Instead a
/// miss is recorded and the caller re-runs with the ids resolved — see
/// `execute_backend_plan`.
// `Send + Sync` because the run context crosses an await point on the async
// backend path.
pub(super) trait DocumentSource: Send + Sync {
    /// Resolved document for a `collection/key` id, if already known.
    fn get(&self, id: &str) -> Option<Value>;
    /// Record an id this evaluation needed but could not resolve.
    fn miss(&self, _id: &str) {}
}

/// What an expression may read besides its row.
#[derive(Clone, Copy)]
pub(super) struct EvalCtx<'a> {
    pub bind_vars: &'a BindVars,
    pub documents: Option<&'a dyn DocumentSource>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(bind_vars: &'a BindVars) -> Self {
        Self {
            bind_vars,
            documents: None,
        }
    }
}

/// `DOCUMENT(id_or_ids)` — resolve an id, or an array of ids, to documents.
/// An unresolved id yields null and is recorded so the caller can fetch it.
fn eval_document(args: &[Value], ctx: &EvalCtx<'_>) -> Value {
    let Some(source) = ctx.documents else {
        return Value::Null;
    };
    let resolve = |value: &Value| -> Value {
        let Some(id) = value.as_str() else {
            return Value::Null;
        };
        match source.get(id) {
            Some(doc) => doc,
            None => {
                source.miss(id);
                Value::Null
            }
        }
    };
    match &args[0] {
        Value::Array(ids) => Value::Array(ids.iter().map(resolve).collect()),
        single => resolve(single),
    }
}

pub(super) fn eval_expr(
    expr: &Expr,
    env: &Env,
    ctx: &EvalCtx<'_>,
) -> Result<Value, ExecutionError> {
    match expr {
        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => {
            // Strictly `true`, exactly as FILTER decides — a non-boolean
            // condition takes the else branch rather than being coerced.
            let taken = match eval_expr(condition, env, ctx)? {
                Value::Bool(true) => then_branch,
                _ => else_branch,
            };
            eval_expr(taken, env, ctx)
        }
        Expr::Field { base, name } => {
            let value = eval_expr(base, env, ctx)?;
            Ok(value.get(name).cloned().unwrap_or(Value::Null))
        }
        Expr::Identifier(parts) => eval_identifier(parts, env),
        Expr::BindVar(name) => ctx
            .bind_vars
            .get(name)
            .cloned()
            .ok_or_else(|| ExecutionError::BindVariableNotFound(name.clone())),
        Expr::String(value) => Ok(Value::String(value.clone())),
        Expr::Number(value) => Ok(json!(value)),
        Expr::Bool(value) => Ok(Value::Bool(*value)),
        Expr::Null => Ok(Value::Null),
        Expr::Array(items) => items
            .iter()
            .map(|item| eval_expr(item, env, ctx))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Expr::Object(fields) => {
            let mut obj = Map::new();
            for field in fields {
                obj.insert(field.name.clone(), eval_expr(&field.value, env, ctx)?);
            }
            Ok(Value::Object(obj))
        }
        Expr::Unary { op, expr } => eval_unary(*op, expr, env, ctx),
        Expr::Binary { left, op, right } => eval_binary(left, *op, right, env, ctx),
        Expr::FunctionCall { name, args } => eval_function(name, args, env, ctx),
        // Subqueries are planned into PlanOp::LetSubquery and executed by
        // the pipeline; validation restricts them to LET values.
        Expr::Subquery(_) => Err(ExecutionError::Backend(
            "subquery in unsupported position".into(),
        )),
    }
}

fn eval_identifier(parts: &[String], env: &Env) -> Result<Value, ExecutionError> {
    let Some(root) = parts.first() else {
        return Ok(Value::Null);
    };
    // Walk by reference and clone only the accessed leaf — cloning the
    // root first would copy the whole document per identifier access
    // (measured ~2-4x on scan/COLLECT benchmarks; see benchmarks.md).
    let mut value: &Value = env
        .get(root)
        .ok_or_else(|| ExecutionError::IdentifierNotFound(root.clone()))?;
    // Missing path segments yield null (document-store semantics); only an
    // unknown root identifier is an error, and validation rules that out.
    for part in &parts[1..] {
        match value.get(part) {
            Some(next) => value = next,
            None => return Ok(Value::Null),
        }
    }
    Ok(value.clone())
}

fn eval_unary(
    op: UnaryOp,
    expr: &Expr,
    env: &Env,
    ctx: &EvalCtx<'_>,
) -> Result<Value, ExecutionError> {
    let value = eval_expr(expr, env, ctx)?;
    match op {
        UnaryOp::Not => Ok(Value::Bool(!bool_from(&value)?)),
        UnaryOp::Neg => Ok(json!(-number_from_value(&value)?)),
    }
}

fn eval_binary(
    left: &Expr,
    op: BinaryOp,
    right: &Expr,
    env: &Env,
    ctx: &EvalCtx<'_>,
) -> Result<Value, ExecutionError> {
    if op == BinaryOp::And {
        if !bool_from(&eval_expr(left, env, ctx)?)? {
            return Ok(Value::Bool(false));
        }
        return Ok(Value::Bool(bool_from(&eval_expr(right, env, ctx)?)?));
    }
    if op == BinaryOp::Or {
        if bool_from(&eval_expr(left, env, ctx)?)? {
            return Ok(Value::Bool(true));
        }
        return Ok(Value::Bool(bool_from(&eval_expr(right, env, ctx)?)?));
    }

    let left_value = eval_expr(left, env, ctx)?;
    let right_value = eval_expr(right, env, ctx)?;
    match op {
        BinaryOp::Eq => Ok(Value::Bool(values_equal(&left_value, &right_value))),
        BinaryOp::Ne => Ok(Value::Bool(!values_equal(&left_value, &right_value))),
        BinaryOp::Lt => compare_values(&left_value, &right_value, Ordering::Less),
        BinaryOp::Le => compare_ordered(&left_value, &right_value, |ord| {
            ord == Ordering::Less || ord == Ordering::Equal
        }),
        BinaryOp::Gt => compare_values(&left_value, &right_value, Ordering::Greater),
        BinaryOp::Ge => compare_ordered(&left_value, &right_value, |ord| {
            ord == Ordering::Greater || ord == Ordering::Equal
        }),
        BinaryOp::In => Ok(Value::Bool(match right_value {
            Value::Array(items) => items.iter().any(|item| values_equal(item, &left_value)),
            _ => false,
        })),
        BinaryOp::Add => Ok(json!(
            number_from_value(&left_value)? + number_from_value(&right_value)?
        )),
        BinaryOp::Sub => Ok(json!(
            number_from_value(&left_value)? - number_from_value(&right_value)?
        )),
        BinaryOp::Mul => Ok(json!(
            number_from_value(&left_value)? * number_from_value(&right_value)?
        )),
        BinaryOp::Div => Ok(json!(
            number_from_value(&left_value)? / number_from_value(&right_value)?
        )),
        BinaryOp::And | BinaryOp::Or => unreachable!("handled before operand evaluation"),
    }
}

/// Null is falsy in boolean contexts (NOT/AND/OR), consistent with filters.
fn bool_from(value: &Value) -> Result<bool, ExecutionError> {
    match value {
        Value::Bool(b) => Ok(*b),
        Value::Null => Ok(false),
        _ => Err(ExecutionError::ExpectedBool),
    }
}

/// Equality that compares numbers by numeric value, so a stored integer
/// `2` equals the literal `2` even though CGQL literals parse as floats.
pub(super) fn values_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(_), Value::Number(_)) => left.as_f64() == right.as_f64(),
        (Value::Array(l), Value::Array(r)) => {
            l.len() == r.len() && l.iter().zip(r).all(|(a, b)| values_equal(a, b))
        }
        (Value::Object(l), Value::Object(r)) => {
            l.len() == r.len()
                && l.iter()
                    .all(|(k, v)| r.get(k).is_some_and(|rv| values_equal(v, rv)))
        }
        _ => left == right,
    }
}

fn compare_values(
    left: &Value,
    right: &Value,
    expected: Ordering,
) -> Result<Value, ExecutionError> {
    compare_ordered(left, right, |ord| ord == expected)
}

fn compare_ordered(
    left: &Value,
    right: &Value,
    predicate: impl FnOnce(Ordering) -> bool,
) -> Result<Value, ExecutionError> {
    Ok(Value::Bool(match (left, right) {
        (Value::Number(_), Value::Number(_)) => {
            let left = number_from_value(left)?;
            let right = number_from_value(right)?;
            left.partial_cmp(&right).is_some_and(predicate)
        }
        (Value::String(left), Value::String(right)) => predicate(left.cmp(right)),
        _ => false,
    }))
}

fn eval_function(
    name: &str,
    args: &[Expr],
    env: &Env,
    ctx: &EvalCtx<'_>,
) -> Result<Value, ExecutionError> {
    let Some(def) = functions::lookup(name) else {
        return Err(ExecutionError::UnsupportedFunction(name.to_string()));
    };
    if name.eq_ignore_ascii_case("DOCUMENT") {
        // Registered in the function table so name and arity validate like any
        // other, but resolved here because it reads documents, not just values.
        let values = args
            .iter()
            .map(|arg| eval_expr(arg, env, ctx))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(eval_document(&values, ctx));
    }
    let values = args
        .iter()
        .map(|arg| eval_expr(arg, env, ctx))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(def.call(&values))
}

pub(super) fn number_from_value(value: &Value) -> Result<f64, ExecutionError> {
    value.as_f64().ok_or(ExecutionError::ExpectedNumber)
}
