# Decision: CGQL bounds intermediate rows, memory and time by default

Status: accepted and implemented 2026-11-11 for [CG-111](../issues/CG-111.md).

## Context

CGQL had two budgets, source rows and wall-clock time, and both were off by
default. Neither covered the work after the scans. A nested `FOR` built its
cross product with no charge and no deadline check. A join on a non-string
key fell back to the same expansion. `COLLECT` and `SORT` never read the
clock. Strings and arrays had no size limit, so ten `LET` lines of
`CONCAT(a, a, …)` asked for more memory than a machine has. Execution is
synchronous inside the request handler, so the request timeout could not
interrupt any of it, although the reference said it was the outer guard.

## Decision

1. **Intermediate-row budget.** One counter per query: rows every `FOR`
   produces, rows `COLLECT` and `SORT` read. A join is charged before it is
   built. Default 100,000,000.
2. **Memory budget.** A per-query meter of estimated bytes, modelled on the
   engine's JSON representation: 32 per value, a string's length plus 16,
   640 per six object fields, 160 per row variable. It has a retained part (`LET` values, joined rows, `COLLECT`
   state, results; `SORT` keys and subquery rows are returned when done) and
   a part for the expression being evaluated. Reads are counted before the
   copy, so the limit holds before an oversized value exists. Default 1 GiB.
3. **Time budget on by default.** Unset, it equals the request timeout.
   The deadline is read per row in every stage, and a large `SORT` runs as
   sorted runs plus stable merges so it can stop.
4. **The library default is bounded too.** `ExecutionBudget::default()`
   carries the row and memory defaults, so internal callers are covered.
   `ExecutionBudget::unlimited()` is explicit.
5. **Source rows stay off by default.** A plain scan is governed by the
   collection's size; the right cap depends on the deployment.
6. **Rejected: a per-value size limit alone.** Memory is rows times value
   size, and each clause can copy a value once per row. A per-value cap
   bounds neither.
7. **Rejected: measuring the allocator.** A per-query allocator is not
   available without replacing the JSON value type; the estimate is
   deterministic and the same on every machine.
8. **Rejected: checking the clock inside the sort comparator.** The standard
   sort may panic when a comparator stops being a consistent order.

## Consequences

- Queries that return or join more than about 1 GiB of values fail by
  default with `query exceeded its memory budget`. Raise
  `COGNIGRAPH_CGQL_MAX_MEMORY_BYTES` or add a `LIMIT`.
- A query that runs longer than the request timeout now fails with
  `query exceeded its time budget`. Before, it ran to completion on a
  runtime worker after the client had gone.
- The estimate is not real memory. Measured with a counting allocator in a
  release build on one Apple-silicon laptop, peak allocation was 92% to 97%
  of the estimate for 360,000 joined rows of small and of fourteen-field
  documents, a string `LET` per row, 600 returned documents and
  `COLLECT ... INTO`. For a join followed by an object projection it was
  66%: joined rows stay counted until the query ends although they are freed
  as they are projected. An earlier per-field formula undercounted small
  objects by a factor of two. The estimate does not cover the source
  documents a scan holds.
- Cost, measured on 200,000 six-field documents in a release build on one
  Apple-silicon laptop, best of five runs, default budgets against unlimited:
  `RETURN d` 171 ms against 152 ms (about 12% slower, the document is
  measured when read and when returned); an object projection, a filter with
  a function, `SORT`, `COLLECT` and a `LET` with a function were within 3%
  either way. One machine, one data shape; not a benchmark claim.
- A query still occupies a runtime worker until it ends;
  [CG-197](../issues/CG-197.md) tracks moving execution off the async
  workers.
- A stopped query still needs the time to free what it built, and a single
  function call or copy is not interrupted. With the memory budget on, both
  are bounded by that budget.
- Budget errors answered HTTP 500 until v2.7.56. Since
  [CG-198](../issues/CG-198.md) they answer 422 with a typed body; see the
  [error statuses decision](decision_cgql_error_statuses.md).
- `EXPLAIN ANALYZE` does not report the two new counters.
