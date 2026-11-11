# Decision: search and traversal requests are bounded before they reach the engine

Status: accepted and implemented 2026-11-11 for [CG-109](../issues/CG-109.md)
and [CG-110](../issues/CG-110.md).

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

## Traversal (CG-110)

`POST /api/graph/traverse` and the graph-augmented route passed `max_depth`
straight to Native, which enumerates every simple path and copies the vertex
and edge documents along each one. On a complete graph of ten documents,
depth 8 took 27.7 seconds and 5.5 GiB; depth 50 never answered. The work is
synchronous under the store's read lock, so neither the request timeout nor
the CGQL budgets applied.

7. **Depth cap at the typed entry points.** `max_depth` above 10 is refused
   on the traversal route, the graph-augmented route and the Lua binding.
   CGQL validates its own depth with the same default.
8. **Path budget in Native.** A traversal that expands more than 10,000
   paths stops with a validation error, in resident and paged modes and for
   every caller, CGQL included. The budget is charged before the documents
   along a path are copied. Depth is not what costs: a 40-step chain is 40
   paths, while depth 5 on the ten-document complete graph is over 15,000.
9. **A constant, not an option.** The budget is `MAX_TRAVERSAL_PATHS` in the
   core crate. Making it configurable per request would add a field to an
   options type built in sixteen places for no present need.
10. **Rejected: a time budget only.** Memory, not time, is what a dense graph
    exhausts first, and a deadline checked between expansions bounds neither
    without also bounding the queue.

## Consequences

- Breaking: a request asking for more than 1,000 results is refused instead
  of served. Page through larger sets with a CGQL query.
- A panic inside a handler still closes the connection without a response;
  [CG-128](../issues/CG-128.md) tracks a panic boundary.
- A traversal that legitimately returns more than 10,000 paths must be
  narrowed: a lower depth, one direction, a confidence floor, or several
  queries. [CG-111](../issues/CG-111.md) covers the other CGQL budgets.
- The traversal still runs synchronously under the read lock. With the
  budget its duration is bounded, but it is not interruptible.
