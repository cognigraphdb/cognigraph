# CG-37 nested subquery binding verification — 2026-09-09

Nested lifted subqueries now receive unique generated bindings across the
complete parsed query. The saved release reproduced 40 name collisions across
five query shapes; the rebuilt release returned the expected results for all
of them. [CG-37](CG-37.md) is resolved. [CG-38](CG-38.md), the read-only route's
error-status defect, remained open at this checkpoint; its subsequent fix is
recorded at the end.

The preceding CG-23/CG-28 batch was committed locally as `b9084e6`. This fix was
committed locally as `c48a3b4`; nothing was pushed. No user research/draft
files, `ui/`, secrets, or local environment files were edited.

## Cause and change

The [parser](../../crates/cognigraph-query/src/parser/mod.rs) previously reset
its synthetic-name counter in every nested `Query`. Nested validation inherits
the enclosing scope, so an earlier outer `$sq0` could collide with a child's
`$sq0`. The `$` prefix excludes user identifiers but cannot prevent collisions
between generated bindings.

`parse_query` now creates one counter and passes it through every recursive
desugaring call, including explicit LET subquery bodies and lifted expressions.
The allocator resets on each parse, uses no global state, and preserves
deterministic output. Internal `$sqN` names may be renumbered in EXPLAIN output.
No grammar, validation, execution, backend, dependency, or storage-format change
was needed. [The decision record](../decisions/decision_cgql_v2.md#generated-subquery-names-cg-37-2026-09-09)
and [specification](../reference/cgql.md#subqueries-in-expression-position) record the fix.

## Regression coverage and Rust gates

The original [collision fixture](../../crates/cognigraph-query/tests/corpus/exec/sq_nested_binding_collision.cgql)
moved from `validate_err` into `exec`, with expected result
`[{"n":4,"nested":[4]}]`. Before changing production source, the corpus runner
failed on its duplicate generated binding: three runner functions passed and
one failed. Four additional fixtures cover explicit LET correlation, bind
variables, sibling lifted expressions, and nested DOCUMENT lookups. All five
fail with the saved release and execute correctly after the fix.

The [parser/validation regressions](../../crates/cognigraph-query/tests/subquery_bindings.rs)
check collected binds, repeated parses separated by an unrelated query,
continued rejection of user-variable shadowing, and depths one through five
(four remains the maximum). The
[dynamic execution regression](../../crates/cognigraph-query/tests/backend_executor/dynamic_analysis.rs)
checks compiled result values, two document fetches in analysis, and exact
source-row budget boundaries in normal and analyzed execution. Existing CG-7
and CG-28 mutation and position restrictions remain in force.

Final verification:

- `cargo test -p cognigraph-query`: **147 passed**, zero failed/ignored.
- `cargo test -p cognigraph-native --test cgql_corpus`: **1 passed**, including
  all shared execution fixtures through Native query execution.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: **953 reported passes** in 72 result groups, zero
  failed/ignored. Eight Arango integration entries early-returned because
  credentials were absent; this run does not provide new live Arango coverage.
- `cargo build --release -p cognigraph-server -p cognigraph-cli`: passed.

The three new Rust test functions account for the increase from 950 to 953.
Corpus examples are additional assertions inside existing test functions.
The full formatting, Clippy, and workspace-test gates ran in that order.
[Validation evidence](../evidence/engineering-historical-checks.md#artifact-8c2e063c848a6d0fcf62)
records log, binary, fixture, harness, and document hashes.

## Release HTTP and Lua verification

The [release harness](../verification/harnesses/subquery-contract-http.py) starts authenticated,
temporary resident/sidecar and paged/sidecar Native stores, seeds the shared
corpus through real HTTP routes, and stops both servers after each run. It uses
explicit configuration outside the checkout and no embedding/completion
provider. Fixture bind headers feed both HTTP bindings and Lua `graph.query()`.
Queries with generated relationship keys compare stable endpoints instead.

The saved pre-fix release server has SHA-256
`a7cb6d882c1f5b438d1b98e95747865c0d63182697e62025ccb26a02e94400cb`;
its production source predates this fix and was unchanged by CG-23/CG-28.
The corrected release server has SHA-256
`434c926c5ac4a0c455d25d4192197c126c997e3789af03ed3660ca1fcd429909`.
The evidence's Git revision identifies HEAD during the run; these hashes
distinguish the old binary from the binary built with uncommitted CG-37 changes.

Each run records **180 observations**:

| Observation | Saved release | Corrected release |
|---|---:|---:|
| Exact read results across `/api/query`, `/api/search/query`, and Lua | 54 | 84 |
| Analyzed result-count checks through `/api/query` | 18 | 28 |
| Generated binding collisions on normal HTTP/Lua queries | 30 | 0 |
| Generated binding collisions on analyzed queries | 10 | 0 |
| Expected parse/validation rejections through HTTP/Lua | 24 | 24 |
| Exact source/destination preservation after rejection | 24 | 24 |
| Successful supported body-subquery mutations | 8 | 8 |
| Subsequent reads of those stored mutation values | 8 | 8 |
| Known CG-38 status mismatches (500, desired 400) | 4 | 4 |

[Baseline evidence](../evidence/engineering-historical-checks.md#artifact-60ebb4e7a1ce44017759)
uses `--expect-collisions` to assert the former failure. The
[corrected evidence](../evidence/engineering-historical-checks.md#artifact-3a263aaba10202f26d74)
uses the default expectations and verifies all 14 executable `sq_*` examples.
The four CG-38 observations intentionally preserve a separate defect; they are
not desired-behavior passes. Lua rejection checks use `pcall`, so their HTTP
envelope succeeds while the returned value identifies the query error.

This verifies execution and analyzed counts, with precise analysis/budget
boundaries covered by Rust tests. It does not measure performance, test restart
recovery, re-run the partner CRM workload, or change mutation atomicity.

## Corrected attempts and reproduction

The initial dynamic budget test cast the observed `u64` row count to `usize`,
which did not match `ExecutionBudget`; compilation failed and the test was
corrected to retain `u64`. The first extended live run prepended
`EXPLAIN ANALYZE` on the same line as a fixture's bind header, preventing the
harness from collecting those bindings. It returned a missing-bind error.
The harness now puts the prefix on its own line, preserving the header. Both
saved-release and corrected-release matrices were then re-run successfully;
neither correction altered product semantics.

Re-run current verification with:

```bash
cargo test -p cognigraph-query
cargo test -p cognigraph-native --test cgql_corpus
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server -p cognigraph-cli
python3 docs/issues/evidence/subquery-contract-http.py --output /tmp/cg37-fixed-http.json
```

To reproduce the old failure while the saved binary is still available:

```bash
python3 docs/issues/evidence/subquery-contract-http.py \
  --binary /tmp/cognigraph-pre-cg37-server --expect-collisions \
  --output /tmp/cg37-baseline-http.json
```

Local links, registry statuses/counts, fixture hashes, and the 24 pre-existing
user-owned paths were checked. The registry then had **30 Resolved and 8 Open**
issues. The next bounded fix was CG-38. Model benchmarking remains deferred until the
original ticket list closes, with Luna as the economical baseline.

## Subsequent CG-38 verification

[CG-38](query-error-status-2026-09-09.md) now returns 400 for plan failures on
both public query routes. The current shared harness runs 288 observations and
requires those statuses by default, with explicit options for old binaries.
The 180-observation artifacts above retain their original hashes and record
the CG-37 checkpoint, including the then-open CG-38 status mismatches.
