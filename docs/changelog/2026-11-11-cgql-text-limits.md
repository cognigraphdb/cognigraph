# v2.7.45 — CGQL text limits

- Date: 2026-11-11
- Status: v2.7.45
- Kind: Security fix

## Changes

[CG-105](../issues/CG-105.md) is resolved under the
[CGQL text limits decision](../decisions/decision_cgql_text_limits.md).

A deeply nested query on the read-only route overflowed a worker stack and
aborted the server. Query text is now checked before it is parsed: at most
262,144 bytes, nesting 32 and expression depth 256. Text beyond a limit gets
a parse error and HTTP 400. The check applies on both HTTP query routes, to
`EXPLAIN` and to Lua `graph.query()`.

The limits are sized from measured stack use on the server's 2 MiB worker
stack and are documented in the specification's new Query Text Limits
section.

**Breaking:** a query nested deeper than 32, or with a chain of more than
about 250 binary operators, is refused. Pass long value lists as a bind
variable and test them with `IN`.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (822 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,049 passed, 2 ignored)
pass, as does the release-only stack-margin test in the query crate. On a
2.7.45 Community release server the ticket's query and three other hostile
forms return HTTP 400 in about a millisecond, the worst accepted query
executes, and the server stays up. The registry, docs and decision-index
checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
