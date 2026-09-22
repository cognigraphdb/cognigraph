# Decision: unique constraints are persisted indexes enforced inside the write transaction

Status: accepted and implemented 2026-09-22 for [CG-86](../issues/CG-86.md).

## Context

No user-facing index API existed and the Native `ensure_index` was a no-op,
so applications could not rely on server-enforced uniqueness for identity
records (a user email, an OAuth provider and id pair, a token hash). Two
concurrent sign-ups could both succeed. The `IndexDef` contract, the
guarded and tenant wrappers and the redb schema-version marker already
existed.

## Decision

1. **Scope: uniqueness only.** Unique `persistent`/`hash` definitions on
   document collections are enforced constraints. Non-unique definitions
   are recorded and listed but not used for acceleration; secondary-index
   pushdown and `EXPLAIN` changes are later work under a separate ticket.
   Unique constraints on edge collections are refused: edge writes keep
   their upsert de-duplication by triple. Non-unique declarations are
   recorded on any collection type (internal callers declare them on edge
   collections) and never create a collection; a unique constraint on an
   unknown name materializes it as a document collection.
2. **Definitions and entries persist in redb.** Schema version 2 → 3 adds
   the `indexes` (`collection\0name` → definition) and `index_entries`
   (`collection\0name\0value` → document key) tables. Entries are written
   in the same transaction as the document, so the constraint is exact
   after a restart in every storage mode, and paged mode never loads
   document bodies to enforce it. The upgrade is additive; the existing
   metadata-only upgrade path re-stamps the identity UUIDs once.
3. **Enforcement before persistence, under the single write lock.** Every
   write path checks the resident mirror of the entries (create, update,
   replace, delete, atomic batch, snapshot import) and refuses with
   `UniqueViolation { collection, index, existing }` before any store
   change. Batches evaluate ops in order against a shadow so a value freed
   earlier in the batch is usable later; a violation rolls back the whole
   batch. Declaring over existing duplicates scans the stored rows and
   refuses without storing anything.
4. **Value identity is canonical JSON.** The key is the JSON array of the
   indexed values with sorted object keys; numbers keep their integer or
   float identity, strings are exact. Dotted paths address nested fields;
   a path through a non-object is absent. Non-sparse indexes store absent
   fields as null, so only one such document; `sparse: true` exempts a
   document when any indexed field is absent or null.
5. **Names and idempotence.** The name defaults to the fields joined by
   `_` plus `_unique`. The same definition again succeeds; a different
   definition under an existing name is a validation error; dropping by
   name lifts the constraint and reports whether it existed. Dropping a
   collection drops its indexes and entries.
6. **HTTP and CGQL surface it as 409.** The server maps the violation to
   409 with `code: "unique_violation"`, distinct from a duplicate `_key`
   conflict. CGQL mutations now preserve conflicts through the executor
   (`ExecutionError::Conflict` and `UniqueViolation`), so `INSERT` and
   `UPSERT` return 409 instead of the previous 500. Routes: `GET`/`POST
   /api/collections/{name}/indexes` and `DELETE
   /api/collections/{name}/indexes/{index}`; CLI `index list|ensure|drop`.
   The `POST` route defaults `unique` to true.
7. **Guards unchanged.** System, managed and generated collections refuse
   index declaration and drop through the existing guard; listing is a
   read. Snapshots carry `indexes` per collection and import declares them
   before the documents.

## Consequences

- Applications get server-side uniqueness with one query string per page
  and no client-side validation.
- Each write on an indexed collection adds an O(1) mirror check and one or
  two entry operations to its existing transaction.
- Fields named with a literal `.` cannot be indexed; that is documented.
- Revisit for secondary-index pushdown, TTL or vector index types, or a
  per-collection cap on the number of declared indexes.
