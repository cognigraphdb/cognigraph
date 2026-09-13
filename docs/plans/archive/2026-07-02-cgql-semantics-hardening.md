# CGQL Semantics Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the semantic gaps in CGQL v1 (null handling, depth-0 traversal, structured errors, bind-var strictness, grammar edge cases) so the language contract is trustworthy before pushdown, persistence, and the native-first migration build on it.

**Architecture:** All changes stay inside `cognigraph-query` plus one small traversal change in `cognigraph-native` (depth-0). No trait changes, no server changes, no new crates. Each task is test-first against the in-memory executor, with backend-executor coverage where behavior crosses the `GraphBackend` boundary.

**Tech Stack:** Rust 2024, pest grammar, thiserror, tokio (tests only).

**Context for the implementer:**
- CGQL pipeline: `grammar.pest` → `parser.rs` (AST) → `validation.rs` → `planner.rs` (LogicalPlan) → `executor.rs` (in-memory + GraphBackend executors).
- Spec lives in `docs/cgql-v1.md` and MUST be updated when behavior changes (Task 8).
- Repo validation standard after all tasks: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all`.
- NOTE: `crates/cognigraph-query/` and `crates/cognigraph-native/` are currently untracked in git. Do NOT create per-task commits; run the full validation suite after each task instead. A single baseline commit is proposed to the user at the end.

---

### Task 1: Missing document paths evaluate to null (not an error)

**Files:**
- Modify: `crates/cognigraph-query/src/executor.rs` (`eval_identifier`, filter handling in `apply_plan_operations`)
- Test: `crates/cognigraph-query/tests/executor.rs`

- [x] **Step 1: Write failing tests**

```rust
#[test]
fn missing_path_returns_null_in_projection() {
    let dataset = InMemoryDataset::new().with_collection(
        "documents",
        vec![json!({ "_id": "documents/a", "title": "Alpha" })],
    );
    let rows = parse_and_execute(
        "FOR d IN documents RETURN d.category",
        &dataset,
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
    let dataset = InMemoryDataset::new().with_collection(
        "documents",
        vec![json!({ "_id": "documents/a", "title": "Alpha" })],
    );
    let rows = parse_and_execute(
        "FOR d IN documents FILTER d.missing RETURN d",
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert!(rows.is_empty());
}
```

- [x] **Step 2: Run to verify failure** — `cargo test -p cognigraph-query --test executor` → the three tests FAIL with `IdentifierNotFound` / `ExpectedBool`.

- [x] **Step 3: Implement** — in `eval_identifier`, missing path *segments* become `Value::Null` (unknown *roots* remain errors; validation guarantees scope):

```rust
fn eval_identifier(parts: &[String], env: &Env) -> Result<Value, ExecutionError> {
    let Some(root) = parts.first() else {
        return Ok(Value::Null);
    };
    let mut value = env
        .get(root)
        .cloned()
        .ok_or_else(|| ExecutionError::IdentifierNotFound(root.clone()))?;
    for part in &parts[1..] {
        value = value.get(part).cloned().unwrap_or(Value::Null);
    }
    Ok(value)
}
```

In `apply_plan_operations`, treat `Null` filter results as false:

```rust
Ok(Value::Bool(true)) => Some(Ok(env)),
Ok(Value::Bool(false)) | Ok(Value::Null) => None,
```

- [x] **Step 4: Run tests** — `cargo test -p cognigraph-query` → PASS, no regressions.

### Task 2: Depth-0 traversal emits the start vertex

**Files:**
- Modify: `crates/cognigraph-query/src/executor.rs` (`traversal_rows`, drop `&& depth > 0`)
- Modify: `crates/cognigraph-native/src/memory.rs` (`traverse`, drop `&& depth > 0`)
- Test: `crates/cognigraph-query/tests/executor.rs`, `crates/cognigraph-native/tests/native_backend.rs`

- [x] **Step 1: Write failing tests**

In-memory executor test (edge var must be `null`, path has zero edges):

```rust
#[test]
fn depth_zero_traversal_emits_start_vertex() {
    let dataset = traversal_dataset(); // existing helper in this test file
    let rows = parse_and_execute(
        r#"FOR v, e, p IN 0..1 OUTBOUND "documents/a" relationships RETURN { id: v._id, edge: e, depth: p.depth }"#,
        &dataset,
        &HashMap::new(),
    )
    .unwrap();
    assert!(rows.contains(&json!({ "id": "documents/a", "edge": null, "depth": 0 })));
}
```

Native backend test:

```rust
#[tokio::test]
async fn traversal_min_depth_zero_includes_start() {
    let backend = seeded_backend().await;
    let paths = backend
        .traverse(
            "documents/a",
            &TraversalOpts {
                max_depth: 1,
                min_depth: 0,
                direction: Direction::Outbound,
                edge_collection: "relationships".into(),
                min_confidence: None,
                path_decay: 0.8,
            },
        )
        .await
        .unwrap();
    let depth0 = paths.iter().find(|p| p.depth == 0).expect("depth-0 path");
    assert!(depth0.edges.is_empty());
    assert!((depth0.score - 1.0).abs() < f64::EPSILON);
}
```

- [x] **Step 2: Verify failure** — depth-0 rows are absent because both traversals guard with `depth > 0`.
- [x] **Step 3: Implement** — remove `&& depth > 0` from the emit condition in `executor.rs::traversal_rows` and `memory.rs::traverse`. `path_decay.powi(0)` already yields score 1.0; `edges.last().unwrap_or(Null)` already yields a null edge var.
- [x] **Step 4: Run** — `cargo test -p cognigraph-query -p cognigraph-native` → PASS. (Defaults keep `min_depth = 1`, so no behavior change for existing callers.)

### Task 3: Structured parse errors with line/column

**Files:**
- Modify: `crates/cognigraph-query/src/parser.rs` (`ParseError`, `parse_query`)
- Test: `crates/cognigraph-query/tests/parser.rs`

- [x] **Step 1: Failing test**

```rust
#[test]
fn parse_error_reports_line_and_column() {
    let err = parse_query("FOR d IN documents\nRETURN {").unwrap_err();
    assert_eq!(err.line_col, Some((2, 9)));
    assert!(err.to_string().starts_with("parse error at line 2, column 9:"));
}
```

- [x] **Step 2: Verify failure** — no `line_col` field exists; does not compile → counts as failing.
- [x] **Step 3: Implement**

```rust
/// CGQL parse error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    /// 1-based line and column of the failure, when known.
    pub line_col: Option<(usize, usize)>,
}

impl std::fmt::Display for ParseError { /* with/without position */ }

impl ParseError {
    fn new(message: impl Into<String>) -> Self { /* line_col: None */ }
    fn from_pest(error: pest::error::Error<Rule>) -> Self { /* extract LineColLocation */ }
}
```

`parse_query` uses `ParseError::from_pest(e)`; internal AST-shape errors keep `ParseError::new` (no position).

- [x] **Step 4: Run** — `cargo test -p cognigraph-query` → PASS (thiserror derive on Display removed in favor of manual impl).

### Task 4: Keyword boundaries in grammar + reserved variable names

**Files:**
- Modify: `crates/cognigraph-query/src/grammar.pest` (all `kw_*` rules)
- Modify: `crates/cognigraph-query/src/validation.rs` (reserved variable names)
- Test: `crates/cognigraph-query/tests/parser.rs`, `crates/cognigraph-query/tests/validation.rs`

- [x] **Step 1: Failing tests**

```rust
// parser.rs
#[test]
fn rejects_keyword_glued_to_identifier() {
    assert!(parse_query("FORd IN documents RETURN d").is_err());
    assert!(parse_query("FOR d IN documents FILTER d.x INitems RETURN d").is_err());
}

// validation.rs
#[test]
fn rejects_reserved_variable_name() {
    let query = parse_query("FOR filter IN documents RETURN filter").unwrap();
    assert!(matches!(
        validate_query(&query),
        Err(ValidationError::ReservedVariableName(name)) if name == "filter"
    ));
}
```

- [x] **Step 2: Verify failure** — glued keywords currently parse; `ReservedVariableName` doesn't exist.
- [x] **Step 3: Implement** — every keyword becomes atomic with a boundary guard, e.g.:

```pest
kw_for = @{ ^"FOR" ~ !(ASCII_ALPHANUMERIC | "_") }
```

(applied to all `kw_*` rules). In validation, `insert_var` rejects reserved words:

```rust
#[error("variable name `{0}` is reserved")]
ReservedVariableName(String),

fn insert_var(scope: &mut BTreeSet<String>, var: &str) -> Result<(), ValidationError> {
    if is_reserved_word(var) {
        return Err(ValidationError::ReservedVariableName(var.to_string()));
    }
    ...
}
```

Path *segments* (`d.filter`) stay legal — only declared variables are restricted.

- [x] **Step 4: Run** — full `cargo test -p cognigraph-query`; watch for existing parser tests that relied on prefix matching (none expected).

### Task 5: Strict bind-variable checking at execution boundary

**Files:**
- Modify: `crates/cognigraph-query/src/executor.rs` (`execute_plan`, `execute_backend_plan`, new `check_bind_vars`, two `ExecutionError` variants)
- Test: `crates/cognigraph-query/tests/executor.rs`

- [x] **Step 1: Failing tests**

```rust
#[test]
fn missing_bind_variable_fails_before_execution() {
    let dataset = InMemoryDataset::new().with_collection("documents", vec![]);
    let err = parse_and_execute(
        "FOR d IN documents FILTER d.category == @category RETURN d",
        &dataset,
        &HashMap::new(),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "missing bind variables: category");
}

#[test]
fn unexpected_bind_variable_fails() {
    let dataset = InMemoryDataset::new().with_collection("documents", vec![]);
    let err = parse_and_execute(
        "FOR d IN documents RETURN d",
        &dataset,
        &HashMap::from([("extra".to_string(), json!(1))]),
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "unexpected bind variables: extra");
}
```

Note: the first test uses an *empty* collection on purpose — it proves the check is eager, not lazy.

- [x] **Step 2: Verify failure** — currently the empty-collection query succeeds and extras are ignored.
- [x] **Step 3: Implement** — `check_bind_vars(&plan.bind_vars, bind_vars)` called at the top of both `execute_plan` and `execute_backend_plan`; missing and unexpected lists are sorted for deterministic messages.
- [x] **Step 4: Run** — `cargo test -p cognigraph-query`; also `cargo test -p cognigraph-native` (its `query()` path now enforces this).

### Task 6: Comparisons are non-associative

**Files:**
- Modify: `crates/cognigraph-query/src/grammar.pest` (`comparison_expr`)
- Test: `crates/cognigraph-query/tests/parser.rs`

- [x] **Step 1: Failing test**

```rust
#[test]
fn rejects_chained_comparisons() {
    assert!(parse_query("FOR d IN documents FILTER d.a == d.b == d.c RETURN d").is_err());
}
```

- [x] **Step 2: Verify failure** — currently parses as `(a == b) == c`.
- [x] **Step 3: Implement** — `comparison_expr = { add_expr ~ (comparison_op ~ add_expr)? }` (one optional comparison instead of a fold).
- [x] **Step 4: Run** — `cargo test -p cognigraph-query` (snapshot tests unaffected: single comparisons produce identical ASTs).

### Task 7: String escapes match JSON

**Files:**
- Modify: `crates/cognigraph-query/src/grammar.pest` (`string` rule)
- Test: `crates/cognigraph-query/tests/parser.rs`

- [x] **Step 1: Failing test**

```rust
#[test]
fn parses_all_json_escapes() {
    let query = parse_query(r#"FOR d IN documents RETURN "a\/b\bc\fd""#).unwrap();
    assert_eq!(query.return_expr, Expr::String("a/b\u{8}c\u{c}d".to_string()));
}
```

- [x] **Step 2: Verify failure** — grammar rejects `\/`, `\b`, `\f` although `parse_string` (serde_json) accepts them.
- [x] **Step 3: Implement** — extend the escape alternatives: `"\\\"" | "\\\\" | "\\/" | "\\b" | "\\f" | "\\n" | "\\r" | "\\t" | "\\u" ~ ASCII_HEX_DIGIT{4}`.
- [x] **Step 4: Run** — `cargo test -p cognigraph-query` → PASS.

### Task 8: Documentation

**Files:**
- Modify: `docs/cgql-v1.md` (null semantics, depth 0, error positions, bind-var strictness, non-associative comparisons, escape set)
- Modify: `docs/implementation-plan.md` (Phase 5 checkboxes, strategic direction section)

- [x] **Step 1:** Update `docs/cgql-v1.md`: add a "Null Semantics" section; state that traversal ranges may start at 0 and depth 0 binds a null edge; document parse-error positions; document strict bind-variable matching; note comparisons are non-associative; list the full JSON escape set.
- [x] **Step 2:** Update `docs/implementation-plan.md` per the roadmap (Milestones M1–M4, ArangoDB demoted to maintenance, SurrealDB dropped).
- [x] **Step 3:** Final validation: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all` → all green.

---

## Follow-on milestones (planned separately, one plan each)

- **M2 — Native-first server:** `query_language()`-aware health check and `/search/query`; hybrid search fails loudly or falls back explicitly instead of `unwrap_or_default()`; shared backend-contract test suite (create-conflict semantics, implicit-collection-creation, collection-type enforcement); adjacency index in `cognigraph-native`.
- **M3 — Native persistence:** storage-model document first (redb tables: collections, documents, edges, adjacency, metadata), then implementation + durability/import/export tests, `COGNIGRAPH_NATIVE_PATH` env.
- **M4 — CGQL v2 + pushdown:** function registry, LIMIT/FILTER pushdown capability on the trait, `Option<f64>` threshold in `VectorSearchOpts` (kills the `-1.0` sentinel), BM25 via tantivy in the native backend, flip default backend to native.
