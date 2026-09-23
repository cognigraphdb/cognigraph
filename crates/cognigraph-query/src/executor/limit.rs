//! Bound LIMIT operands (CG-85): resolved from bind variables before any
//! storage access, with the same errors literal limits raise at validation.

use serde_json::Value;

use super::{BindVars, ExecutionError};
use crate::ast::{LimitClause, LimitValue};
use crate::planner::{LogicalPlan, PlanError, PlanOp};
use crate::validation::ValidationError;

/// A LIMIT with both operands known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResolvedLimit {
    pub offset: u64,
    pub count: u64,
}

impl ResolvedLimit {
    /// Rows a scan must fetch to satisfy offset + count.
    pub fn fetch_len(self) -> usize {
        to_len(self.offset.saturating_add(self.count))
    }
    pub fn offset_len(self) -> usize {
        to_len(self.offset)
    }
    pub fn count_len(self) -> usize {
        to_len(self.count)
    }
}

fn to_len(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn validation(error: ValidationError) -> ExecutionError {
    ExecutionError::Plan(PlanError::Validation(error))
}

fn resolve_value(
    value: &LimitValue,
    role: &str,
    bind_vars: &BindVars,
) -> Result<u64, ExecutionError> {
    match value {
        LimitValue::Literal(n) => Ok(*n),
        LimitValue::Bind { bind } => bind_vars
            .get(bind)
            .ok_or_else(|| ExecutionError::BindVariableNotFound(bind.clone()))?
            .as_u64()
            .ok_or_else(|| {
                validation(ValidationError::LimitBindNotInteger {
                    role: role.to_string(),
                    name: bind.clone(),
                })
            }),
    }
}

/// Resolve both operands and apply the literal rules: count above zero and
/// at most the plan's configured maximum.
pub(super) fn resolve_limit(
    limit: &LimitClause,
    bind_vars: &BindVars,
    max_limit: u64,
) -> Result<ResolvedLimit, ExecutionError> {
    let offset = match &limit.offset {
        Some(value) => resolve_value(value, "offset", bind_vars)?,
        None => 0,
    };
    let count = resolve_value(&limit.count, "count", bind_vars)?;
    if count == 0 {
        return Err(validation(ValidationError::ZeroLimit));
    }
    if count > max_limit {
        return Err(validation(ValidationError::LimitTooLarge {
            count,
            max: max_limit,
        }));
    }
    Ok(ResolvedLimit { offset, count })
}

/// Resolve every LIMIT in the plan tree so an invalid bound value fails
/// before the first scan, like a missing bind variable does.
pub(super) fn precheck(plan: &LogicalPlan, bind_vars: &BindVars) -> Result<(), ExecutionError> {
    if let Some(limit) = &plan.limit {
        resolve_limit(limit, bind_vars, plan.max_limit)?;
    }
    for op in &plan.body {
        if let PlanOp::LetSubquery { plan, .. } = op {
            precheck(plan, bind_vars)?;
        }
    }
    Ok(())
}

/// EXPLAIN text: `3, 2` or `@o, @n`.
pub(super) fn render(limit: &LimitClause) -> String {
    match &limit.offset {
        Some(offset) => format!("{offset}, {}", limit.count),
        None => limit.count.to_string(),
    }
}

/// EXPLAIN pushdown total: a number for literals, `@o + @n` text otherwise.
pub(super) fn render_pushdown(limit: &LimitClause) -> Value {
    match (&limit.offset, &limit.count) {
        (None, LimitValue::Literal(count)) => Value::from(*count),
        (Some(LimitValue::Literal(offset)), LimitValue::Literal(count)) => {
            Value::from(offset.saturating_add(*count))
        }
        (Some(offset), count) => Value::from(format!("{offset} + {count}")),
        (None, count) => Value::from(count.to_string()),
    }
}
