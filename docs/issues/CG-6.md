# CG-6: Lua graph.query does not receive the configured CGQL budgets

- Status: Resolved
- Priority: P1
- Area: Query resource controls
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Live HTTP reproduction

## Problem

The server's CGQL row/deadline budget is applied by HTTP query handlers but is not passed into Lua graph bindings. Writable Lua uses the unbudgeted query helper; read-only Lua delegates to the backend's default unbudgeted query. Lua instruction hooks do not account for the Rust work performed inside a graph callback.

## Evidence

- `crates/cognigraph-lua/src/bindings.rs:50-81` — query execution without a server budget.
- `crates/cognigraph-server/src/routes/lua.rs` — engine construction carries instruction limits but no CGQL execution budget.
- `crates/cognigraph-server/src/routes/query.rs` and `routes/search/basic.rs` — budgeted HTTP paths.
- `docs/cgql-v1.md:445-452` — configured execution budgets.

## Reproduction / failure sequence

With `COGNIGRAPH_CGQL_MAX_SOURCE_ROWS=3`, HTTP query `FOR n IN [1,2,3,4,5] RETURN n` failed with the row-budget error. `/api/lua/execute` running `return graph.query("FOR n IN [1,2,3,4,5] RETURN n")` returned all five rows with HTTP 200.

## Acceptance criteria

- [x] Thread execution budgets through the Lua engine and both read-only and writable query bindings.
- [x] Use a shared script-level deadline so multiple callbacks cannot reset the request allowance indefinitely.
- [x] Add endpoint-level parity checks showing the same query is bounded from HTTP and Lua.

## Resolution — 2026-09-08

`LuaExecutionControl` carries the configured query budget into both CGQL
permission modes. Source-row caps apply per query; the time allowance covers
the whole script, including query parsing/planning and graph callbacks. Pending
backend futures observe that deadline and request cancellation. Synchronous
backend operations remain cooperative and committed writes are not rolled back.

Route tests and release-server HTTP checks prove parity under a three-row cap
for script-runner/editor roles; repeated small queries cannot renew the shared
deadline. Cancellation tests prove pending futures are dropped and the worker
is joined with the correct tenant's cache invalidated. Formatting, Clippy, and
the full Rust suite passed. See [batch verification](lua-controls-2026-09-08.md).

Run the repository Rust gates and a focused runtime regression before closing this issue. See [review evidence](review-2026-09-08.md).
