# CG-38 query error status verification — 2026-09-09

The read-only `POST /api/search/query` route now returns HTTP 400 for parser
and semantic-validation failures, matching `POST /api/query`. Normal queries,
EXPLAIN, and EXPLAIN ANALYZE preserve identical diagnostics with a
`Validation error:` prefix. The saved release reproduced 24 status mismatches;
the corrected release returned the desired status in all 288 HTTP/Lua checks.

CG-37 was committed locally as `c48a3b4`, preserving the user's unrelated
research/draft work and existing `CLAUDE.md` deletion. CG-38 was committed
locally as `aae43ad`; nothing was pushed.

## Cause and fix

The [read-only mapper](../../crates/cognigraph-server/src/routes/search/basic.rs)
had branches for forbidden and connection errors, followed by a query-error
fallback. `ExecutionError::Plan` reached that fallback and produced 500.
The [read-write mapper](../../crates/cognigraph-server/src/routes/query.rs)
already classified plan failures as `CogniGraphError::ValidationError` (400).

Adding the same typed plan branch to the read-only mapper corrects its status
and error prefix. The parser/validator's diagnostic is preserved. The fix does
not infer error types from message text or change query execution, language
rules, authorization, mutation gating, result invalidation, or backend routing.
Forbidden errors remain 403, connection errors 503, and other backend/execution
errors retain their existing mapping. The [specification](../reference/cgql.md#subqueries-in-expression-position)
and [decision record](../decisions/decision_cgql_v2.md#public-cgql-plan-errors-cg-38-2026-09-09)
describe the corrected public contract.

## Rust verification

The new [production-route regression](../../crates/cognigraph-server/src/routes/document_lookup_tests.rs)
failed before the source fix: `RETURN` on `/api/search/query` returned 500 where
400 was required. Four invalid query shapes, three execution/explanation forms,
and both endpoints now assert 400 and identical JSON error bodies. The test
backend panics on collection access or writes, proving rejected plans do not
reach storage. A second test preserves 500 for an actual execution failure.
Existing tests in the same module exercise typed forbidden, connection, and
storage errors through both query routes and Lua, including dynamic DOCUMENT
reads and analyzed execution.

- Focused server route module: **5 passed**, zero failed/ignored.
- `cargo test -p cognigraph-query`: **147 passed**, zero failed/ignored.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: **955 reported passes** in 72 result groups, zero
  failed/ignored. Eight unconfigured Arango integration entries early-returned;
  this is not new live Arango coverage.
- `cargo build --release -p cognigraph-server -p cognigraph-cli`: passed.

The two new Rust test functions cover plan rejection and execution-error
preservation. Full formatting, Clippy, and workspace-test gates run in that
order. [Validation evidence](evidence/query-error-status-validation-2026-09-09.json)
records logs, hashes, registry/link checks, and preservation of unrelated work.

## Release verification

The [shared harness](evidence/subquery-contract-http.py) starts authenticated,
disposable resident/sidecar and paged/sidecar Native servers outside the
checkout, with explicit configuration and no embedding/completion provider.
It seeds only the public corpus and synthetic mutation destination, exercises
the real binary over HTTP, and stops both processes after each run.

The saved pre-fix server has SHA-256
`434c926c5ac4a0c455d25d4192197c126c997e3789af03ed3660ca1fcd429909`.
The corrected release server has SHA-256
`9bf65bcdae28e09dca921dae0025a2f0fe78984f0219eff4634731223472a903`.
Both artifacts record the same HEAD revision; the binary hashes distinguish
the saved executable from the release built with uncommitted CG-38 changes.

Each run records **288 observations**:

| Observation | Count across both storage modes |
|---|---:|
| Exact results for 14 subquery examples via both HTTP routes and Lua | 84 |
| Analyzed result counts | 28 |
| Expected CG-7/CG-28 parse/validation rejections via HTTP/Lua | 24 |
| Exact before/after source and destination preservation for those rejections | 24 |
| Supported body-subquery mutation results | 8 |
| Subsequent reads of their stored values | 8 |
| Four invalid query shapes × three forms × two HTTP endpoints | 48 |
| Exact source and destination preservation after those invalid queries | 48 |
| Forbidden 403 and execution 500 controls on both HTTP routes | 16 |

The [baseline](evidence/query-error-status-baseline-http-2026-09-09.json)
explicitly expects CG-38's old read-only 500 status: **24 of 48 plan-error
responses differ from the desired 400**. It confirms the other endpoint already
returns 400. The [corrected run](evidence/query-error-status-fixed-http-2026-09-09.json)
requires 400 for all 48 plan errors and equal JSON bodies between endpoints,
with **zero status mismatches**. The old known-status assertions have been
replaced by this default desired-behavior matrix. `--expect-plan-errors-500`
remains available only for saved-release reproduction; `--expect-collisions`
also enables it for the older CG-37 baseline.

The matrix covers ordinary syntax errors, unresolved identifiers, post-COLLECT
lifting rejection, and unsupported inline INTO subqueries. Prefix variants
cover ordinary execution, EXPLAIN, and EXPLAIN ANALYZE. Lua's existing syntax
and mutation controls remain in the shared matrix; this fix changes only the
read-only HTTP mapper. The live Native run does not inject connection or disk
failure: typed 503 and backend 500 preservation are verified through production
routers with a deterministic fault backend. No restart, Arango deployment,
performance, partner workload, or model benchmark is claimed.

## Reproduction

```bash
cargo test -p cognigraph-server routes::document_lookup_tests
cargo test -p cognigraph-query
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server -p cognigraph-cli
python3 docs/issues/evidence/subquery-contract-http.py --output /tmp/cg38-fixed-http.json
```

While the saved pre-fix binary remains available:

```bash
python3 docs/issues/evidence/subquery-contract-http.py \
  --binary /tmp/cognigraph-pre-cg38-server --expect-plan-errors-500 \
  --output /tmp/cg38-baseline-http.json
```

The baseline route-test failure was intentional and reproduced the defect.
The final focused tests and both live matrices completed without failed
verification attempts. Their baseline options explicitly assert historical
failures; they do not count those failures as desired-behavior passes.

Local links, fixture references/hashes, registry totals/statuses, and all 24
pre-existing user-owned paths were checked. CG-38 is resolved; the registry now
has **31 Resolved and 7 Open** issues. Next bounded fix: CG-35 startup derivative
reuse. Model benchmarking remains deferred with Luna as the economical baseline.
