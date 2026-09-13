# CG-8: EXPLAIN ANALYZE uses different DOCUMENT semantics than execution

- Status: Resolved
- Priority: P2
- Area: CGQL diagnostics
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Pre-fix release reproduction; final Rust gates and release HTTP/Lua parity passed
- Committed: `41490db` (2026-09-09, local only)

## Problem

`EXPLAIN ANALYZE` enters an execution path that never installs or runs the dynamic document/traversal resolver. Queries accepted and correctly evaluated by normal execution can therefore produce misleading zero-row statistics or unresolved traversal behavior under analysis.

## Evidence

- `crates/cognigraph-query/src/executor/mod.rs:22-42,284-299` — early analysis branch versus document-resolving execution.
- `docs/cgql-v1.md:680-704` — analysis promises execution statistics for the query.

## Reproduction / failure sequence

`FOR n IN notes FILTER DOCUMENT(n.ref).v == 2 RETURN n._key` returned `["a","b"]`. Prefixing it with `EXPLAIN ANALYZE` reported `result_rows:0` and a FILTER stage with zero rows.

## Acceptance criteria

- [x] Share the same resolution and execution semantics between normal queries and analysis.
- [x] Report actual source work across resolution passes without counting speculative rows as final results.
- [x] Add analysis parity tests for DOCUMENT, dependent LETs, and correlated traversals.

## Resolution — 2026-09-08

Normal execution, precompiled backend plans, and analysis share
`executor/resolve.rs`. Final stage rows/runs exclude unresolved passes, while
attempted stage counts and source fetch statistics retain actual retry work.
The detector also covers DOCUMENT inside COLLECT INTO projections. CG-15's
shared accounting was resolved in the same batch.

Formatting, Clippy, all 901 tests, 60 release normal/analysis comparisons,
240 budget-boundary executions, plain EXPLAIN checks, and resident/paged
restart preservation passed. No final validation or live check failed.
See [implementation, compatibility, and evidence](dynamic-query-2026-09-08.md).
