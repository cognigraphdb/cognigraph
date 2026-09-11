use serde_json::Value;

pub(super) fn fn_cosine_similarity(args: &[Value]) -> Value {
    let (Some(a), Some(b)) = (
        args[0]
            .as_array()
            .and_then(|v| v.iter().map(Value::as_f64).collect::<Option<Vec<_>>>()),
        args[1]
            .as_array()
            .and_then(|v| v.iter().map(Value::as_f64).collect::<Option<Vec<_>>>()),
    ) else {
        return Value::Null;
    };
    if a.is_empty() || a.len() != b.len() {
        return Value::Null;
    }
    let dot: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|v| v * v).sum::<f64>().sqrt();
    let nb = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return Value::Null;
    }
    json_number(dot / (na * nb))
}

pub(super) fn number(value: &Value) -> Option<f64> {
    value.as_f64()
}

pub(super) fn json_number(value: f64) -> Value {
    serde_json::Number::from_f64(value).map_or(Value::Null, Value::Number)
}

pub(super) fn fn_abs(args: &[Value]) -> Value {
    number(&args[0]).map_or(Value::Null, |n| json_number(n.abs()))
}

pub(super) fn fn_floor(args: &[Value]) -> Value {
    number(&args[0]).map_or(Value::Null, |n| json_number(n.floor()))
}

pub(super) fn fn_ceil(args: &[Value]) -> Value {
    number(&args[0]).map_or(Value::Null, |n| json_number(n.ceil()))
}

pub(super) fn fn_round(args: &[Value]) -> Value {
    number(&args[0]).map_or(Value::Null, |n| json_number(n.round()))
}

fn numeric_items(value: &Value) -> Option<Vec<f64>> {
    Some(value.as_array()?.iter().filter_map(Value::as_f64).collect())
}

pub(super) fn fn_min(args: &[Value]) -> Value {
    match numeric_items(&args[0]) {
        Some(items) => items
            .into_iter()
            .fold(None::<f64>, |acc, n| Some(acc.map_or(n, |a| a.min(n))))
            .map_or(Value::Null, json_number),
        None => Value::Null,
    }
}

pub(super) fn fn_max(args: &[Value]) -> Value {
    match numeric_items(&args[0]) {
        Some(items) => items
            .into_iter()
            .fold(None::<f64>, |acc, n| Some(acc.map_or(n, |a| a.max(n))))
            .map_or(Value::Null, json_number),
        None => Value::Null,
    }
}

pub(super) fn fn_sum(args: &[Value]) -> Value {
    numeric_items(&args[0]).map_or(Value::Null, |items| json_number(items.iter().sum()))
}

pub(super) fn fn_avg(args: &[Value]) -> Value {
    match numeric_items(&args[0]) {
        Some(items) if !items.is_empty() => {
            json_number(items.iter().sum::<f64>() / items.len() as f64)
        }
        _ => Value::Null,
    }
}
