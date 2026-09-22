//! CG-85: bind variables in LIMIT. Values are resolved before any storage
//! access with the same error classes as literal limits; presence checks
//! come first, exactly as for every other bind variable.

use std::collections::HashMap;

use cognigraph_query::{InMemoryDataset, parse_and_execute};
use serde_json::{Value, json};

fn dataset() -> InMemoryDataset {
    InMemoryDataset::new().with_collection(
        "documents",
        vec![
            json!({"_id": "documents/a", "_key": "a", "n": 1}),
            json!({"_id": "documents/b", "_key": "b", "n": 2}),
            json!({"_id": "documents/c", "_key": "c", "n": 3}),
        ],
    )
}

fn binds<const N: usize>(items: [(&str, Value); N]) -> HashMap<String, Value> {
    items
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

const COUNT_QUERY: &str = "FOR d IN documents SORT d._key ASC LIMIT @n RETURN d._key";
const OFFSET_QUERY: &str = "FOR d IN documents SORT d._key ASC LIMIT @o, @n RETURN d._key";

fn run(query: &str, vars: HashMap<String, Value>) -> Result<Vec<Value>, String> {
    parse_and_execute(query, &dataset(), &vars).map_err(|e| e.to_string())
}

#[test]
fn bound_count_and_offset_select_rows() {
    assert_eq!(
        run(COUNT_QUERY, binds([("n", json!(2))])).unwrap(),
        vec![json!("a"), json!("b")]
    );
    assert_eq!(
        run(OFFSET_QUERY, binds([("o", json!(1)), ("n", json!(1))])).unwrap(),
        vec![json!("b")]
    );
}

#[test]
fn missing_and_unexpected_bind_variables_are_reported_first() {
    assert_eq!(
        run(COUNT_QUERY, HashMap::new()).unwrap_err(),
        "missing bind variables: n"
    );
    // Presence is checked before values: a missing count wins over an
    // invalid offset.
    assert_eq!(
        run(OFFSET_QUERY, binds([("o", json!(-1))])).unwrap_err(),
        "missing bind variables: n"
    );
    assert_eq!(
        run(COUNT_QUERY, binds([("n", json!(1)), ("extra", json!(1))])).unwrap_err(),
        "unexpected bind variables: extra"
    );
}

#[test]
fn count_must_be_a_non_negative_integer_json_number() {
    for value in [
        json!("5"),
        json!(2.5),
        json!(2.0),
        json!(-1),
        json!(true),
        json!(null),
        json!([2]),
        json!({"n": 2}),
    ] {
        assert_eq!(
            run(COUNT_QUERY, binds([("n", value.clone())])).unwrap_err(),
            "LIMIT count bind variable `@n` must be a non-negative integer",
            "{value}"
        );
    }
}

#[test]
fn offset_must_be_a_non_negative_integer_json_number() {
    for value in [json!("1"), json!(1.5), json!(-1), json!(false), json!(null)] {
        assert_eq!(
            run(OFFSET_QUERY, binds([("o", value.clone()), ("n", json!(1))])).unwrap_err(),
            "LIMIT offset bind variable `@o` must be a non-negative integer",
            "{value}"
        );
    }
}

#[test]
fn bound_count_keeps_the_literal_validation_errors() {
    assert_eq!(
        run(COUNT_QUERY, binds([("n", json!(0))])).unwrap_err(),
        "LIMIT count must be greater than zero"
    );
    assert_eq!(
        run(COUNT_QUERY, binds([("n", json!(10_001))])).unwrap_err(),
        "LIMIT count 10001 exceeds configured maximum 10000"
    );
    assert_eq!(
        run(COUNT_QUERY, binds([("n", json!(u64::MAX))])).unwrap_err(),
        format!("LIMIT count {} exceeds configured maximum 10000", u64::MAX)
    );
}

#[test]
fn offset_past_the_end_yields_no_rows_and_a_zero_offset_is_allowed() {
    assert_eq!(
        run(OFFSET_QUERY, binds([("o", json!(10)), ("n", json!(2))])).unwrap(),
        Vec::<Value>::new()
    );
    assert_eq!(
        run(OFFSET_QUERY, binds([("o", json!(0)), ("n", json!(1))])).unwrap(),
        vec![json!("a")]
    );
    // A huge offset is a valid u64; it simply skips everything.
    assert_eq!(
        run(
            OFFSET_QUERY,
            binds([("o", json!(u64::MAX)), ("n", json!(1))])
        )
        .unwrap(),
        Vec::<Value>::new()
    );
}

#[test]
fn explain_renders_bind_names_without_needing_values() {
    let plain = run(
        "EXPLAIN FOR d IN documents LIMIT @o, @n RETURN d",
        HashMap::new(),
    )
    .unwrap();
    let text = serde_json::to_string(&plain).unwrap();
    assert!(text.contains("LIMIT @o, @n"), "{text}");
    assert!(text.contains("\"fetch_limit\":\"@o + @n\""), "{text}");

    let literal = run(
        "EXPLAIN FOR d IN documents LIMIT 3, 2 RETURN d",
        HashMap::new(),
    )
    .unwrap();
    let text = serde_json::to_string(&literal).unwrap();
    assert!(text.contains("LIMIT 3, 2"), "{text}");
    assert!(text.contains("\"fetch_limit\":5"), "{text}");
}

#[test]
fn explain_analyze_executes_with_bound_limits() {
    let report = run(
        "EXPLAIN ANALYZE FOR d IN documents SORT d._key ASC LIMIT @o, @n RETURN d._key",
        binds([("o", json!(1)), ("n", json!(1))]),
    )
    .unwrap();
    assert_eq!(report.len(), 1);
    assert_eq!(report[0]["analyze"], json!(true));
    let text = serde_json::to_string(&report).unwrap();
    assert!(text.contains("LIMIT @o, @n"), "{text}");
}

#[test]
fn a_bind_shared_between_limit_and_projection_is_declared_once() {
    assert_eq!(
        run(
            "FOR d IN documents SORT d._key ASC LIMIT @n RETURN { k: d._key, n: @n }",
            binds([("n", json!(1))])
        )
        .unwrap(),
        vec![json!({"k": "a", "n": 1})]
    );
}
