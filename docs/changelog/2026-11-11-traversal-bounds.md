# v2.7.49 — Traversal depth cap and path budget

- Date: 2026-11-11
- Status: v2.7.49
- Kind: Security fix

## Changes

[CG-110](../issues/CG-110.md) is resolved under the
[search and traversal limits decision](../decisions/decision_search_and_traversal_limits.md).

Typed traversal and graph-augmented search accepted any `max_depth`. Native
enumerates every simple path and copies the documents along each one, so on a
complete graph of ten documents depth 8 took 27.7 seconds and 5.5 GiB of
memory, and depth 50 never answered. Neither the request timeout nor the CGQL
budgets applied.

- `max_depth` above 10 is refused on `/api/graph/traverse`, on the
  graph-augmented route and in Lua `graph.traverse`.
- Native stops any traversal that expands more than 10,000 paths, in every
  storage mode and for every caller, CGQL included.
- Both are HTTP 400; the read-only query route now returns a backend input
  error as 400 instead of 500.

**Breaking:** a traversal that returns more than 10,000 paths must be
narrowed with a lower depth, one direction or a confidence floor.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (841 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,068 passed, 2 ignored)
pass. On a 2.7.49 Community release server the ticket's graph answers depth 4
as before and returns HTTP 400 for depths 5 to 10 in about 35 ms with 133 MiB
resident memory; depths 11 and 50 are refused by the depth cap. The registry,
docs and decision-index checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
