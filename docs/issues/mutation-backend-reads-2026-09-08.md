# CG-7 mutation backend-read validation — 2026-09-08

CG-7 is resolved locally using its accepted validation remedy: reject
unsupported dynamic backend reads before a mutation starts. Full
document/correlated traversal resolution
inside mutations is not enabled by this change. The work remains uncommitted
with CG-29, CG-32, and the model comparison; nothing was pushed. Unrelated
research/draft changes and the `CLAUDE.md` deletion were preserved.

## Change and compatibility

The semantic validator walks the whole mutation query, including nested read
subqueries, and rejects `DOCUMENT()` calls and traversal starts that depend on
row variables. It covers selectors, INSERT/UPDATE/REPLACE payloads, every UPSERT
branch, source expressions, filters, LETs, and `OLD`/`NEW` return projections.
Checks are static, including untaken branches and EXPLAIN, and happen before
backend materialization or any write. Function names remain case-insensitive;
string contents, field names, and bind variable names are not mistaken for
function calls.

The read-write HTTP endpoint maps CGQL parsing/planning validation errors to
HTTP 400. Other runtime error mappings and protected-collection HTTP 403
responses retain their existing behavior. Lua `graph.query()` surfaces the
validation error before performing the mutation.

The deterministic validation errors are:

- `DOCUMENT() is not supported in mutation queries`
- `correlated traversal starts are not supported in mutation queries`

Ordinary mutations, direct `OLD`/`NEW` projections, read-only `DOCUMENT()`,
ordinary nested read subqueries, and traversals starting from literals or bind
variables retain their existing behavior. Applications can fetch a document
separately and pass the required value as a mutation bind variable; separate
queries do not provide a cross-document transactional snapshot. Existing bulk
mutations still have per-document atomicity.

This corrects silent data loss by rejecting queries that the executor could
not correctly evaluate. Supporting these reads in mutation execution later
requires deliberate read visibility and resolution semantics without replaying
writes. The shared resolver's open budget/error issues remain CG-15 and CG-16;
the separate analyzed-query issue remains CG-8. No grammar or backend API
change is included here.

## Verification

The focused query crate and Native mutation regression passed. The new matrix
contains 32 query shapes; the Native test runs every shape against existing and
absent document IDs and compares the complete stored documents after each
rejection. Validation and planning share the matrix. Positive cases cover
ordinary mutations and reads; EXPLAIN cannot bypass the restriction.

The final formatting, Clippy (`--all-targets -- -D warnings`), and full-suite
gates passed: **893 tests**, zero failed or ignored, across 68 result summaries.
The focused query crate passed 133 tests, the Native regression covered 64
rejections, and the three query route tests passed. OpenAPI YAML parsed and
includes the query endpoint's HTTP 400 response. The release server build and
runtime checks below passed after the final Rust changes.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server --bin cognigraph-server
python3 docs/issues/evidence/mutation-backend-reads-http.py
```

## Release HTTP evidence

The saved pre-fix release returned HTTP 200 with `[null]` for
`UPDATE "a" WITH {copied:DOCUMENT("notes/b").v} IN notes RETURN NEW.copied`,
committed `copied: null`, and preserved it across restart. A separate read of
the same reference returned `2`. Both resident and paged storage reproduced it.

The first fixed-release HTTP check exposed an existing error-mapping problem:
the executor correctly rejected the query, but `/api/query` wrapped the
validation error as a generic query failure and returned HTTP 500. That live
check failed its expected-400 assertion. The endpoint now retains the planning
error category and has a focused route regression for the status and unchanged
documents. The final gates and live matrix passed after that correction.

The corrected release passed **256 rejection cases**: 32 query shapes × two
reference states (existing/absent) × two surfaces (HTTP query/Lua
`graph.query()`) × two storage modes (resident/paged). Each case compared the
complete stored documents, including timestamps, with the original snapshot.
HTTP returned 400 with the expected diagnostic. Lua `pcall` caught the expected
query error without a write; the enclosing script returned HTTP 200.

Both storage modes also passed INSERT, UPDATE, REPLACE, both UPSERT branches,
REMOVE, direct OLD/NEW projections, separate DOCUMENT reads of existing/absent
IDs, and using a fetched value as a bind variable through HTTP and Lua.
Restart preserved the complete final documents. No provider calls, production
data, or local environment edits were used. Temporary stores/logs are retained
at the evidence artifact's `temporary_root` for inspection.

- [Shared query fixtures](../../crates/cognigraph-query/tests/fixtures/mutation_backend_reads.json)
- [Reproducible release-server regression](../evidence/engineering-historical-checks.md#artifact-fa1db110f28dbefcccd9)
- [Pre-fix observations and binary hash](../evidence/engineering-historical-checks.md#artifact-8e9212b9fd04ee94f14b)
- [Final runtime observations and binary/fixture hashes](../evidence/engineering-historical-checks.md#artifact-f327e1474a0a8b8ec5be)
