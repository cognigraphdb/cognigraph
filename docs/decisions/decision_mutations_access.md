# Decision: CGQL mutations semantics and access control

Date: 2026-07-02 (M7, design discussion held first) · Status: ACCEPTED

## Context
Making CGQL read-write turns every query surface into a potential write
surface; predates auth.

## Decision (owner: skitsanos, five explicit calls)
1. v1 = INSERT / UPDATE / REPLACE / REMOVE; UPSERT deferred. UPDATE is a
   partial merge; REPLACE swaps whole documents.
2. New `POST /api/query` endpoint for read-write (clean RBAC attachment).
3. Lua `graph.query()` read-only until RBAC exists, then lifted.
4. Per-document atomicity; batch transactions marked planned.
5. Ship before auth, gated by CGQL_MUTATIONS_ENABLED=false.

## Outcome
All five held. The QueryMode split later gave Phase 8 its natural scope
attachment point, and the Lua lift landed exactly as scheduled when RBAC
shipped (M8). One design wrinkle surfaced by tests: `IN` ambiguity forced
mutation operands to additive expressions. Both deferrals closed 2026-07-03:
UPSERT (key fast path + all-fields match) and atomic batch transactions
(`execute_batch` capability, one redb transaction, POST /api/batch) — the
mid-batch-failure test proves full rollback.
