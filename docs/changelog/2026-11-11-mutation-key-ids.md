# v2.7.46 — Mutation keys name the target collection

- Date: 2026-11-11
- Status: v2.7.46
- Kind: Data-integrity fix

## Changes

[CG-106](../issues/CG-106.md) is resolved under the
[mutation key decision](../decisions/decision_mutation_key_ids.md).

`UPDATE`, `REPLACE` and `REMOVE` accepted a full document id as the key but
kept only what followed the last `/`. `REMOVE "organizations/42" IN persons`
deleted `persons/42`, and `FOR e IN rels REMOVE e._to IN persons` wrote to
whichever person shared a key with the edge's target.

A key is now a plain key or a full id of the mutation's own collection,
split once from the left like `PARSE_IDENTIFIER`. Any other id is refused
before that row's write and returned as HTTP 400. A non-string key is 400 as
well, instead of 500.

**Breaking:** a key that contains `/` must be given as a full id, for example
`REMOVE "docs/a/b" IN docs`.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (827 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,054 passed, 2 ignored)
pass. On a 2.7.46 Community release server the ticket's statement returns
HTTP 400 and both documents survive, while plain keys and matching ids still
work. The registry, docs and decision-index checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
