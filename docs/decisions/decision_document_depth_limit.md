# Decision: stored documents nest at most 100 levels, and an unreadable row fails closed

Status: accepted and implemented 2026-11-11 for [CG-107](../issues/CG-107.md).

## Context

Native serialized documents without a depth check and read them back with a
JSON parser that stops at 128 nested containers. HTTP request bodies go
through the same parser, so the gap was reachable only through values built
inside the server: 135 runs of a CGQL `UPDATE` that wraps a field in another
object each time, or a deep Lua table. Every write succeeded. After a restart
a resident store failed to open; in paged or sidecar mode every scan of that
collection failed.

## Decision

1. **Limit.** A stored document may nest arrays and objects 100 levels deep,
   the document itself being level 1. The parser's limit is 128; the
   difference leaves room for the envelopes that wrap a document in a
   snapshot (four levels) or a response.
2. **One choke point.** The check is in `NativeBackend::persist`, which every
   write path reaches with or without a store behind it, before anything is
   committed or counted. A deep document in an atomic batch refuses the whole
   batch.
3. **Not recursive.** The depth walk uses an explicit stack, because the
   value comes from a caller and may be deeper than a recursive walk
   survives.
4. **Client error.** The backend returns a validation error. CGQL carries it
   as `InvalidInput`, and the HTTP routes answer 400.
5. **An unreadable stored row fails closed.** A row written before this limit
   cannot be parsed. The store reports which row (`collection/key`) and
   refuses to open in resident mode, or fails that scan in paged mode.
6. **Rejected: skipping the unreadable row.** The store would open, but the
   document would vanish from reads, exports and backups with only a log
   line to show for it. For an evidence store, a loud failure is the smaller
   harm.
7. **Rejected: parsing stored rows without a depth limit.** A row can be
   arbitrarily deep, and an unbounded recursive parse would overflow the
   stack instead of returning an error.

## Consequences

- Breaking: documents nested deeper than 100 levels are refused. Existing
  documents between 101 and 127 levels stay readable but cannot be rewritten
  until they are flattened.
- A store that already holds an unreadable row needs repair outside the
  server. No tool does that yet; [CG-196](../issues/CG-196.md) tracks it. New
  rows of that kind can no longer be written.
- The limit covers what is stored. Deep values that exist only inside one
  query are bounded by the [CGQL text limits](decision_cgql_text_limits.md).
