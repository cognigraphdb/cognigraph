# Decision: a CGQL error answers the status of whoever caused it

Status: accepted and implemented 2026-11-11 for [CG-198](../issues/CG-198.md).

## Context

Both query routes named a few execution errors and answered 500 for every
other one. That included the four budget errors, type errors in the caller's
own query, a missing bind variable, a missing collection and a mutation sent
to the read-only route. A 500 is logged at `warn`, counts as a server error
for monitoring, and tells a client that retrying may help. For a budget
refusal none of that is true: the request was understood and fails the same
way every time.

## Decision

1. **One mapping, no catch-all.** `cgql_errors::cgql_error` is the only
   place an `ExecutionError` becomes an HTTP response, for both routes. Its
   match is exhaustive, so a new error variant does not compile until its
   status is decided.
2. **Budget refusals answer 422.** The body carries
   `code: "query_budget_exceeded"`, the budget's name, its limit and unit.
   The message keeps the executor's text. The time limit, which the executor
   does not report, is taken from the budget the query ran under.
3. **Errors in the caller's query answer 400**: bind variables, type errors
   of computed values, unknown identifiers, functions and collation
   locales. A missing collection, or a document a mutation names that does
   not exist, answers 404, and a mutation on the read-only route 403. The
   backend's `CollectionNotFound` and `DocumentNotFound` are carried through
   the executor as typed errors instead of being flattened into a backend
   failure.
4. **A backend failure stays 500**, and an unavailable backend 503.
5. **Rejected: 408 for the time budget.** Browsers and proxies retry a 408,
   and the outer request timeout already uses it for a different condition.
6. **Rejected: 429 for budgets.** It means "slow down and retry" and carries
   `Retry-After`; a budget refusal is not about request rate.
7. **Rejected: 413.** It describes the request body, which is small here.
8. **Rejected: 400 for budgets.** The query is valid; a client must be able
   to tell "fix the query text" from "this query costs too much here".

## Consequences

- Clients that treated every query failure as a retryable 500 stop retrying
  budget refusals and type errors.
- Budget refusals are logged at `info` and no longer count as server errors.
- `CogniGraphError` gains `QueryBudgetExceeded`. Paths that do not go
  through the query routes still flatten execution errors into
  `QueryError`: Lua `graph.query()` (a script error) and
  `GraphBackend::query`.
- The verification harnesses that expected 500 for a source-row refusal, a
  mutation on the read route and `FOR x IN 1` now expect 422, 403 and 400.
