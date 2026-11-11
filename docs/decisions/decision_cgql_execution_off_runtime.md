# Decision: HTTP CGQL runs off the async workers, bounded and cancellable

Status: accepted and implemented 2026-11-11 for [CG-197](../issues/CG-197.md).

## Context

A CGQL query fetches its sources and then joins, groups and sorts without
yielding. Both query routes ran that inside the request future, on a Tokio
worker. The [budgets decision](decision_cgql_execution_budgets.md) made the
time budget end such a query, but until then the worker served nothing else.
With as many long queries as workers, every other request waited, health
checks included. Lua already ran on a blocking thread.

Two things followed from execution being synchronous on the worker. The
request timeout could not fire while a query ran, so the time budget was set
equal to it. And a client that disconnected was not noticed until the query
ended.

## Decision

1. **A blocking thread per query.** Both routes call one runner. It hands
   parsing, source fetches and computation to Tokio's blocking pool and
   awaits the result, so the worker is free while the query runs. The
   request's tenant context is captured and re-entered on that thread;
   without it an Enterprise query would read the default tenant's store.
2. **A bounded number at once.** `COGNIGRAPH_CGQL_MAX_CONCURRENT_QUERIES`,
   default the number of CPUs. The work is CPU-bound, so more would only
   take turns, and the limit times the memory budget bounds what CGQL can
   hold. A query beyond the limit waits; the slot is held until the work has
   stopped, not until the request has gone. There is no unlimited setting.
3. **Waiting does not use the time budget.** The budget starts when the
   query starts to run, so a 422 for `time` still means the query itself ran
   that long and would again. The request timeout bounds the wait and
   answers 408, as for any request that is not served in time.
4. **Cancellation.** The executor takes a `Cancellation` flag and reads it
   wherever it reads the deadline: per row in every stage, once every 1,024
   rows in inner loops, between sort runs. The runner raises it when the
   request future is dropped. A cancelled query ends at its next check, also
   with the time budget off. Lua's script cancellation raises the same flag
   for a running `graph.query()`.
5. **A supervisor for writes.** A task that outlives the request joins the
   worker and, on the read-write route, sweeps cached results whatever the
   outcome. A write stopped by cancellation keeps the documents it had
   already written, as a write stopped by a budget does.
6. **Default time budget: nine tenths of the request timeout.** Now that the
   request timeout can fire while a query runs, a budget equal to it would
   lose the race every time and the caller would get a bare 408 instead of
   the typed 422. An explicit `COGNIGRAPH_CGQL_TIME_BUDGET_MS` is unchanged.
7. **Rejected: yielding to the runtime inside the executor.** It would make
   the synchronous row pipeline async throughout, and it shares the workers
   fairly without bounding how many queries compute at once.
8. **Rejected: `block_in_place`.** It keeps the tenant scope for free, but
   it panics on a current-thread runtime, starts a replacement worker per
   query with no bound, and still cannot notice a dropped request.
9. **Rejected: counting the wait against the time budget.** A query that
   waited 20 seconds and ran 7 would be refused for `time` although it would
   pass on an idle server; 422 is documented as deterministic.
10. **Rejected: refusing with 429 when no slot is free.** A short wait is
    the common case under a burst; the request timeout already bounds it.

## Consequences

- With the default budget a query that runs too long is refused after 27
  seconds, not 30. A query whose wait plus run exceeds the request timeout
  gets 408 and is stopped.
- A client disconnect now stops a read-write query between documents.
  Before, the query ran to its end or its budget.
- Each query costs one task spawn and one hand-off to the blocking pool.
- A worker that panics answers 500 instead of unwinding the request task.
- Lua scripts are not counted against the limit; they already ran on
  blocking threads. `GraphBackend::query` and other internal callers run
  where they are called.
- Source fetches run on the blocking thread too. Native storage calls made
  by other routes still run on the async workers.
- A stopped query still needs the time to free what it built, and a single
  function call or copy is not interrupted; the memory budget bounds both.
