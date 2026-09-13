use serde_json::{Value, json};

pub(super) fn fn_length(args: &[Value]) -> Value {
    match &args[0] {
        Value::String(s) => json!(s.chars().count()),
        Value::Array(items) => json!(items.len()),
        Value::Object(obj) => json!(obj.len()),
        _ => Value::Null,
    }
}

pub(super) fn fn_first(args: &[Value]) -> Value {
    args[0]
        .as_array()
        .and_then(|items| items.first().cloned())
        .unwrap_or(Value::Null)
}

pub(super) fn fn_last(args: &[Value]) -> Value {
    args[0]
        .as_array()
        .and_then(|items| items.last().cloned())
        .unwrap_or(Value::Null)
}

pub(super) fn fn_unique(args: &[Value]) -> Value {
    let Some(items) = args[0].as_array() else {
        return Value::Null;
    };
    let mut out: Vec<Value> = Vec::new();
    for item in items {
        let duplicate = out.iter().any(|seen| match (seen, item) {
            (Value::Number(_), Value::Number(_)) => seen.as_f64() == item.as_f64(),
            _ => seen == item,
        });
        if !duplicate {
            out.push(item.clone());
        }
    }
    Value::Array(out)
}

pub(super) fn fn_has(args: &[Value]) -> Value {
    match (&args[0], args[1].as_str()) {
        (Value::Object(obj), Some(key)) => json!(obj.contains_key(key)),
        _ => Value::Null,
    }
}

pub(super) fn fn_typename(args: &[Value]) -> Value {
    json!(match &args[0] {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    })
}

/// First argument that is not null; null when every argument is null.
///
/// The null-defaulting CGQL had no way to express: without it, a numeric that
/// may be missing (`spent_hours ?? 0`) forced callers to exclude those rows
/// entirely, which silently changes the answer rather than the formatting
/// (decision_cgql_v2_workload_gaps.md, D3).
pub(super) fn fn_coalesce(args: &[Value]) -> Value {
    args.iter()
        .find(|value| !value.is_null())
        .cloned()
        .unwrap_or(Value::Null)
}

/// The `collection/key` string an argument denotes: the value itself when it is
/// an id string, or its `_id` when it is a document.
fn identifier_of(value: &Value) -> Option<&str> {
    match value {
        Value::String(id) => Some(id.as_str()),
        Value::Object(_) => value.get("_id").and_then(Value::as_str),
        _ => None,
    }
}

/// `IS_SAME_COLLECTION(collection, doc_or_id)` → bool.
///
/// A heterogeneous edge collection (one edge collection pointing at two vertex
/// collections) previously had no discriminator but `STARTS_WITH(e._to,
/// "persons/")` — string surgery that breaks silently on a rename
/// (decision_cgql_v2_workload_gaps.md, D4).
pub(super) fn fn_is_same_collection(args: &[Value]) -> Value {
    let Some(expected) = args[0].as_str() else {
        return Value::Null;
    };
    let Some(id) = identifier_of(&args[1]) else {
        return Value::Null;
    };
    match id.split_once('/') {
        Some((collection, _)) => Value::Bool(collection == expected),
        None => Value::Null,
    }
}

/// `PARSE_IDENTIFIER(doc_or_id)` → `{ collection, key }`, or null when the
/// argument is not a `collection/key` identifier.
pub(super) fn fn_parse_identifier(args: &[Value]) -> Value {
    let Some(id) = identifier_of(&args[0]) else {
        return Value::Null;
    };
    match id.split_once('/') {
        // A key may itself contain '/', so split once from the left only.
        Some((collection, key)) if !collection.is_empty() && !key.is_empty() => {
            json!({ "collection": collection, "key": key })
        }
        _ => Value::Null,
    }
}

/// Total order over values for `SORTED`/`SORTED_UNIQUE`.
///
/// Same-type comparison is exactly the SORT clause's — numbers by f64 value,
/// strings byte-wise, `false < true` — so `FOR x IN SORTED(a)` and
/// `FOR x IN a SORT x ASC` agree wherever the clause defines an order. The
/// clause leaves mixed types unordered (it compares them Equal); a function
/// returning an array cannot, so mixed types take AQL's ladder:
/// null < bool < number < string < array < object. Arrays compare
/// element-wise then by length, objects by sorted key list then per-key
/// value, which makes `same_value`-equal elements compare Equal — the sort
/// keeps them adjacent for `SORTED_UNIQUE`'s dedup pass.
fn total_order(left: &Value, right: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    fn rank(value: &Value) -> u8 {
        match value {
            Value::Null => 0,
            Value::Bool(_) => 1,
            Value::Number(_) => 2,
            Value::String(_) => 3,
            Value::Array(_) => 4,
            Value::Object(_) => 5,
        }
    }
    match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Bool(l), Value::Bool(r)) => l.cmp(r),
        (Value::Number(_), Value::Number(_)) => left
            .as_f64()
            .partial_cmp(&right.as_f64())
            .unwrap_or(Ordering::Equal),
        (Value::String(l), Value::String(r)) => l.cmp(r),
        (Value::Array(l), Value::Array(r)) => l
            .iter()
            .zip(r)
            .map(|(a, b)| total_order(a, b))
            .find(|ord| *ord != Ordering::Equal)
            .unwrap_or_else(|| l.len().cmp(&r.len())),
        (Value::Object(l), Value::Object(r)) => {
            let mut lk: Vec<&String> = l.keys().collect();
            let mut rk: Vec<&String> = r.keys().collect();
            lk.sort();
            rk.sort();
            lk.iter()
                .zip(&rk)
                .map(|(a, b)| a.cmp(b))
                .find(|ord| *ord != Ordering::Equal)
                .unwrap_or_else(|| lk.len().cmp(&rk.len()))
                .then_with(|| {
                    lk.iter()
                        .map(|key| total_order(&l[key.as_str()], &r[key.as_str()]))
                        .find(|ord| *ord != Ordering::Equal)
                        .unwrap_or(Ordering::Equal)
                })
        }
        _ => rank(left).cmp(&rank(right)),
    }
}

/// `SORTED(array)`: the elements in ascending `total_order`. The sort is
/// stable, so ties (`1` and `1.0`) keep their input order. Non-array → null.
pub(super) fn fn_sorted(args: &[Value]) -> Value {
    let Some(items) = args[0].as_array() else {
        return Value::Null;
    };
    let mut out = items.clone();
    out.sort_by(total_order);
    Value::Array(out)
}

/// `SORTED_UNIQUE(array)`: `SORTED` then deduplicated under `UNIQUE`'s
/// equality, keeping the first of each tie. Non-array → null.
pub(super) fn fn_sorted_unique(args: &[Value]) -> Value {
    let Value::Array(mut out) = fn_sorted(args) else {
        return Value::Null;
    };
    out.dedup_by(|a, b| same_value(a, b));
    Value::Array(out)
}

/// Equality used by the set-shaped array functions, identical to `UNIQUE`'s:
/// numbers compare as f64 so `1` and `1.0` are one value; everything else
/// compares structurally.
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => a.as_f64() == b.as_f64(),
        _ => a == b,
    }
}

/// Resolve an AQL-style index against a length: negative counts from the end,
/// and anything out of range clamps rather than erroring.
fn clamp_index(raw: f64, len: usize) -> usize {
    if raw < 0.0 {
        let from_end = len as f64 + raw;
        from_end.max(0.0) as usize
    } else {
        (raw as usize).min(len)
    }
}

/// `SLICE(array, start [, length])`. Negative `start` counts from the end; a
/// negative `length` drops that many elements from the end instead.
pub(super) fn fn_slice(args: &[Value]) -> Value {
    let Some(items) = args[0].as_array() else {
        return Value::Null;
    };
    let Some(start_raw) = args[1].as_f64() else {
        return Value::Null;
    };
    let len = items.len();
    let start = clamp_index(start_raw, len);
    let end = match args.get(2) {
        None | Some(Value::Null) => len,
        Some(value) => {
            let Some(count) = value.as_f64() else {
                return Value::Null;
            };
            if count < 0.0 {
                // Negative length: stop that many before the end.
                clamp_index(count, len).max(start)
            } else {
                (start + count as usize).min(len)
            }
        }
    };
    Value::Array(items[start.min(end)..end].to_vec())
}

/// `FLATTEN(array [, depth])` — depth defaults to 1, matching AQL.
pub(super) fn fn_flatten(args: &[Value]) -> Value {
    let Some(items) = args[0].as_array() else {
        return Value::Null;
    };
    let depth = match args.get(1) {
        None | Some(Value::Null) => 1i64,
        Some(value) => match value.as_f64() {
            Some(d) if d >= 0.0 => d as i64,
            _ => return Value::Null,
        },
    };
    fn walk(items: &[Value], depth: i64, out: &mut Vec<Value>) {
        for item in items {
            match item {
                Value::Array(inner) if depth > 0 => walk(inner, depth - 1, out),
                other => out.push(other.clone()),
            }
        }
    }
    let mut out = Vec::new();
    walk(items, depth, &mut out);
    Value::Array(out)
}

/// `INTERSECTION(a, b, ...)` — values present in EVERY argument, deduped,
/// in the order they appear in the first.
pub(super) fn fn_intersection(args: &[Value]) -> Value {
    let Some(first) = args[0].as_array() else {
        return Value::Null;
    };
    let mut others = Vec::with_capacity(args.len() - 1);
    for arg in &args[1..] {
        match arg.as_array() {
            Some(items) => others.push(items),
            None => return Value::Null,
        }
    }
    let mut out: Vec<Value> = Vec::new();
    for item in first {
        if others
            .iter()
            .all(|other| other.iter().any(|candidate| same_value(candidate, item)))
            && !out.iter().any(|seen| same_value(seen, item))
        {
            out.push(item.clone());
        }
    }
    Value::Array(out)
}

/// `MINUS(a, b, ...)` — values in the first argument that appear in none of
/// the others, deduped.
pub(super) fn fn_minus(args: &[Value]) -> Value {
    let Some(first) = args[0].as_array() else {
        return Value::Null;
    };
    let mut others = Vec::with_capacity(args.len() - 1);
    for arg in &args[1..] {
        match arg.as_array() {
            Some(items) => others.push(items),
            None => return Value::Null,
        }
    }
    let mut out: Vec<Value> = Vec::new();
    for item in first {
        if !others
            .iter()
            .any(|other| other.iter().any(|candidate| same_value(candidate, item)))
            && !out.iter().any(|seen| same_value(seen, item))
        {
            out.push(item.clone());
        }
    }
    Value::Array(out)
}

/// `TO_NUMBER(v)` — number as-is, numeric string parsed, bool as 1/0.
///
/// Anything else is null rather than 0. AQL returns 0 for unconvertible input;
/// CGQL's own convention throughout this registry is "wrong type → null", and a
/// silent 0 inside a SUM is a wrong answer rather than a missing one.
pub(super) fn fn_to_number(args: &[Value]) -> Value {
    match &args[0] {
        Value::Number(_) => args[0].clone(),
        Value::Bool(b) => json!(if *b { 1 } else { 0 }),
        Value::String(s) => match s.trim().parse::<f64>() {
            Ok(n) if n.is_finite() => json!(n),
            _ => Value::Null,
        },
        _ => Value::Null,
    }
}

/// `TO_STRING(v)` — scalars rendered; null stays null; arrays/objects null.
pub(super) fn fn_to_string(args: &[Value]) -> Value {
    match &args[0] {
        Value::String(_) => args[0].clone(),
        // Every CGQL numeric LITERAL is an f64, so a plain `42` would otherwise
        // render as "42.0". Integral values print without the fraction.
        Value::Number(n) => match n.as_f64() {
            Some(v) if v.fract() == 0.0 && v.abs() < 9e15 => json!((v as i64).to_string()),
            _ => json!(n.to_string()),
        },
        Value::Bool(b) => json!(b.to_string()),
        _ => Value::Null,
    }
}

/// `TO_BOOL(v)` — bool as-is; a number is true when non-zero; a string is true
/// when non-empty. Null and containers are null, not false.
pub(super) fn fn_to_bool(args: &[Value]) -> Value {
    match &args[0] {
        Value::Bool(_) => args[0].clone(),
        Value::Number(n) => json!(n.as_f64().is_some_and(|v| v != 0.0)),
        Value::String(s) => json!(!s.is_empty()),
        _ => Value::Null,
    }
}
