# Decision: construction gate refusals are recorded in a capped, generated ledger

Status: accepted and implemented 2026-09-22 for [CG-90](../issues/CG-90.md).

## Context

Refusing an unsupported fact is the product's central governance claim, yet
the request-time gates kept no record of what they refused. `POST
/api/construct/directed` returned its gate rejections only as strings in the
HTTP response, and `POST /api/construct/propose` did the same for skipped
gaps. The structured facts a gate judged (the nominated triple, the chunk,
the evidence quote and its span) were discarded once the response was sent,
so refusal counts in evidence records could not be regenerated from a
database and an operator could not ask what a gate refused last week.

## Decision

1. **Refusals are structured at the source.** The directed gate returns
   `Refusal { gate, source, relation, target, chunk_id, evidence, reason }`
   records; `gate` is a stable snake_case code per gate
   (`cognigraph_construct::refusals::DirectedGate`). The human-readable
   `reason` text is unchanged from the previous strings. Proposal skips
   carry a gate code the same way.
2. **The HTTP contract adds, it does not replace.** `skips` (directed) and
   `skipped` (propose) keep their existing string shape for current clients,
   including the console. A parallel `refusals` array carries the structured
   rows with their stored keys, plus `refusals_stored` and
   `refusals_dropped` counts.
3. **Rows live in a generated collection, `construction_refusals`.** It joins
   `side_views` and `fact_semantics` in `GENERATED_COLLECTIONS`: readable
   through generic reads and CGQL, publicly write-protected on every
   surface, never swept into promotion, attestation or materialization, and
   already included in `cognigraph export`. Rows are written through the
   internal handle in the same request that produced them, in one atomic
   batch.
4. **Rows are identified deterministically.** The key is a SHA-256 over
   origin, space type, gate, triple, chunk id and evidence, so re-submitting
   the same nomination records once rather than duplicating; an existing
   key counts as stored.
5. **Per-tenant cap of 10,000 rows, drop newest.** At the cap, new refusals
   are still returned in the response but not stored, and the response
   reports how many were dropped. There is no collection retention sweeper
   today; operators clear the ledger through snapshot tooling, and a sweeper
   is later work. Evicting oldest was rejected because it puts a delete and
   an ordering read on the construction hot path.
6. **A ledger write failure never hides a successful construction.** The
   facts were already written when the ledger is appended; a backend error on
   the ledger is logged and reported as `refusals_error` in the response
   rather than turning a completed ingest into an HTTP failure.
7. **Read path is CGQL, not a new route.** The dataops guide shows queries
   per space and per gate. A dedicated read route with filters and paging is
   deferred until a console workflow needs it.

## Consequences

- Refusal counts become reproducible from the database, and every refused
  nomination is traceable to its gate, chunk and quote.
- Each directed or propose request adds one collection listing and one
  batch write when refusals exist; requests without refusals add nothing.
- Retention is the operator's responsibility until a sweeper exists; the
  cap bounds growth per tenant store.
- Revisit when a console refusal view or a retention policy is scheduled:
  the row shape is the contract to keep.
