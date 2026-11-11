use std::cmp::Ordering;

use serde_json::{Value, json};

use crate::ast::{SortClause, SortDirection};
use crate::functions;
use crate::planner::LogicalPlan;

use super::eval::{eval_expr, number_from_value};
use super::memory::weight;
use super::run::RunCx;
use super::{Deadline, Env, ExecutionError};

/// Apply the tail clauses — COLLECT, SORT, LIMIT, RETURN [DISTINCT] — to
/// the rows the body pipeline produced (see `run.rs`; v2 moved LET/FILTER
/// into the positional body).
/// COLLECT, SORT and LIMIT — everything that decides WHICH rows survive.
/// The projection is separate so the runner can execute deferred LETs
/// (move-calculations-down) between the two.
pub(super) fn apply_collect_sort_limit(
    plan: &LogicalPlan,
    rows: &mut Vec<Env>,
    cx: &RunCx<'_>,
    stage: &mut usize,
) -> Result<(), ExecutionError> {
    let eval_ctx = cx.eval();
    if let Some(collect) = &plan.collect {
        type Group = (Vec<Value>, usize, Vec<Vec<Value>>, Vec<Value>);
        let mut groups: Vec<Group> = Vec::new();
        // Hash grouping over a canonical key encoding that mirrors
        // `values_equal` exactly (numbers by value, objects key-order
        // insensitive) — first-seen group order and first-seen key VALUES
        // (e.g. int 2 before float 2.0) are preserved.
        let mut group_index: std::collections::HashMap<Vec<u8>, usize> =
            std::collections::HashMap::new();
        let mut canonical = Vec::new();
        // COLLECT reads every row: charged and timed like the stages before it.
        cx.charge_rows(rows.len() as u64)?;
        let metered = cx.memory.enabled();
        for env in rows.iter() {
            cx.deadline.check()?;
            let keys = collect
                .bindings
                .iter()
                .map(|binding| eval_expr(&binding.expr, env, &eval_ctx))
                .collect::<Result<Vec<_>, _>>()?;
            let agg_values = collect
                .aggregates
                .iter()
                .map(|agg| eval_expr(&agg.expr, env, &eval_ctx))
                .collect::<Result<Vec<_>, _>>()?;
            let captured = match &collect.into {
                Some(into) => Some(match &into.projection {
                    Some(projection) => eval_expr(projection, env, &eval_ctx)?,
                    // Bare INTO captures all in-scope variables, sorted by
                    // name for deterministic output.
                    None => Value::Object(
                        env.iter()
                            .collect::<std::collections::BTreeMap<_, _>>()
                            .into_iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ),
                }),
                None => None,
            };
            canonical.clear();
            for key in &keys {
                write_canonical(key, &mut canonical);
            }
            let known = group_index.get(&canonical).copied();
            if metered {
                // A group keeps its first keys and every aggregate input and
                // captured value.
                let kept = agg_values.iter().chain(captured.iter());
                let kept = if known.is_none() {
                    keys.iter().chain(kept).map(weight).sum::<u64>()
                } else {
                    kept.map(weight).sum::<u64>()
                };
                cx.memory.retain(kept)?;
            }
            match known {
                Some(at) => {
                    let (_, count, lists, captures) = &mut groups[at];
                    *count += 1;
                    for (list, value) in lists.iter_mut().zip(agg_values) {
                        list.push(value);
                    }
                    captures.extend(captured);
                }
                None => {
                    group_index.insert(canonical.clone(), groups.len());
                    groups.push((
                        keys,
                        1,
                        agg_values.into_iter().map(|v| vec![v]).collect(),
                        captured.into_iter().collect(),
                    ));
                }
            }
        }
        *rows = groups
            .into_iter()
            .map(|(keys, count, lists, captures)| {
                let mut env = Env::new();
                for (binding, key) in collect.bindings.iter().zip(keys) {
                    env.insert(binding.name.clone(), key);
                }
                for (agg, list) in collect.aggregates.iter().zip(lists) {
                    // Aggregates are the array built-ins applied to the
                    // group's collected values, so null handling matches.
                    let value = functions::lookup(&agg.function)
                        .map(|def| def.call(&[Value::Array(list)]))
                        .unwrap_or(Value::Null);
                    env.insert(agg.name.clone(), value);
                }
                if let Some(into) = &collect.into {
                    env.insert(into.name.clone(), Value::Array(captures));
                }
                if let Some(count_var) = &collect.count_into {
                    env.insert(count_var.clone(), json!(count));
                }
                env
            })
            .collect();
        cx.record_stage(*stage, rows.len());
        *stage += 1;
    }

    if let Some(sort) = &plan.sort {
        sort_rows(rows, sort, cx)?;
        cx.record_stage(*stage, rows.len());
        *stage += 1;
    }

    if let Some(limit) = &plan.limit {
        let resolved = super::limit::resolve_limit(limit, cx.bind_vars, plan.max_limit)?;
        *rows = apply_limit(std::mem::take(rows), resolved);
        cx.record_stage(*stage, rows.len());
        *stage += 1;
    }
    Ok(())
}

/// RETURN (and DISTINCT), over rows that already survived the tail.
pub(super) fn apply_projection(
    plan: &LogicalPlan,
    rows: &mut Vec<Env>,
    cx: &RunCx<'_>,
    stage: &mut usize,
) -> Result<Vec<Value>, ExecutionError> {
    let eval_ctx = cx.eval();
    let deadline = &cx.deadline;
    let projection = plan
        .projection
        .as_ref()
        .ok_or_else(|| ExecutionError::Backend("read query missing RETURN".into()))?;
    let projected = std::mem::take(rows)
        .into_iter()
        .map(|env| {
            deadline.check()?;
            let value = eval_expr(projection, &env, &eval_ctx)?;
            cx.memory.retain_value(&value)?;
            Ok(value)
        })
        .collect::<Result<Vec<_>, ExecutionError>>()?;

    if plan.distinct {
        // Same canonical-key dedup as COLLECT: first occurrence wins, both
        // in order and in value shape (int 2 survives over a later 2.0).
        let mut seen: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();
        let mut unique: Vec<Value> = Vec::new();
        let mut canonical = Vec::new();
        let mut ticks = 0u32;
        for value in projected {
            deadline.check_every(&mut ticks)?;
            canonical.clear();
            write_canonical(&value, &mut canonical);
            if !seen.contains(&canonical) {
                seen.insert(canonical.clone());
                unique.push(value);
            }
        }
        cx.record_stage(*stage, unique.len());
        *stage += 1;
        return Ok(unique);
    }
    cx.record_stage(*stage, projected.len());
    *stage += 1;
    Ok(projected)
}

/// Canonical byte encoding under which two values encode identically IFF
/// `values_equal` holds: numbers collapse to f64 bits (-0.0 folded into
/// 0.0), object keys are sorted, strings and containers length-prefix.
/// serde_json numbers cannot be NaN, so f64 equality is total here.
fn write_canonical(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.push(b'n'),
        Value::Bool(true) => out.push(b't'),
        Value::Bool(false) => out.push(b'f'),
        Value::Number(_) => {
            out.push(b'#');
            let mut number = value.as_f64().unwrap_or(0.0);
            if number == 0.0 {
                number = 0.0; // fold -0.0: values_equal treats them equal
            }
            out.extend_from_slice(&number.to_bits().to_be_bytes());
        }
        Value::String(text) => {
            out.push(b's');
            out.extend_from_slice(&(text.len() as u64).to_be_bytes());
            out.extend_from_slice(text.as_bytes());
        }
        Value::Array(items) => {
            out.push(b'[');
            out.extend_from_slice(&(items.len() as u64).to_be_bytes());
            for item in items {
                write_canonical(item, out);
            }
        }
        Value::Object(fields) => {
            out.push(b'{');
            out.extend_from_slice(&(fields.len() as u64).to_be_bytes());
            let mut names: Vec<&String> = fields.keys().collect();
            names.sort();
            for name in names {
                out.extend_from_slice(&(name.len() as u64).to_be_bytes());
                out.extend_from_slice(name.as_bytes());
                write_canonical(&fields[name], out);
            }
        }
    }
}

fn sort_rows(rows: &mut Vec<Env>, sort: &SortClause, cx: &RunCx<'_>) -> Result<(), ExecutionError> {
    let eval_ctx = cx.eval();
    let ctx = &eval_ctx;
    // SORT reads every row: charged and timed like the stages before it.
    cx.charge_rows(rows.len() as u64)?;
    let mut key_bytes = 0u64;
    let collator = match &sort.collation {
        Some(locale) => {
            let locale: icu::locale::Locale = locale
                .parse()
                .map_err(|_| ExecutionError::InvalidInput(format!("invalid locale `{locale}`")))?;
            Some(
                icu::collator::Collator::try_new(
                    icu::collator::CollatorPreferences::from(&locale),
                    icu::collator::options::CollatorOptions::default(),
                )
                .map_err(|e| ExecutionError::Backend(format!("collator: {e}")))?,
            )
        }
        None => None,
    };
    // Schwartzian by move: rows travel with their keys, so ordering costs
    // zero row clones (the old permute-by-index cloned every row).
    let keyed = std::mem::take(rows)
        .into_iter()
        .map(|env| {
            cx.deadline.check()?;
            let values = sort
                .keys
                .iter()
                .map(|key| eval_expr(&key.expr, &env, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            if cx.memory.enabled() {
                let bytes = values.iter().map(weight).sum::<u64>();
                cx.memory.retain(bytes)?;
                key_bytes = key_bytes.saturating_add(bytes);
            }
            Ok((values, env))
        })
        .collect::<Result<Vec<_>, ExecutionError>>()?;
    let keyed = sort_within_deadline(keyed, &cx.deadline, |(left, _), (right, _)| {
        for (position, key) in sort.keys.iter().enumerate() {
            let ord = compare_sort_key(
                &left[position],
                &right[position],
                key.direction,
                collator.as_ref(),
            );
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    })?;
    *rows = keyed.into_iter().map(|(_, env)| env).collect();
    // The keys are dropped with the sort.
    cx.memory.release(key_bytes);
    Ok(())
}

/// Rows per sorted run when a deadline applies.
const SORT_RUN: usize = 1 << 14;

/// A stable sort that can stop. Without a deadline, or for a short input, it
/// is the standard sort. Otherwise it sorts fixed-size runs and merges them
/// pairwise, reading the clock between runs and once every 1,024 merged
/// rows, so a large `SORT` ends with the query's time budget instead of
/// outliving it (CG-111).
pub(super) fn sort_within_deadline<T>(
    mut items: Vec<T>,
    deadline: &Deadline,
    compare: impl Fn(&T, &T) -> Ordering,
) -> Result<Vec<T>, ExecutionError> {
    if !deadline.can_stop() || items.len() <= SORT_RUN {
        items.sort_by(&compare);
        return Ok(items);
    }
    let mut runs: Vec<Vec<T>> = Vec::with_capacity(items.len().div_ceil(SORT_RUN));
    let mut source = items.into_iter();
    loop {
        let mut run: Vec<T> = source.by_ref().take(SORT_RUN).collect();
        if run.is_empty() {
            break;
        }
        deadline.check()?;
        run.sort_by(&compare);
        runs.push(run);
    }
    while runs.len() > 1 {
        let mut merged = Vec::with_capacity(runs.len().div_ceil(2));
        let mut pending = runs.into_iter();
        while let Some(left) = pending.next() {
            match pending.next() {
                Some(right) => merged.push(merge_runs(left, right, deadline, &compare)?),
                None => merged.push(left),
            }
        }
        runs = merged;
    }
    Ok(runs.pop().unwrap_or_default())
}

/// Merge two sorted runs, taking from the left on ties to stay stable.
fn merge_runs<T>(
    left: Vec<T>,
    right: Vec<T>,
    deadline: &Deadline,
    compare: &impl Fn(&T, &T) -> Ordering,
) -> Result<Vec<T>, ExecutionError> {
    let mut out = Vec::with_capacity(left.len() + right.len());
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    let mut ticks = 0u32;
    loop {
        deadline.check_every(&mut ticks)?;
        let take_right = match (left.peek(), right.peek()) {
            (Some(a), Some(b)) => compare(b, a) == Ordering::Less,
            (Some(_), None) => {
                out.extend(left);
                break;
            }
            (None, _) => {
                out.extend(right);
                break;
            }
        };
        out.extend(if take_right {
            right.next()
        } else {
            left.next()
        });
    }
    Ok(out)
}

fn compare_sort_key(
    left: &Value,
    right: &Value,
    direction: SortDirection,
    collator: Option<&icu::collator::CollatorBorrowed<'_>>,
) -> Ordering {
    let ord = match (left, right) {
        (Value::Number(_), Value::Number(_)) => number_from_value(left)
            .ok()
            .and_then(|left| {
                number_from_value(right)
                    .ok()
                    .and_then(|right| left.partial_cmp(&right))
            })
            .unwrap_or(Ordering::Equal),
        (Value::String(left), Value::String(right)) => match collator {
            Some(collator) => collator.compare(left, right),
            None => left.cmp(right),
        },
        (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
        _ => Ordering::Equal,
    };
    match direction {
        SortDirection::Asc => ord,
        SortDirection::Desc => ord.reverse(),
    }
}

fn apply_limit(rows: Vec<Env>, limit: super::limit::ResolvedLimit) -> Vec<Env> {
    rows.into_iter()
        .skip(limit.offset_len())
        .take(limit.count_len())
        .collect()
}

#[cfg(test)]
#[path = "pipeline_sort_tests.rs"]
mod sort_tests;
