use std::collections::HashMap;

use cognigraph_query::{ExecutionError, InMemoryDataset, parse_and_execute};
use serde_json::{Value, json};

#[test]
fn executes_collection_filter_sort_limit_projection() {
    let dataset = documents_dataset();
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FILTER d.category == @category
        SORT d.created_at DESC
        LIMIT 1
        RETURN { id: d._id, title: d.title }
        "#,
        &dataset,
        &binds([("category", json!("research"))]),
    )
    .unwrap();

    assert_eq!(rows, vec![json!({ "id": "documents/b", "title": "Beta" })]);
}

#[test]
fn executes_limit_with_offset() {
    let dataset = documents_dataset();
    let rows = parse_and_execute(
        "FOR d IN documents SORT d.title ASC LIMIT 1, 1 RETURN d.title",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(rows, vec![json!("Beta")]);
}

#[test]
fn executes_boolean_and_in_filters() {
    let dataset = documents_dataset();
    let rows = parse_and_execute(
        r#"FOR d IN documents FILTER d.active == true AND d.category IN ["research"] RETURN d._id"#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(rows, vec![json!("documents/a")]);
}

#[test]
fn executes_length_function() {
    let dataset = documents_dataset();
    let rows = parse_and_execute(
        "FOR d IN documents FILTER LENGTH(d.title) > 4 SORT d.title ASC RETURN d.title",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(rows, vec![json!("Alpha"), json!("Gamma")]);
}

#[test]
fn executes_vector_search_by_cosine_score() {
    let dataset = documents_dataset();
    let rows = parse_and_execute(
        r#"
        FOR d IN VECTOR_SEARCH(documents, @embedding)
        LIMIT 2
        RETURN { id: d._id, score: d._score }
        "#,
        &dataset,
        &binds([("embedding", json!([1.0, 0.0]))]),
    )
    .unwrap();

    assert_eq!(rows[0]["id"], json!("documents/a"));
    assert_eq!(rows[1]["id"], json!("documents/c"));
    assert!(rows[0]["score"].as_f64().unwrap() > rows[1]["score"].as_f64().unwrap());
}

#[test]
fn executes_outbound_traversal() {
    let dataset = graph_dataset();
    let rows = parse_and_execute(
        r#"
        FOR v, e, p IN 1..2 OUTBOUND @start relationships
        FILTER e.confidence >= 0.7
        SORT v._id ASC
        RETURN { vertex: v._id, relation: e.relation_type, depth: p.depth }
        "#,
        &dataset,
        &binds([("start", json!("documents/a"))]),
    )
    .unwrap();

    assert_eq!(
        rows,
        vec![
            json!({ "vertex": "documents/b", "relation": "links", "depth": 1 }),
            json!({ "vertex": "documents/c", "relation": "links", "depth": 2 })
        ]
    );
}

#[test]
fn executes_any_traversal_from_middle_vertex() {
    let dataset = graph_dataset();
    let rows = parse_and_execute(
        r#"
        FOR v, e, p IN 1..1 ANY "documents/b" relationships
        SORT v._id ASC
        RETURN v._id
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(rows, vec![json!("documents/a"), json!("documents/c")]);
}

#[test]
fn returns_collection_not_found_error() {
    let err = parse_and_execute(
        "FOR d IN missing RETURN d",
        &documents_dataset(),
        &HashMap::new(),
    )
    .unwrap_err();

    assert_eq!(err, ExecutionError::CollectionNotFound("missing".into()));
}

#[test]
fn missing_bind_variable_fails_before_execution() {
    // Empty collection on purpose: proves the check is eager, not evaluation-driven.
    let err = parse_and_execute(
        "FOR d IN documents FILTER d.category == @category RETURN d",
        &InMemoryDataset::new().with_collection("documents", vec![]),
        &HashMap::new(),
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "missing bind variables: category");
}

#[test]
fn unexpected_bind_variable_fails() {
    let err = parse_and_execute(
        "FOR d IN documents RETURN d",
        &documents_dataset(),
        &binds([("extra", json!(1))]),
    )
    .unwrap_err();

    assert_eq!(err.to_string(), "unexpected bind variables: extra");
}

#[test]
fn missing_path_returns_null_in_projection() {
    let rows = parse_and_execute(
        "FOR d IN documents SORT d.title ASC LIMIT 1 RETURN d.publisher",
        &documents_dataset(),
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(rows, vec![json!(null)]);
}

#[test]
fn filter_on_missing_field_excludes_instead_of_erroring() {
    let dataset = InMemoryDataset::new().with_collection(
        "documents",
        vec![
            json!({ "_id": "documents/a", "title": "Alpha", "category": "research" }),
            json!({ "_id": "documents/b", "title": "Beta" }),
        ],
    );
    let rows = parse_and_execute(
        r#"FOR d IN documents FILTER d.category == "research" RETURN d.title"#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(rows, vec![json!("Alpha")]);
}

#[test]
fn filter_evaluating_to_null_is_false() {
    let rows = parse_and_execute(
        "FOR d IN documents FILTER d.missing RETURN d",
        &documents_dataset(),
        &HashMap::new(),
    )
    .unwrap();

    assert!(rows.is_empty());
}

#[test]
fn depth_zero_traversal_emits_start_vertex() {
    let rows = parse_and_execute(
        r#"
        FOR v, e, p IN 0..1 OUTBOUND "documents/a" relationships
        SORT p.depth ASC
        RETURN { id: v._id, edge: e, depth: p.depth }
        "#,
        &graph_dataset(),
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        rows[0],
        json!({ "id": "documents/a", "edge": null, "depth": 0 })
    );
    assert!(rows.len() > 1);
}

fn documents_dataset() -> InMemoryDataset {
    InMemoryDataset::new().with_collection(
        "documents",
        vec![
            json!({
                "_id": "documents/a",
                "title": "Alpha",
                "category": "research",
                "created_at": "2026-01-01",
                "active": true,
                "embedding": [1.0, 0.0]
            }),
            json!({
                "_id": "documents/b",
                "title": "Beta",
                "category": "research",
                "created_at": "2026-02-01",
                "active": false,
                "embedding": [0.0, 1.0]
            }),
            json!({
                "_id": "documents/c",
                "title": "Gamma",
                "category": "notes",
                "created_at": "2026-03-01",
                "active": true,
                "embedding": [0.8, 0.2]
            }),
        ],
    )
}

fn graph_dataset() -> InMemoryDataset {
    documents_dataset().with_collection(
        "relationships",
        vec![
            json!({
                "_from": "documents/a",
                "_to": "documents/b",
                "relation_type": "links",
                "confidence": 0.9
            }),
            json!({
                "_from": "documents/b",
                "_to": "documents/c",
                "relation_type": "links",
                "confidence": 0.8
            }),
            json!({
                "_from": "documents/a",
                "_to": "documents/c",
                "relation_type": "weak",
                "confidence": 0.2
            }),
        ],
    )
}

fn binds<const N: usize>(items: [(&str, Value); N]) -> HashMap<String, Value> {
    items
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
}

#[test]
fn explain_analyze_reports_stage_statistics() {
    use serde_json::Value;
    let dataset = corpus_like_dataset();
    let report = cognigraph_query::parse_and_execute(
        "EXPLAIN ANALYZE FOR d IN docs FILTER d.n >= 2 LET double = (FOR x IN docs FILTER x.n == d.n RETURN x.n * 2) RETURN [d.n, double]",
        &dataset,
        &std::collections::HashMap::new(),
    )
    .unwrap();
    assert_eq!(report.len(), 1);
    let doc = &report[0];
    assert_eq!(doc["analyze"], Value::Bool(true));

    // Engine-comparable parts: rows per stage and per source.
    let stages = doc["stages"].as_array().unwrap();
    let stage = |label: &str| -> &Value {
        stages
            .iter()
            .find(|s| s["stage"] == label)
            .unwrap_or_else(|| panic!("missing stage {label}"))
    };
    assert_eq!(stage("FOR d")["rows"], 3);
    assert_eq!(stage("FILTER")["rows"], 2, "n >= 2 keeps two rows");
    // The correlated subquery ran once per surviving outer row.
    assert_eq!(stage("FOR x")["runs"], 2);
    assert_eq!(stage("FOR x")["rows"], 6, "3 scanned rows per run");
    assert_eq!(stage("RETURN")["rows"], 2);
    assert_eq!(doc["stats"]["result_rows"], 2);
    // Sources: the outer scan and the subquery scan, each materialized once.
    let sources = doc["sources"].as_array().unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0]["rows"], 3);
    assert_eq!(sources[1]["rows"], 3, "materialize-once despite 2 runs");
    // Timing fields exist but are not asserted (wall time).
    assert!(doc["stats"]["total_ms"].is_number());
}

fn corpus_like_dataset() -> cognigraph_query::InMemoryDataset {
    cognigraph_query::InMemoryDataset::new().with_collection(
        "docs",
        vec![
            serde_json::json!({"_key": "a", "n": 1}),
            serde_json::json!({"_key": "b", "n": 2}),
            serde_json::json!({"_key": "c", "n": 3}),
        ],
    )
}

/// A correlated equality inside a subquery is served from a hash index built
/// once over the materialized site (decision_cgql_v2_workload_gaps.md, D1b).
/// The index may only NARROW candidates, so every one of these must return
/// exactly what the unindexed expansion returned.
#[test]
fn correlated_subquery_equality_matches_the_unindexed_expansion() {
    let dataset = InMemoryDataset::new()
        .with_collection(
            "people",
            vec![
                json!({ "_id": "people/p1", "name": "one" }),
                json!({ "_id": "people/p2", "name": "two" }),
                json!({ "_id": "people/p3", "name": "three" }),
            ],
        )
        .with_collection(
            "links",
            vec![
                json!({ "_id": "links/l1", "_to": "people/p1", "tag": "keep" }),
                json!({ "_id": "links/l2", "_to": "people/p1", "tag": "drop" }),
                json!({ "_id": "links/l3", "_to": "people/p2", "tag": "keep" }),
                // Non-string endpoint: can never equal a string key, and must
                // not be returned for one.
                json!({ "_id": "links/l4", "_to": 7, "tag": "keep" }),
                // Missing endpoint entirely.
                json!({ "_id": "links/l5", "tag": "keep" }),
            ],
        );

    // Per-person counts: p1 has 2, p2 has 1, p3 has 0.
    let rows = parse_and_execute(
        "FOR p IN people \
         LET r = (FOR e IN links FILTER e._to == p._id RETURN e._id) \
         RETURN { p: p._id, n: LENGTH(r) }",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(
        rows,
        vec![
            json!({ "p": "people/p1", "n": 2 }),
            json!({ "p": "people/p2", "n": 1 }),
            json!({ "p": "people/p3", "n": 0 }),
        ]
    );

    // The correlated equality narrows; a second predicate must still apply.
    let rows = parse_and_execute(
        "FOR p IN people \
         LET r = (FOR e IN links FILTER e._to == p._id AND e.tag == \"keep\" RETURN e._id) \
         RETURN { p: p._id, n: LENGTH(r) }",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(
        rows,
        vec![
            json!({ "p": "people/p1", "n": 1 }),
            json!({ "p": "people/p2", "n": 1 }),
            json!({ "p": "people/p3", "n": 0 }),
        ]
    );

    // Flipped operands resolve to the same index.
    let rows = parse_and_execute(
        "FOR p IN people \
         LET r = (FOR e IN links FILTER p._id == e._to RETURN e._id) \
         RETURN LENGTH(r)",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!(2), json!(1), json!(0)]);

    // A NON-string correlated key must fall back to the full expansion and
    // still find the numeric endpoint.
    let rows = parse_and_execute(
        "FOR n IN [7] LET r = (FOR e IN links FILTER e._to == n RETURN e._id) RETURN r",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!(["links/l4"])]);

    // An UNCORRELATED equality (both sides fixed) is unaffected.
    let rows = parse_and_execute(
        "FOR e IN links FILTER e._to == \"people/p1\" RETURN e._id",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("links/l1"), json!("links/l2")]);
}

/// D3 — a conditional expression. CGQL had no way to default a null, which
/// forced callers to exclude those rows entirely and silently change the answer.
#[test]
fn executes_conditional_expression() {
    let dataset = InMemoryDataset::new().with_collection(
        "rows",
        vec![
            json!({ "_id": "rows/a", "spent": 10 }),
            json!({ "_id": "rows/b" }),
        ],
    );

    // The null-defaulting case that motivated D3.
    let rows = parse_and_execute(
        "FOR r IN rows RETURN r.spent == null ? 0 : r.spent",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    // Numeric LITERALS are f64 throughout CGQL, so `0` renders as 0.0 while the
    // 10 read from the document keeps its integer form. Pre-existing behaviour,
    // unrelated to the conditional.
    assert_eq!(rows, vec![json!(10), json!(0.0)]);

    // COALESCE / NOT_NULL express the same thing without a branch.
    let rows = parse_and_execute(
        "FOR r IN rows RETURN { a: COALESCE(r.spent, 0), b: NOT_NULL(r.missing, r.spent, -1) }",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(
        rows,
        vec![json!({ "a": 10, "b": 10 }), json!({ "a": 0.0, "b": -1.0 }),]
    );

    // A non-boolean condition takes the else branch — same rule FILTER uses,
    // rather than inventing truthiness.
    let rows = parse_and_execute(
        "FOR r IN rows LIMIT 1 RETURN r.spent ? \"yes\" : \"no\"",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("no")]);
}

/// D4 — identifier helpers, so a heterogeneous edge collection has a
/// discriminator that is not string surgery on `_to`.
#[test]
fn executes_identifier_helpers() {
    let dataset = InMemoryDataset::new().with_collection(
        "edges",
        vec![
            json!({ "_id": "edges/e1", "_to": "persons/p1" }),
            json!({ "_id": "edges/e2", "_to": "organizations/o1" }),
        ],
    );

    let rows = parse_and_execute(
        "FOR e IN edges FILTER IS_SAME_COLLECTION(\"persons\", e._to) RETURN e._id",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("edges/e1")]);

    let rows = parse_and_execute(
        "FOR e IN edges LIMIT 1 RETURN PARSE_IDENTIFIER(e._to)",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!({ "collection": "persons", "key": "p1" })]);

    // A document works wherever an id string does (its `_id` is used).
    let rows = parse_and_execute(
        "FOR e IN edges LIMIT 1 RETURN IS_SAME_COLLECTION(\"edges\", e)",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!(true)]);

    // Not an identifier → null, never a wrong answer.
    let rows = parse_and_execute(
        "FOR e IN edges LIMIT 1 RETURN PARSE_IDENTIFIER(42)",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!(null)]);
}

/// D5 — two AQL reflexes that used to be parse errors an evaluator reads as
/// missing capability.
#[test]
fn accepts_shorthand_objects_and_bare_collect_count() {
    let dataset = InMemoryDataset::new().with_collection(
        "rows",
        vec![
            json!({ "_id": "rows/a", "title": "Alpha" }),
            json!({ "_id": "rows/b", "title": "Beta" }),
        ],
    );

    // `{ title }` is `{ title: title }`.
    let rows = parse_and_execute(
        "FOR r IN rows LET title = r.title LIMIT 1 RETURN { title }",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!({ "title": "Alpha" })]);

    // Mixed shorthand and explicit fields in one object.
    let rows = parse_and_execute(
        "FOR r IN rows LET title = r.title LIMIT 1 RETURN { title, id: r._id }",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!({ "title": "Alpha", "id": "rows/a" })]);

    // `COLLECT WITH COUNT INTO n` with no grouping binding.
    let rows = parse_and_execute(
        "FOR r IN rows COLLECT WITH COUNT INTO n RETURN { n }",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!({ "n": 2 })]);
}

/// D5 (third reflex) — a subquery in expression position is lifted into a
/// synthetic LET, so the planner and executor still only ever see the
/// LET-position shape `decision_cgql_v2.md` approved.
#[test]
fn desugars_subqueries_in_expression_position() {
    let dataset = InMemoryDataset::new()
        .with_collection(
            "people",
            vec![
                json!({ "_id": "people/p1", "name": "one" }),
                json!({ "_id": "people/p2", "name": "two" }),
            ],
        )
        .with_collection(
            "links",
            vec![
                json!({ "_id": "links/l1", "_to": "people/p1" }),
                json!({ "_id": "links/l2", "_to": "people/p1" }),
                json!({ "_id": "links/l3", "_to": "people/p2" }),
            ],
        );

    // The reflex: a function applied straight to a subquery.
    let sugared = parse_and_execute(
        "FOR p IN people RETURN LENGTH((FOR e IN links FILTER e._to == p._id RETURN 1))",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    // ...must equal the LET the author would otherwise write by hand.
    let manual = parse_and_execute(
        "FOR p IN people LET r = (FOR e IN links FILTER e._to == p._id RETURN 1) RETURN LENGTH(r)",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(sugared, manual);
    assert_eq!(sugared, vec![json!(2), json!(1)]);

    // In a FILTER, where the lifted LET must land BEFORE the clause using it.
    let rows = parse_and_execute(
        "FOR p IN people \
         FILTER LENGTH((FOR e IN links FILTER e._to == p._id RETURN 1)) > 1 \
         RETURN p._id",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("people/p1")]);

    // Two subqueries in one expression each get their own binding.
    let rows = parse_and_execute(
        "FOR p IN people LIMIT 1 RETURN \
         LENGTH((FOR e IN links RETURN 1)) + LENGTH((FOR q IN people RETURN 1))",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!(5.0)]);

    // Nested: a subquery inside a subquery.
    let rows = parse_and_execute(
        "FOR p IN people LIMIT 1 \
         RETURN (FOR e IN links RETURN LENGTH((FOR q IN people RETURN 1)))",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!([2, 2, 2])]);

    // Inside a conditional, which the hoist must also walk.
    let rows = parse_and_execute(
        "FOR p IN people LIMIT 1 \
         RETURN LENGTH((FOR e IN links RETURN 1)) > 2 ? \"many\" : \"few\"",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("many")]);
}

/// The one position the lift is NOT valid: COLLECT drops row variables, so a
/// subquery after it would not see what it reads. Refused with an explanation
/// rather than silently answering from the wrong scope.
#[test]
fn refuses_to_lift_a_subquery_past_collect() {
    let dataset = InMemoryDataset::new().with_collection(
        "rows",
        vec![
            json!({ "_id": "rows/a", "k": 1 }),
            json!({ "_id": "rows/b", "k": 1 }),
        ],
    );
    let err = cognigraph_query::parse_query(
        "FOR r IN rows COLLECT k = r.k WITH COUNT INTO n \
         RETURN { k: k, extra: LENGTH((FOR x IN rows RETURN 1)) }",
    )
    .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("COLLECT"), "message: {message}");
    assert!(
        message.contains("LET"),
        "should say what to do instead: {message}"
    );

    // The same query with the LET written before the COLLECT is accepted.
    let rows = parse_and_execute(
        "FOR r IN rows LET extra = LENGTH((FOR x IN rows RETURN 1)) \
         COLLECT k = r.k, e = extra WITH COUNT INTO n RETURN { k: k, extra: e, n: n }",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!({ "k": 1, "extra": 2, "n": 2 })]);
}

/// D6 — array helpers. `SLICE` was wanted immediately (top-N of a subquery
/// result); the set operations follow `UNIQUE`'s equality rule, so `1` and `1.0`
/// are one value.
#[test]
fn executes_array_helpers() {
    let dataset = InMemoryDataset::new().with_collection("rows", vec![json!({ "_id": "rows/a" })]);
    let one = |cgql: &str| {
        parse_and_execute(
            &format!("FOR r IN rows LIMIT 1 RETURN {cgql}"),
            &dataset,
            &HashMap::new(),
        )
        .unwrap()
    };

    assert_eq!(one("SLICE([1,2,3,4,5], 1, 2)"), vec![json!([2.0, 3.0])]);
    // Negative start counts from the end.
    assert_eq!(one("SLICE([1,2,3,4,5], -2)"), vec![json!([4.0, 5.0])]);
    // Negative length drops from the end instead.
    assert_eq!(
        one("SLICE([1,2,3,4,5], 1, -1)"),
        vec![json!([2.0, 3.0, 4.0])]
    );
    // Out of range clamps rather than erroring.
    assert_eq!(one("SLICE([1,2], 5, 3)"), vec![json!([])]);

    assert_eq!(
        one("FLATTEN([[1,2],[3,[4]]])"),
        vec![json!([1.0, 2.0, 3.0, [4.0]])]
    );
    assert_eq!(
        one("FLATTEN([[1,[2]],[3]], 2)"),
        vec![json!([1.0, 2.0, 3.0])]
    );

    assert_eq!(
        one("INTERSECTION([1,2,3],[2,3,4],[3,2])"),
        vec![json!([2.0, 3.0])]
    );
    assert_eq!(one("MINUS([1,2,3,4],[2],[4])"), vec![json!([1.0, 3.0])]);
    // Deduped, first-occurrence order preserved.
    assert_eq!(one("MINUS([1,1,2],[9])"), vec![json!([1.0, 2.0])]);
    // A non-array argument is null, never a silently empty result.
    assert_eq!(one("INTERSECTION([1], \"nope\")"), vec![json!(null)]);
}

/// D7 — date arithmetic. `DATE_ADD` is the inverse of the existing `DATE_DIFF`
/// and shares its units, so the pair round-trips.
#[test]
fn executes_date_add() {
    let dataset = InMemoryDataset::new().with_collection(
        "rows",
        vec![json!({ "_id": "rows/a", "start": "2026-01-01" })],
    );
    let one = |cgql: &str| {
        parse_and_execute(
            &format!("FOR r IN rows LIMIT 1 RETURN {cgql}"),
            &dataset,
            &HashMap::new(),
        )
        .unwrap()
    };

    // The motivating question: what is 90 days out?
    assert_eq!(
        one("DATE_YEAR(DATE_ADD(r.start, 90, \"days\"))"),
        vec![json!(2026)]
    );
    assert_eq!(
        one("DATE_MONTH(DATE_ADD(r.start, 90, \"days\"))"),
        vec![json!(4)]
    );
    assert_eq!(
        one("DATE_DAY(DATE_ADD(r.start, 90, \"days\"))"),
        vec![json!(1)]
    );
    // Round-trips with DATE_DIFF.
    assert_eq!(
        one("DATE_DIFF(r.start, DATE_ADD(r.start, 36, \"hours\"), \"hours\")"),
        vec![json!(36.0)]
    );
    // Negative amounts go backwards.
    assert_eq!(
        one("DATE_YEAR(DATE_ADD(r.start, -1, \"days\"))"),
        vec![json!(2025)]
    );
    // An unknown unit is null rather than a guess.
    assert_eq!(
        one("DATE_ADD(r.start, 1, \"fortnights\")"),
        vec![json!(null)]
    );
}

/// D8 — casts and regex. "Wrong type" is null throughout, matching the rest of
/// the registry: a silent 0 inside a SUM is a wrong answer, not a missing one.
#[test]
fn executes_casts_and_regex() {
    let dataset = InMemoryDataset::new().with_collection(
        "rows",
        vec![json!({ "_id": "rows/a", "n": "42", "t": "Contract - 2024 - 7852" })],
    );
    let one = |cgql: &str| {
        parse_and_execute(
            &format!("FOR r IN rows LIMIT 1 RETURN {cgql}"),
            &dataset,
            &HashMap::new(),
        )
        .unwrap()
    };

    assert_eq!(one("TO_NUMBER(r.n) + 1"), vec![json!(43.0)]);
    assert_eq!(one("TO_NUMBER(\"abc\")"), vec![json!(null)]);
    assert_eq!(one("TO_STRING(42)"), vec![json!("42")]);
    assert_eq!(one("TO_BOOL(0)"), vec![json!(false)]);
    assert_eq!(one("TO_BOOL(\"\")"), vec![json!(false)]);

    assert_eq!(one("REGEX_TEST(r.t, \"^Contract\")"), vec![json!(true)]);
    assert_eq!(one("REGEX_TEST(r.t, \"^contract\")"), vec![json!(false)]);
    assert_eq!(
        one("REGEX_TEST(r.t, \"^contract\", true)"),
        vec![json!(true)]
    );
    // Group references in the replacement.
    assert_eq!(
        one("REGEX_REPLACE(r.t, \"Contract - (\\\\d{4}) - (\\\\d+)\", \"$1/$2\")"),
        vec![json!("2024/7852")]
    );
    // An uncompilable pattern is null — "could not ask" is not "did not match".
    assert_eq!(one("REGEX_TEST(r.t, \"([unclosed\")"), vec![json!(null)]);
}

/// D4 (final piece) — `DOCUMENT()` resolves an id to its document. The
/// motivating case is a heterogeneous edge collection, where the target's
/// collection is not known until the row is read.
#[test]
fn executes_document_lookup() {
    let dataset = InMemoryDataset::new()
        .with_collection(
            "persons",
            vec![json!({ "_id": "persons/p1", "name": "Ada" })],
        )
        .with_collection(
            "organizations",
            vec![json!({ "_id": "organizations/o1", "name": "Acme" })],
        )
        .with_collection(
            "edges",
            vec![
                json!({ "_id": "edges/e1", "_to": "persons/p1" }),
                json!({ "_id": "edges/e2", "_to": "organizations/o1" }),
                json!({ "_id": "edges/e3", "_to": "persons/missing" }),
            ],
        );

    // One expression spans both target collections — the point of the feature.
    let rows = parse_and_execute(
        "FOR e IN edges RETURN DOCUMENT(e._to).name",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("Ada"), json!("Acme"), json!(null)]);

    // An array of ids resolves element-wise.
    let rows = parse_and_execute(
        "FOR e IN edges LIMIT 1 RETURN DOCUMENT([\"persons/p1\", \"organizations/o1\"])",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(
        rows,
        vec![json!([
            { "_id": "persons/p1", "name": "Ada" },
            { "_id": "organizations/o1", "name": "Acme" }
        ])]
    );

    // A non-identifier is null, not an error.
    let rows = parse_and_execute(
        "FOR e IN edges LIMIT 1 RETURN DOCUMENT(42)",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!(null)]);

    // Usable in a FILTER, where the lookup drives row selection.
    let rows = parse_and_execute(
        "FOR e IN edges FILTER DOCUMENT(e._to).name == \"Acme\" RETURN e._id",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("edges/e2")]);
}

// ---------------------------------------------------------------------------
// D2: a traversal that starts from the enclosing row
// ---------------------------------------------------------------------------

#[test]
fn traversal_start_can_read_the_enclosing_row() {
    let dataset = graph_dataset();
    // "For each document, what does it link to?" — the natural phrasing, which
    // before D2 could not be written at all: a traversal had to be the first
    // FOR, so its start could never name an outer variable.
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FILTER d.category == "research"
        FOR v, e IN 1..1 OUTBOUND d._id relationships
        SORT d._id ASC, v._id ASC
        RETURN { from: d.title, to: v.title, via: e.relation_type }
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        rows,
        vec![
            json!({ "from": "Alpha", "to": "Beta", "via": "links" }),
            json!({ "from": "Alpha", "to": "Gamma", "via": "weak" }),
            json!({ "from": "Beta", "to": "Gamma", "via": "links" }),
        ]
    );
}

#[test]
fn correlated_traversal_keeps_depth_semantics() {
    let dataset = graph_dataset();
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FILTER d._id == "documents/a"
        FOR v, e, p IN 1..2 OUTBOUND d._id relationships
        SORT p.depth ASC, v._id ASC
        RETURN { to: v._id, depth: p.depth }
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    // a->b and a->c at depth 1, then a->b->c at depth 2.
    assert_eq!(
        rows,
        vec![
            json!({ "to": "documents/b", "depth": 1 }),
            json!({ "to": "documents/c", "depth": 1 }),
            json!({ "to": "documents/c", "depth": 2 }),
        ]
    );
}

#[test]
fn correlated_traversal_matches_the_hand_written_join() {
    let dataset = graph_dataset();
    // The rewrite users had to write before D2, and the traversal that replaces
    // it, must agree — otherwise D2 is a new dialect, not a shorthand.
    let traversal = parse_and_execute(
        r#"
        FOR d IN documents
        FOR v, e IN 1..1 OUTBOUND d._id relationships
        SORT d._id ASC, v._id ASC
        RETURN [d._id, v._id]
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    let join = parse_and_execute(
        r#"
        FOR d IN documents
        FOR e IN relationships
        FILTER e._from == d._id
        FOR v IN documents
        FILTER v._id == e._to
        SORT d._id ASC, v._id ASC
        RETURN [d._id, v._id]
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(traversal, join);
    assert_eq!(traversal.len(), 3);
}

#[test]
fn correlated_traversal_works_inside_a_subquery() {
    let dataset = graph_dataset();
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        LET outgoing = (FOR v IN 1..1 OUTBOUND d._id relationships RETURN v._id)
        SORT d._id ASC
        RETURN { id: d._id, n: LENGTH(outgoing) }
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        rows,
        vec![
            json!({ "id": "documents/a", "n": 2 }),
            json!({ "id": "documents/b", "n": 1 }),
            json!({ "id": "documents/c", "n": 0 }),
        ]
    );
}

#[test]
fn uncorrelated_traversal_in_a_later_for_keeps_the_outer_row() {
    let dataset = graph_dataset();
    // The start is a literal, so the traversal is still materialized once — but
    // it is no longer the first FOR, and the outer binding must survive the
    // join rather than being replaced by the traversal's own environments.
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FILTER d._id == "documents/c"
        FOR v IN 1..1 OUTBOUND "documents/a" relationships
        SORT v._id ASC
        RETURN { outer: d.title, reached: v._id }
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        rows,
        vec![
            json!({ "outer": "Gamma", "reached": "documents/b" }),
            json!({ "outer": "Gamma", "reached": "documents/c" }),
        ]
    );
}

#[test]
fn correlated_traversal_from_a_missing_vertex_yields_nothing() {
    let dataset = graph_dataset();
    // A row whose start is absent or not a string contributes no rows instead
    // of failing the query — one bad row should not take the result with it.
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FOR v IN 1..1 OUTBOUND d.no_such_field relationships
        RETURN v._id
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert!(rows.is_empty());
}

#[test]
fn explain_marks_a_traversal_correlated_and_hides_invented_vars() {
    let dataset = graph_dataset();
    let correlated = parse_and_execute(
        "EXPLAIN FOR d IN documents FOR v IN 1..1 OUTBOUND d._id relationships RETURN v._id",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    let source = &correlated[0]["sources"][1];
    assert_eq!(source["kind"], json!("traversal"));
    assert_eq!(source["correlated"], json!(true));
    // Only the variable the query named — `$edge` and `$path` are the parser's.
    assert_eq!(source["vars"], json!(["v"]));

    let literal = parse_and_execute(
        r#"EXPLAIN FOR v, e, p IN 1..1 OUTBOUND "documents/a" relationships RETURN v._id"#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    let source = &literal[0]["sources"][0];
    assert_eq!(
        source["correlated"],
        json!(false),
        "a literal start is still materialized once, before the run"
    );
    assert_eq!(source["vars"], json!(["v", "e", "p"]));
}

#[test]
fn two_traversals_in_one_query_do_not_collide() {
    let dataset = graph_dataset();
    // Neither traversal names an edge or path variable, so the parser invents
    // both. Inventing the same name twice made this a duplicate-variable error
    // — found by a real workload query, not a fixture.
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FILTER d._id == "documents/a"
        FOR x IN 1..1 OUTBOUND d._id relationships
        FOR y IN 1..1 OUTBOUND d._id relationships
        SORT x._id ASC, y._id ASC
        RETURN [x._id, y._id]
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    // a reaches b and c, so the self-join is 2 x 2.
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0], json!(["documents/b", "documents/b"]));
}

// ---------------------------------------------------------------------------
// Move-calculations-down: RETURN-only LETs run after SORT/LIMIT
// ---------------------------------------------------------------------------

#[test]
fn return_only_lets_defer_past_limit_with_identical_results() {
    let dataset = graph_dataset();
    // `outgoing` feeds FILTER and SORT so it must stay; `titles` decorates
    // the projection only, so the planner defers it. The observable contract
    // is that the answer is exactly what the undeferred semantics give.
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        LET outgoing = (FOR v IN 1..1 OUTBOUND d._id relationships RETURN v._id)
        FILTER LENGTH(outgoing) > 0
        LET titles = (FOR v IN 1..1 OUTBOUND d._id relationships SORT v.title ASC RETURN v.title)
        SORT LENGTH(outgoing) DESC, d._id ASC
        LIMIT 1
        RETURN { id: d._id, n: LENGTH(outgoing), titles: titles }
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();

    assert_eq!(
        rows,
        vec![json!({ "id": "documents/a", "n": 2, "titles": ["Beta", "Gamma"] })]
    );
}

#[test]
fn deferred_let_may_reference_another_deferred_let() {
    let dataset = documents_dataset();
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        LET a = CONCAT(d.title, "!")
        LET b = CONCAT(a, "?")
        SORT d.title ASC
        LIMIT 2
        RETURN b
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("Alpha!?"), json!("Beta!?")]);
}

#[test]
fn let_read_by_a_filter_is_not_deferred() {
    let dataset = documents_dataset();
    // `flag` is read by the FILTER, so deferring it would break the filter.
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        LET flag = d.active == true
        FILTER flag
        SORT d.title ASC
        LIMIT 5
        RETURN d.title
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!("Alpha"), json!("Gamma")]);
}

#[test]
fn explain_marks_deferred_lets() {
    let dataset = documents_dataset();
    let report = parse_and_execute(
        r#"
        EXPLAIN FOR d IN documents
        LET extra = UPPER(d.title)
        SORT d.title ASC
        LIMIT 2
        RETURN [d.title, extra]
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    let pipeline: Vec<String> = report[0]["pipeline"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        pipeline
            .iter()
            .any(|label| label == "LET extra [deferred past LIMIT]"),
        "pipeline was {pipeline:?}"
    );
}

// ---------------------------------------------------------------------------
// D9: SORTED / SORTED_UNIQUE
// ---------------------------------------------------------------------------

#[test]
fn sorted_agrees_with_the_sort_clause() {
    let dataset = documents_dataset();
    // The contract that makes SORTED trustworthy: wherever the SORT clause
    // defines an order, the function produces the same one.
    let via_clause = parse_and_execute(
        "FOR d IN documents SORT d.title ASC RETURN d.title",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    let via_function = parse_and_execute(
        r#"
        FOR d IN documents
        COLLECT x = true INTO g = d.title
        RETURN SORTED(g)
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(via_function, vec![Value::Array(via_clause)]);
}

#[test]
fn sorted_pins_mixed_types_and_sorted_unique_dedups() {
    let dataset = documents_dataset();
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FILTER d._id == "documents/a"
        RETURN {
            mixed: SORTED([ "b", 2, null, [1], true, {a: 1}, 1.5, "a" ]),
            deduped: SORTED_UNIQUE([ 3, 1, 2.0, "b", 1.0, 2, "a", "b" ]),
            not_an_array: SORTED(42)
        }
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(
        rows,
        vec![json!({
            // null < bool < number < string < array < object. Numeric
            // literals parse as f64, so the expectation spells them that way.
            "mixed": [null, true, 1.5, 2.0, "a", "b", [1.0], {"a": 1.0}],
            // 1 == 1.0 and 2.0 == 2 under UNIQUE's equality; first survives
            "deduped": [1.0, 2.0, 3.0, "a", "b"],
            "not_an_array": null
        })]
    );
}

#[test]
fn slice_of_sorted_unique_is_deterministic() {
    let dataset = graph_dataset();
    // The D9 motivating case: "first 3 of a UNIQUE set" had no single right
    // answer. Sorting first makes it one.
    let rows = parse_and_execute(
        r#"
        FOR d IN documents
        FILTER d._id == "documents/a"
        LET reached = (FOR v IN 1..2 OUTBOUND d._id relationships RETURN v.title)
        RETURN SLICE(SORTED_UNIQUE(reached), 0, 2)
        "#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert_eq!(rows, vec![json!(["Beta", "Gamma"])]);
}
