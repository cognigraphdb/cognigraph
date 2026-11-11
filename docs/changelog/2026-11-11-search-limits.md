# v2.7.48 — Search limits and field-name validation

- Date: 2026-11-11
- Status: v2.7.48
- Kind: Security fix

## Changes

[CG-109](../issues/CG-109.md) is resolved under the
[search limits decision](../decisions/decision_search_and_traversal_limits.md).

A text-search `limit` of 18446744073709551615, or a field name that was empty
or started with `-`, reached the text index library and panicked the worker;
the request got no response. A mid-range limit asked for terabytes.

- The text, vector, semantic, hybrid and graph-augmented routes refuse
  `limit` above 1,000 with HTTP 400; the graph-augmented route also caps
  `seed_limit` at 1,000 and `graph_facts_limit` at 10,000.
- Native clamps the text-search limit to the number of indexed documents, so
  the Lua binding is covered as well.
- Field names are validated with the index library's own rule and refused
  with a validation error.

**Breaking:** a search request for more than 1,000 results is refused.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (836 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,063 passed, 2 ignored)
pass. On a 2.7.48 Community release server the ticket's requests return HTTP
400, the boundary value is served and the server log has no panic. The
registry, docs and decision-index checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
