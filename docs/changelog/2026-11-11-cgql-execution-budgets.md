# v2.7.53 — CGQL intermediate-row, memory and time budgets

- Date: 2026-11-11
- Status: v2.7.53
- Kind: Resource-exhaustion fix

## Changes

[CG-111](../issues/CG-111.md) is resolved under the
[CGQL execution budgets decision](../decisions/decision_cgql_execution_budgets.md).

A few hundred bytes of CGQL could exhaust the server. A nested `FOR` built
its cross product without any charge, a join on a non-string key did the
same per row, `COLLECT` and `SORT` never read the clock, and strings and
arrays had no size limit. The request timeout could not interrupt any of it
because execution is synchronous.

- **Intermediate-row budget**, default 100,000,000
  (`COGNIGRAPH_CGQL_MAX_INTERMEDIATE_ROWS`). A join is charged before it is
  built.
- **Memory budget**, default 1 GiB (`COGNIGRAPH_CGQL_MAX_MEMORY_BYTES`): an
  estimate of what a query builds and keeps, checked before a value is
  copied.
- **Time budget on by default**: `COGNIGRAPH_CGQL_TIME_BUDGET_MS` now
  follows `COGNIGRAPH_REQUEST_TIMEOUT_SECS` when unset. Joins, `COLLECT`,
  `SORT` and `DISTINCT` read the clock, and a large `SORT` can stop.
- `0` switches a budget off. Lua `graph.query()` and internal callers get
  the same row and memory limits.

**Behaviour changes:** a query that builds or returns more than about 1 GiB
fails with `query exceeded its memory budget`; a query that runs past the
request timeout fails with `query exceeded its time budget` instead of
finishing after the client has gone. `ExecutionBudget` has two new fields
and its default is no longer unlimited.

Filed while fixing: [CG-197](../issues/CG-197.md) (execution holds a runtime
worker) and [CG-198](../issues/CG-198.md) (budget errors answer HTTP 500).

## Validation

At this commit: `cargo fmt --all -- --check`, both Clippy gates with
`-D warnings`, the query, Native and Lua test crates (315 passed) and the
server's budget configuration tests pass. The full suites passed on an
earlier state of this change (867 Community, 1,096 Enterprise), before the
memory estimate was recalibrated against a counting allocator and repeated
rounds were released. They are run again on the next commit of the batch,
v2.7.54, whose record has the totals.

Live check on the 2.7.53 release binary with default settings and three
2,000-document collections, over `POST /api/search/query`:

| Query | Result |
|---|---|
| `FOR a IN c FOR b IN c FOR d IN c RETURN 1` | memory budget error at once, process at 29 MB |
| Join on numeric ids, 2,000 x 2,000 | memory budget error after 0.31 s, process at 1,113 MB |
| Thirteen `CONCAT` levels (10^14 bytes asked) | memory budget error after 0.11 s |
| 2,000 runs of a subquery over 2,000 rows | answers in 1.6 s; with a 1 s request timeout, time budget error at 1.00 s |

One machine, one run; the process figures are resident size after the
query and include what the allocator had not returned. A counting-allocator
measurement of the estimate is in the decision record. The registry, docs
and decision-index checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
