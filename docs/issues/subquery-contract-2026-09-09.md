# CG-28 subquery contract verification — 2026-09-09

The specification now agrees with the parser's supported expression positions,
post-`COLLECT` rejection, and narrower mutation restrictions. The obsolete
LET-only subsection and unsupported bullet are removed. The decision record
distinguishes its original July scope from the implemented amendment. No Rust
implementation or grammar changed; this batch adds documentation and corpus
examples. CG-23 and CG-28 were committed locally as `b9084e6` on top of
`9fb4933`; nothing was pushed. Counts and known failures below describe that
batch; the subsequent CG-37 update is recorded at the end.

## Source and examples

The [grammar](../../crates/cognigraph-query/src/grammar.pest) restricts each
subquery to `read_query`. The [parser](../../crates/cognigraph-query/src/parser/mod.rs)
lowers expressions to synthetic LETs in body clauses, grouping keys, aggregate
arguments, and supported tail positions. It rejects inline subqueries in
`SORT`/`RETURN` after `COLLECT` during parsing, including uncorrelated ones.
It does not lower inline subqueries in `COLLECT … INTO` projections or mutation
selectors/payloads; the [validator](../../crates/cognigraph-query/src/validation.rs)
rejects those raw expressions. The old LET-only diagnostic describes the
internal form and is not the complete user-facing grammar.

Ordinary read subqueries can supply the mutation's optional `FOR` source or a
body `LET`, and the mutation can reference the resulting values. The
[CG-7 validator](../../crates/cognigraph-query/src/validation/mutation.rs) still
rejects dynamic `DOCUMENT()` and row-dependent traversal throughout mutation
queries. A subquery cannot contain a write. Precomputing a value before
`COLLECT` is insufficient by itself: a group binding, aggregate, or `INTO`
projection must carry it into the post-grouping scope.

The [specification's position table](../reference/cgql.md#subqueries-in-expression-position)
links **18 added corpus cases**: nine executable forms with expected JSON,
two mutation-body forms, three parse rejections, and four validation rejections.
The execution examples run against both the in-memory executor and Native;
the other cases pin the parse-versus-validation boundary. Existing depth,
shadowing, correlation, and mutation tests remain unchanged.

The nested-expression probe exposed [CG-37](CG-37.md): separate desugaring
calls reset their generated-name counters, allowing an inner `$sq0` to collide
with one already visible from the outer scope. Both Rust engines and the
release HTTP/Lua paths reject the reproducer. A standalone nested control
succeeds. Its fixture originally recorded the known validation failure; CG-37
subsequently moved it to the execution corpus. CG-28 does not claim
to fix this defect or redefine it as intended language behavior.

## Rust verification

- `cargo test -p cognigraph-query`: **144 passed**, zero failed/ignored.
- `cargo test -p cognigraph-native --test cgql_corpus`: **1 passed**; all shared
  execution cases, including the nine new cases, match expected results.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: **950 reported passes**, zero failed/ignored. Adding
  corpus files does not add Rust test functions; each corpus runner executes
  multiple files. Eight Arango integration entries early-returned without
  credentials, so this is not new live Arango coverage.

The full gates ran in the required order. The production source and release
binary are unchanged from CG-22; the binary hash is checked against that batch's
validation artifact. Gate and fixture hashes are recorded in the
[validation artifact](../evidence/engineering-historical-checks.md#artifact-ce9180259737a08450d6).

## Release verification

The [harness](../verification/harnesses/subquery-contract-http.py) starts the release binary with
authenticated, disposable resident/sidecar and paged/sidecar Native stores,
explicit configuration, and no embedding/completion provider. It seeds the
shared corpus through document/relationship routes; generated edge keys are
not used in comparisons. It runs from a temporary directory rather than loading
the checkout's local environment. Both processes stop after verification.

[HTTP/Lua evidence](../evidence/engineering-historical-checks.md#artifact-8a8eb5bf06340e73565f) records
**148 checks** across both modes:

- 54 exact read results through `/api/query`, `/api/search/query`, and Lua
  `graph.query()`, plus 18 analyzed result-count checks.
- 28 expected rejections through `/api/query` and caught Lua errors, including
  four reproductions of CG-37. HTTP parse/validation errors return 400; the Lua
  `pcall` wrapper returns its own successful HTTP envelope containing the error.
- 28 exact before/after snapshot comparisons show rejected queries preserve
  both the source documents and the mutation destination.
- Eight successful mutation executions use the two body forms through HTTP
  and Lua; eight subsequent reads verify their stored values.
- Four known-status observations reproduce [CG-38](CG-38.md): the read-only
  `/api/search/query` route reports parse/validation failures as HTTP 500, while
  `/api/query` reports 400. These assertions preserve the observed defect;
  they do not claim the desired response behavior passes.

This checks the current syntax and execution contract, not performance,
concurrency, a new model benchmark, or a new transactional guarantee. Supported
mutations here use synthetic data only and retain per-document atomicity.

## Corrected attempts and remaining work

The first aggregate fixture expected integer `4` while `SUM` returns JSON
floating-point `4.0`; its golden output was corrected. The combined LET/nested
example then exposed CG-37, which was recorded separately and retained as a
known-failure case. The first live harness attempted to serialize a caught Lua
error value directly, causing HTTP 500; it now converts the caught error with
`tostring`, and the complete run passes. No product change was used to bypass
these observations.

Re-run with:

```bash
cargo test -p cognigraph-query
cargo test -p cognigraph-native --test cgql_corpus
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/subquery-contract-http.py --output /tmp/cg28-http.json
```

Local documentation links, fixture references/counts, registry totals, and
unrelated-file preservation were checked. No research drafts or `ui/` files
were edited. The registry had **29 Resolved and 9 Open** issues after closing
CG-28 and adding CG-37/CG-38. The next bounded query fix was CG-37. Model benchmarking
remains deferred with Luna as the economical baseline.

## Subsequent CG-37 verification

[CG-37](subquery-bindings-2026-09-09.md) resolves nested generated-name collisions
with one allocator per parsed query. The original failing example now executes,
with four additional examples; the current harness runs 180 observations and
accepts fixture bind variables on HTTP/Lua and analyzed requests. Its default
requires the corrected results; `--expect-collisions` reproduces the old release.
The 148-observation artifact and its recorded hashes remain historical evidence
of the CG-28 batch. [CG-38 is subsequently resolved](query-error-status-2026-09-09.md):
the current harness requires matching HTTP 400 plan-error responses by default.
