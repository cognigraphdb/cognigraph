# v2.7.57 — HTTP CGQL runs off the async workers and can be cancelled

- Date: 2026-11-11
- Status: v2.7.57
- Kind: Availability

## Changes

[CG-197](../issues/CG-197.md) is resolved under the
[execution decision](../decisions/decision_cgql_execution_off_runtime.md).

A CGQL query ran inside its request future, on a Tokio worker, until it
finished or its time budget ended it. As many long queries as workers left
none for other requests, health checks included.

- `POST /api/query` and `POST /api/search/query` run each query on a
  blocking thread. The request's tenant context is carried to it.
- New setting `COGNIGRAPH_CGQL_MAX_CONCURRENT_QUERIES`, default the number
  of CPUs. A query beyond the limit waits for a slot; the wait does not use
  its time budget, and the request timeout bounds it with 408.
- A query stops when its request is dropped, on a client disconnect or the
  request timeout, at its next deadline check. Lua's script cancellation
  now stops a running `graph.query()` the same way.
- The read-write route sweeps cached results after the query has ended,
  also when nobody is waiting for the answer.

**Changed defaults and behavior:**

- The default CGQL time budget is nine tenths of the request timeout
  (27,000 ms with the default 30 seconds), not the whole timeout. An
  explicit `COGNIGRAPH_CGQL_TIME_BUDGET_MS` is unchanged.
- A query whose wait plus run exceeds the request timeout answers 408 and
  is stopped. Before, the request timeout could not interrupt a query.
- A client disconnect stops a read-write query between documents; documents
  already written remain.
- `cognigraph-query` gains `Cancellation`,
  `parse_and_execute_backend_cancellable` and `ExecutionError::Cancelled`.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (888 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,119 passed, 2 ignored)
pass. The Native runtime harness passes on both release editions (348
checks each).

The starvation test failed on v2.7.56: with two long queries on a
two-worker runtime a health request waited 2.6 seconds. It passes now.

Live check on the 2.7.57 Enterprise release binary with two runtime workers
(one laptop, twelve CPUs): 51 health probes answered in at most 0.6 ms
while six long queries ran, on either route; each long query answered 422
for `time` at 9.0 seconds under a 10-second request timeout; a client
disconnect freed the only slot within 3 ms; a query running at the request
timeout answered 408 and was stopped.

Not run: the full CI and Docker suites, which the push gate runs. The cost
of the thread hand-off per query was not measured.
