use serde_json::{Value, json};

use super::numeric::number;

pub(super) fn fn_upper(args: &[Value]) -> Value {
    args[0]
        .as_str()
        .map_or(Value::Null, |s| json!(s.to_uppercase()))
}

pub(super) fn fn_lower(args: &[Value]) -> Value {
    args[0]
        .as_str()
        .map_or(Value::Null, |s| json!(s.to_lowercase()))
}

pub(super) fn fn_trim(args: &[Value]) -> Value {
    args[0].as_str().map_or(Value::Null, |s| json!(s.trim()))
}

pub(super) fn fn_contains(args: &[Value]) -> Value {
    match (args[0].as_str(), args[1].as_str()) {
        (Some(haystack), Some(needle)) => json!(haystack.contains(needle)),
        _ => Value::Null,
    }
}

pub(super) fn fn_starts_with(args: &[Value]) -> Value {
    match (args[0].as_str(), args[1].as_str()) {
        (Some(s), Some(prefix)) => json!(s.starts_with(prefix)),
        _ => Value::Null,
    }
}

pub(super) fn fn_substring(args: &[Value]) -> Value {
    let (Some(s), Some(start)) = (args[0].as_str(), number(&args[1])) else {
        return Value::Null;
    };
    if start < 0.0 {
        return Value::Null;
    }
    let start = start as usize;
    let chars: Vec<char> = s.chars().collect();
    let end = match args.get(2) {
        Some(len_value) => {
            let Some(len) = number(len_value) else {
                return Value::Null;
            };
            if len < 0.0 {
                return Value::Null;
            }
            (start + len as usize).min(chars.len())
        }
        None => chars.len(),
    };
    if start >= chars.len() {
        return json!("");
    }
    json!(chars[start..end].iter().collect::<String>())
}

pub(super) fn fn_concat(args: &[Value]) -> Value {
    let mut out = String::new();
    for arg in args {
        match arg {
            Value::Null => {}
            Value::String(s) => out.push_str(s),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            _ => return Value::Null,
        }
    }
    json!(out)
}

pub(super) fn fn_split(args: &[Value]) -> Value {
    match (args[0].as_str(), args[1].as_str()) {
        (Some(s), Some(sep)) if !sep.is_empty() => {
            json!(s.split(sep).collect::<Vec<_>>())
        }
        _ => Value::Null,
    }
}

pub(super) fn fn_normalize_nfc(args: &[Value]) -> Value {
    use unicode_normalization::UnicodeNormalization;
    args[0]
        .as_str()
        .map_or(Value::Null, |s| json!(s.nfc().collect::<String>()))
}
