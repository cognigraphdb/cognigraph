//! CG-105: query text is bounded before the recursive parser sees it.

use std::collections::HashMap;

use cognigraph_query::{
    InMemoryDataset, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NESTING, MAX_QUERY_BYTES,
    parse_and_execute, parse_query,
};
use serde_json::{Value, json};

/// Run `work` on a thread with a small, explicit stack. A regression that lets
/// the parser recurse on hostile input overflows here instead of passing by
/// luck on a roomier thread; the server's worker threads have 2 MiB.
fn on_small_stack<T: Send + 'static>(kib: usize, work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .name("cgql-limit".into())
        .stack_size(kib * 1024)
        .spawn(work)
        .unwrap()
        .join()
        .unwrap()
}

fn rejected(query: String) -> String {
    on_small_stack(256, move || parse_query(&query).unwrap_err().to_string())
}

/// Parse and execute on an explicit stack. The limits are sized for release
/// builds on the server's 2 MiB worker threads; unoptimized test builds use
/// several times more stack per level, so accepted queries run on 8 MiB here.
fn run(query: String) -> Vec<Value> {
    on_small_stack(8 * 1024, move || {
        let dataset = InMemoryDataset::new().with_collection(
            "docs",
            vec![json!({"_id": "docs/a", "n": 1, "not": true, "tags": ["x"]})],
        );
        parse_and_execute(&query, &dataset, &HashMap::new()).unwrap()
    })
}

const HOSTILE_DEPTH: usize = 100_000;

#[test]
fn every_recursive_construct_is_refused_before_parsing() {
    let n = HOSTILE_DEPTH;
    let cases = [
        (
            "parentheses",
            format!("RETURN {}1{}", "(".repeat(n), ")".repeat(n)),
        ),
        (
            "arrays",
            format!("RETURN {}1{}", "[".repeat(n), "]".repeat(n)),
        ),
        (
            "objects",
            format!("RETURN {}1{}", "{a:".repeat(n), "}".repeat(n)),
        ),
        (
            "calls",
            format!("RETURN {}1{}", "ABS(".repeat(n), ")".repeat(n)),
        ),
        ("not", format!("RETURN {}true", "NOT ".repeat(n))),
        ("lowercase not", format!("RETURN {}true", "not ".repeat(n))),
        ("minus", format!("RETURN {}1", "- ".repeat(n))),
        ("ternary", format!("RETURN {}1", "true ? 1 : ".repeat(n))),
        (
            "ternary middle",
            format!("RETURN {}1{}", "true ? ".repeat(n), " : 1".repeat(n)),
        ),
        (
            "subqueries",
            format!("RETURN {}1{}", "(RETURN ".repeat(n), ")".repeat(n)),
        ),
        ("unbalanced", format!("RETURN {}", "(".repeat(n))),
        (
            "mixed",
            format!("RETURN {}1", "[NOT (true ? ".repeat(n / 4)),
        ),
    ];
    for (name, query) in cases {
        if query.len() > MAX_QUERY_BYTES {
            // Long enough to hit the length limit first; shorten to test nesting.
            let short: String = query.chars().take(MAX_QUERY_BYTES).collect();
            let error = rejected(short);
            assert!(error.contains("nesting exceeds"), "{name}: {error}");
            continue;
        }
        let error = rejected(query);
        assert!(error.contains("nesting exceeds"), "{name}: {error}");
    }
}

#[test]
fn long_operator_chains_are_refused_with_a_hint() {
    for chain in [" + 1", " OR true", " AND true", " * 2", " - 1"] {
        let query = format!("RETURN 1{}", chain.repeat(HOSTILE_DEPTH / 10));
        let error = rejected(query);
        assert!(error.contains("depth exceeds"), "{chain}: {error}");
        assert!(error.contains("bind variable"), "{chain}: {error}");
    }
}

#[test]
fn query_text_length_is_bounded() {
    let padding = " ".repeat(MAX_QUERY_BYTES);
    let error = rejected(format!("RETURN 1{padding}"));
    assert!(error.contains("bytes; the limit is"), "{error}");
    // Exactly at the limit is accepted.
    let query = format!("RETURN 1{}", " ".repeat(MAX_QUERY_BYTES - "RETURN 1".len()));
    assert_eq!(query.len(), MAX_QUERY_BYTES);
    assert_eq!(run(query), vec![json!(1.0)]);
}

#[test]
fn queries_at_the_limits_parse_and_execute() {
    let nest = MAX_EXPRESSION_NESTING;
    // Brackets at the nesting limit, one construct at a time.
    assert_eq!(
        run(format!("RETURN {}1{}", "(".repeat(nest), ")".repeat(nest))),
        vec![json!(1.0)]
    );
    let nested_array = run(format!("RETURN {}1{}", "[".repeat(nest), "]".repeat(nest)));
    let mut value = &nested_array[0];
    for _ in 0..nest {
        value = &value[0];
    }
    assert_eq!(value, &json!(1.0));
    // An even number of prefix operators cancels out.
    assert_eq!(
        run(format!("RETURN {}true", "NOT ".repeat(nest))),
        vec![json!(true)]
    );
    assert_eq!(
        run(format!("RETURN {}7", "true ? ".repeat(nest)) + &" : 0".repeat(nest)),
        vec![json!(7.0)]
    );
    // One level more is refused.
    let error = rejected(format!(
        "RETURN {}1{}",
        "(".repeat(nest + 1),
        ")".repeat(nest + 1)
    ));
    assert!(error.contains("nesting exceeds"), "{error}");

    // A chain at the depth limit folds into the deepest tree the later passes see.
    let terms = MAX_EXPRESSION_DEPTH;
    assert_eq!(
        run(format!("RETURN 1{}", " + 1".repeat(terms))),
        vec![json!(terms as f64 + 1.0)]
    );
    let error = rejected(format!("RETURN 1{}", " + 1".repeat(terms + 1)));
    assert!(error.contains("depth exceeds"), "{error}");
    // The worst allowed combination: a full chain inside full nesting.
    let chain = " + 1".repeat(MAX_EXPRESSION_DEPTH - nest);
    assert_eq!(
        run(format!(
            "RETURN {}1{chain}{}",
            "(".repeat(nest),
            ")".repeat(nest)
        )),
        vec![json!((MAX_EXPRESSION_DEPTH - nest) as f64 + 1.0)]
    );
}

#[test]
fn ordinary_wide_queries_are_not_mistaken_for_deep_ones() {
    // Commas and clause keywords end a chain, so width is not depth.
    let numbers: Vec<String> = (0..5_000).map(|n| n.to_string()).collect();
    let rows = run(format!(
        "FOR d IN docs FILTER d.n IN [{}] RETURN d.n",
        numbers.join(", ")
    ));
    assert_eq!(rows, vec![json!(1)]);

    let ternaries = vec!["d.n == 1 ? 1 : 0"; 500].join(", ");
    let rows = run(format!("FOR d IN docs RETURN LENGTH([{ternaries}])"));
    assert_eq!(rows, vec![json!(500)]);

    let lets: String = (0..300)
        .map(|n| format!("LET v{n} = d.n + 1 > 0 ? 1 : 0 "))
        .collect();
    let rows = run(format!("FOR d IN docs {lets}RETURN v299"));
    assert_eq!(rows, vec![json!(1.0)]);

    let fields: String = (0..500)
        .map(|n| format!("f{n}: d.n + {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let rows = run(format!("FOR d IN docs RETURN {{{fields}}}"));
    assert_eq!(rows[0]["f499"], json!(500.0));
}

#[test]
fn strings_comments_and_field_names_do_not_count_as_structure() {
    let brackets = "((((([[[[[{{{{{?????".repeat(200);
    let rows = run(format!(r#"RETURN "{brackets}""#));
    assert_eq!(rows, vec![json!(brackets)]);

    let escaped = r#"\" ((( \\"#.repeat(200);
    assert_eq!(
        run(format!(r#"RETURN LENGTH("{escaped}") > 0"#)),
        vec![json!(true)]
    );

    let comment = "(".repeat(500);
    let rows = run(format!("// {comment}\nRETURN 1 // {comment}"));
    assert_eq!(rows, vec![json!(1.0)]);

    // `not`, `and`, `or` and `in` are ordinary field names after a dot.
    assert_eq!(
        run("FOR d IN docs RETURN d.not == d.not".to_string()),
        vec![json!(true)]
    );
    // A field named like an operator does not start a chain either.
    let fields = vec!["d.not"; 400].join(", ");
    assert_eq!(
        run(format!("FOR d IN docs RETURN LENGTH([{fields}])")),
        vec![json!(400)]
    );
}

#[test]
fn malformed_text_gets_a_parse_error_not_a_scanner_panic() {
    for query in [
        "RETURN \"unterminated",
        "RETURN \"ends with escape\\",
        "RETURN )))))",
        "RETURN ]",
        "",
        "?",
        ",,,,",
        "RETURN \u{1F600}",
    ] {
        let query = query.to_string();
        let result = on_small_stack(256, move || parse_query(&query).map(|_| ()));
        assert!(result.is_err(), "malformed text must be a parse error");
    }
}

/// The claim behind the limits: in an optimized build the worst accepted
/// query needs well under the server's 2 MiB worker stack. Unoptimized
/// builds use several times more, so this runs only under `--release`.
#[cfg(not(debug_assertions))]
#[test]
fn worst_accepted_queries_fit_in_half_the_server_stack_when_optimized() {
    let nest = MAX_EXPRESSION_NESTING;
    let chain = " + 1".repeat(MAX_EXPRESSION_DEPTH - nest);
    for query in [
        format!("RETURN {}1{chain}{}", "[".repeat(nest), "]".repeat(nest)),
        format!("RETURN {}1{chain}{}", "(".repeat(nest), ")".repeat(nest)),
        format!("RETURN {}1{chain}{}", "{a:".repeat(nest), "}".repeat(nest)),
        format!("RETURN {}7", "true ? ".repeat(nest)) + &" : 0".repeat(nest),
        format!("RETURN 1{}", " + 1".repeat(MAX_EXPRESSION_DEPTH)),
    ] {
        on_small_stack(1024, move || {
            let dataset = InMemoryDataset::new();
            parse_and_execute(&query, &dataset, &HashMap::new()).unwrap();
        });
    }
}
