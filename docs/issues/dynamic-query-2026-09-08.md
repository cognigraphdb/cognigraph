# CG-8 / CG-15 dynamic query execution — 2026-09-08

The completed CG-7, CG-29, CG-32, and model-baseline work was committed locally
as `41cdb8a`. This subsequent batch shares the backend read path between normal
execution and EXPLAIN ANALYZE and preserves query-wide accounting across every
resolution pass. CG-8 and CG-15 are resolved locally and were committed with
CG-16 as `41490db` on 2026-09-09. Unrelated research/draft work is preserved,
and nothing was pushed.

## Execution and reporting

`executor/resolve.rs` owns backend read execution, initial materialization,
document/traversal resolution, and head/tail retries. The normal parsed query,
precompiled backend plan, and analysis entry points call the same path. A
cloned `RunCx` retains shared accounting and optional analysis state while
installing that round's resolvers. There is no separate analysis-only data
evaluation path.

Final stage `rows` and `runs` exclude unresolved speculative passes. The new
`attempted_rows` and `attempted_runs` include all passes; restoring logical
stage counters never restores the row budget or erases source work. Final
`stats.result_rows` comes from settled execution. Materialized sources report
their fetched rows, fetch count, and combined fetch time. Correlated traversal
sources aggregate distinct start-value fetches; document lookups have separate
`stats.document_fetches` and `stats.document_fetch_ms` fields.

The static read detector now includes `COLLECT ... INTO` projection expressions,
so their DOCUMENT calls enter the same resolver. Existing query grammar,
read/write permissions, plain EXPLAIN behavior, and CG-7's mutation restriction
are preserved.

## Budget units and compatibility

One counter spans initial materialization, backend lookups, query-head retries,
deferred-tail retries, and nested plans. It charges:

- Scan/vector/static-traversal rows once at materialization. Reusing a cached
  site does not charge another backend fetch.
- One unit before each distinct backend document lookup, including missing
  documents. Duplicate IDs share the lookup; non-string values and strings
  without a collection/key separator cause no backend call or lookup charge.
- Correlated-traversal paths at fetch time, then each runtime expansion of
  those paths into rows, including speculative retries.
- Every array/variable source expansion, including nested and repeated passes.

These are source units, not a bound on all intermediate join rows or backend
scan effort. `stats.source_rows` exposes the cumulative units. Backend work may
differ from the in-memory dataset engine, which resolves document values
immediately. Configured caps can now reject queries that previously received
fresh allowances per phase; no operator configuration is changed automatically.

Dynamic batches check the existing query deadline before and after each backend
fetch. The existing four-round convergence bound remains. Backend operations
are still cooperatively timed. Document lookup error propagation remains the
separate issue CG-16 at this checkpoint; the same existing resolver error
behavior applied to normal and analyzed queries. The subsequent
[CG-16 batch](document-errors-2026-09-08.md) now propagates those failures.

## Verification

Eight new focused regression tests cover normal/analyzed/precompiled-plan
parity, dependent LETs, deferred nested plans, array sources, correlated
traversals, COLLECT INTO projections, settled versus attempted counters,
exact and one-below budget boundaries, duplicate/missing/malformed lookup
costs, early lookup-batch termination, deadlines, and convergence errors.
The focused query crate passed 141 tests. Formatting, Clippy with warnings
denied, and the full suite passed: **901 tests**, zero failed or ignored,
across 68 result summaries. The optimized release server built successfully.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server --bin cognigraph-server
python3 docs/issues/evidence/dynamic-query-http.py
```

## Release evidence

The saved pre-fix release reproduced both defects in resident and paged storage
through `/api/query`, `/api/search/query`, and Lua `graph.query()`. Normal
execution returned `["a","b"]`, while analysis reported zero rows. With a
three-unit cap, the same query succeeded despite needing three materialized
rows plus two distinct document lookups. The persisted documents remained
unchanged across restart.

The corrected release passed **60 normal/analysis comparisons** (ten query
shapes × three surfaces × two storage modes) and **240 budget-boundary query
executions**. Each boundary ran the normal and analyzed form with exactly the
required units and with one fewer unit: 120 accepted and 120 rejected as
expected. Shapes covered document filters, deferred/dependent LETs, correlated
traversals, mixed document/traversal resolution, array sources, deferred nested
subqueries, COLLECT INTO projections, empty results, and DISTINCT.

For the original filter case, analysis now reports two final rows, three
settled FOR rows, six attempted FOR rows over two passes, two document fetches,
and five source units. All matched observed normal results and the declared
accounting. Plain EXPLAIN remained inert under every tested cap, even with an
absent collection and omitted bind value. Both stores preserved the exact
documents across restart. The regression used only disposable local data and
loopback HTTP, with no external providers or environment-file edits.

No final validation or live check failed. The initial new unit regressions
failed against the old implementation as expected. Budget failures retain the
existing HTTP 500 mapping on the query routes; Lua pcall catches the query
error and the enclosing script returns HTTP 200. This batch changes budget
enforcement and diagnostic counts, not those runtime error status mappings.

- [Reproducible provider-free HTTP regression](evidence/dynamic-query-http.py)
- [Pre-fix observations and binary hash](evidence/dynamic-query-baseline-http-2026-09-08.json)
- [Corrected release observations and binary hash](evidence/dynamic-query-http-2026-09-08.json)
