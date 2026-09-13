//! Regular-expression functions.
//!
//! Patterns are compiled once and cached: a FILTER over a 27k-row collection
//! calls the same function once per row, and recompiling the identical pattern
//! each time is pure waste. The cache is bounded — an unbounded map keyed by
//! user input is a memory-growth vector.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use regex::{Regex, RegexBuilder};
use serde_json::{Value, json};

/// Plenty for real queries, where the pattern set is fixed by the query text.
const CACHE_CAPACITY: usize = 256;

fn compiled(pattern: &str, case_insensitive: bool) -> Option<Regex> {
    static CACHE: OnceLock<Mutex<HashMap<(String, bool), Regex>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (pattern.to_string(), case_insensitive);
    // A poisoned lock must not take the query down: fall back to compiling.
    if let Ok(guard) = cache.lock()
        && let Some(found) = guard.get(&key)
    {
        return Some(found.clone());
    }
    let built = RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .size_limit(1 << 20)
        .build()
        .ok()?;
    if let Ok(mut guard) = cache.lock() {
        if guard.len() >= CACHE_CAPACITY {
            guard.clear();
        }
        guard.insert(key, built.clone());
    }
    Some(built)
}

fn case_insensitive(args: &[Value], index: usize) -> bool {
    args.get(index).and_then(Value::as_bool).unwrap_or(false)
}

/// `REGEX_TEST(text, pattern [, case_insensitive])` → bool.
/// A non-string argument or an uncompilable pattern is null, never false —
/// "did not match" and "could not ask" are different answers.
pub(super) fn fn_regex_test(args: &[Value]) -> Value {
    let (Some(text), Some(pattern)) = (args[0].as_str(), args[1].as_str()) else {
        return Value::Null;
    };
    match compiled(pattern, case_insensitive(args, 2)) {
        Some(re) => json!(re.is_match(text)),
        None => Value::Null,
    }
}

/// `REGEX_REPLACE(text, pattern, replacement [, case_insensitive])` → string.
/// Replaces every match; `$1`/`${name}` in the replacement refer to groups.
pub(super) fn fn_regex_replace(args: &[Value]) -> Value {
    let (Some(text), Some(pattern), Some(replacement)) =
        (args[0].as_str(), args[1].as_str(), args[2].as_str())
    else {
        return Value::Null;
    };
    match compiled(pattern, case_insensitive(args, 3)) {
        Some(re) => json!(re.replace_all(text, replacement).into_owned()),
        None => Value::Null,
    }
}
