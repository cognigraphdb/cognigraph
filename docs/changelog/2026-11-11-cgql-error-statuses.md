# v2.7.56 — CGQL errors answer the status of whoever caused them

- Date: 2026-11-11
- Status: v2.7.56
- Kind: API correctness

## Changes

[CG-198](../issues/CG-198.md) is resolved under the
[CGQL error statuses decision](../decisions/decision_cgql_error_statuses.md).

`POST /api/query` and `POST /api/search/query` answered HTTP 500 for every
execution error they did not name. A budget refusal, a type error in the
caller's query, a missing bind variable, a missing collection and a mutation
sent to the read-only route all looked like server failures.

| Case | Before | Now |
|---|---|---|
| Query stopped at a budget | 500 | 422 with `code`, `budget`, `limit`, `unit` |
| Type error, bind variable, unknown locale | 500 | 400 |
| Missing collection, mutation of a missing document | 500 | 404 |
| Mutation on the read-only route | 500 | 403 |
| Backend failure | 500 | 500 |

Both routes now share one mapping with no catch-all.

**Breaking:** clients that matched on status 500 or on the
`Query execution error:` prefix for these cases see the new status and a
`Validation error:`, `Forbidden:`, `Collection not found:` or
`Query budget exceeded:` prefix. `ExecutionError` gains `DocumentNotFound`
and `CogniGraphError` gains `QueryBudgetExceeded`.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (874 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,104 passed, 2 ignored)
pass. The Native runtime harness passes with its updated expectations (348
checks per edition) and the subquery contract harness passes (288 checks).

Live check on the 2.7.56 Enterprise release binary, both routes: memory,
time (1 s request timeout), intermediate-row and source-row refusals each
answered 422 with the typed body; a type error, a missing bind variable and
a syntax error 400; a missing collection and an `UPDATE` of a missing
document 404; a mutation on the read route 403. The server logged the
refusals at `info` and nothing at `warn`. The registry, docs,
decision-index and server-modularity checks pass.

Not run: the UI, Helm, Docker, advisory and dependency suites. Nothing was
pushed, released or deployed.
