# Decision: search routes cap their counts and validate field names before the index library

Status: accepted and implemented 2026-11-11 for [CG-109](../issues/CG-109.md).

## Context

`POST /api/search/text` passed a caller's `limit` straight to the text index
library, which reserves room for twice that many hits in every segment before
it looks at a document. `limit: 18446744073709551615` panicked the worker
with "capacity overflow"; a mid-range value asks for terabytes, and on a host
that refuses the allocation the whole process aborts. Hybrid search
multiplied the limit by three first. Field names had the same problem: the
library asserts that a name is non-empty and does not start with `-`, so
`fields: [""]` panicked as well. A panicking request gets no HTTP response.

## Decision

1. **Server caps.** `limit` on the text, vector, semantic, hybrid and
   graph-augmented routes, and `seed_limit` on the graph-augmented route, may
   not exceed 1,000. `graph_facts_limit` may not exceed 10,000. A larger
   value is HTTP 400. The check runs first in each handler.
2. **Native clamp.** Text search clamps the limit to the number of indexed
   documents before building the collector. Any other caller, such as the Lua
   binding, is covered by this without a cap of its own.
3. **Field names.** Validation calls the index library's own field-name
   predicate, so the two cannot drift apart, and returns a validation error.
4. **No wrapping arithmetic.** The limit multipliers in hybrid and vector
   search saturate.
5. **Rejected: only the server cap.** The Lua binding and any future caller
   would still reach the library with an unbounded limit.
6. **Rejected: only the Native clamp.** Vector, semantic and graph-augmented
   search allocate from the limit in other places; a documented cap at the
   boundary is simpler to reason about than auditing each one.

## Consequences

- Breaking: a request asking for more than 1,000 results is refused instead
  of served. Page through larger sets with a CGQL query.
- A panic inside a handler still closes the connection without a response;
  [CG-128](../issues/CG-128.md) tracks a panic boundary.
- Typed traversal depth is a separate limit; see [CG-110](../issues/CG-110.md).
