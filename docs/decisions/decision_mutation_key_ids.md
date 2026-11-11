# Decision: a mutation key names a document of the mutation's own collection

Status: accepted and implemented 2026-11-11 for [CG-106](../issues/CG-106.md).

## Context

`UPDATE`, `REPLACE` and `REMOVE` take a key expression and a target
collection. The executor accepted "a plain key or a full id" by keeping
whatever followed the last `/` in the string. The collection part of an id
was never compared with the target, so `REMOVE "organizations/42" IN persons`
deleted `persons/42`. The natural edge pattern
`FOR e IN rels REMOVE e._to IN persons` therefore wrote to any document of
`persons` that shared a key with the edge's target. Splitting from the right
also disagreed with `PARSE_IDENTIFIER`, which splits from the left because a
key may contain `/`.

## Decision

1. A key expression that evaluates to a string without `/` is a plain key.
2. A string with `/` is a full id. It is split once from the left. The part
   before the slash must equal the target collection exactly, and the rest
   is the key.
3. Anything else is an error raised before that row's write:
   `MutationKeyOutsideCollection`, returned as HTTP 400. This covers ids of
   other collections, an empty collection or key part, and a different
   letter case.
4. A key that contains `/` can only be addressed by its full id.
5. **Rejected: treating a non-matching string as a plain key that contains
   `/`.** `REMOVE "a/b" IN docs` would then silently mean "key `a/b`", and the
   edge-endpoint mistake would become a quiet no-op instead of an error.
6. **Rejected: keeping the right split with a collection check.** It cannot
   address a key that contains `/` at all.

## Consequences

- Breaking for statements that passed an id of another collection and relied
  on the key part. No test or documented example did.
- Multi-row mutations are still applied per document: rows before the
  failing row stay applied, as the specification already says.
- `UPSERT` is unaffected: its `_key` search field was always a plain key.
