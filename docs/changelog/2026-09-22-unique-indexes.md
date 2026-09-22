# v2.7.26 — Unique constraints on document collections

- Date: 2026-09-22
- Status: v2.7.26
- Kind: Storage contract and HTTP API

## Changes

Applications can declare unique constraints on ordinary document
collections and the Native engine enforces them on every write path
([CG-86](../issues/CG-86.md), [decision record](../decisions/decision_unique_indexes.md)):

- `GET`/`POST /api/collections/{name}/indexes` and
  `DELETE /api/collections/{name}/indexes/{index}`; CLI `index list`,
  `index ensure COLLECTION FIELD[,FIELD] [--name] [--sparse] [--non-unique]`,
  `index drop`. `GraphBackend` gains `list_indexes` and `drop_index`;
  `ensure_index` is no longer a no-op.
- Definitions and entries persist in redb (schema version 3, additive
  upgrade) and are written in the same transaction as the document, so the
  constraint holds after restart in every storage mode. Create, update,
  replace, delete, atomic batch and snapshot import check before persisting;
  a batch violation rolls back the whole batch; declaring over existing
  duplicates is refused and stores nothing. Snapshots carry `indexes`.
- Violations are `UniqueViolation` in the core error type, 409 with
  `code: "unique_violation"` over HTTP. CGQL mutations now preserve
  conflicts through the executor, so a duplicate `_key` or a unique
  violation inside `INSERT`/`UPSERT` returns 409 instead of 500.
- Dotted field paths, canonical JSON value identity, `sparse`, derived
  names and idempotent re-declaration are documented in the CGQL and HTTP
  references. Non-unique declarations are recorded only; secondary-index
  pushdown is later work. Edge collections are refused.

The workspace version moves to 2.7.26.

## Validation

The shared backend contract gains a unique-index case run in all four
storage modes. Native integration tests cover reopen in each persistent
mode, paged-mode enforcement over stored rows, snapshot round trips and
duplicate snapshots, concurrent inserts admitting one holder, durable drop
of a collection's indexes, and batch rollback with persistence. Unit tests
cover name derivation and value encoding. Server tests cover the 409 shape
and code, the index routes' declare/list/enforce/drop flow and every
refusal, and CGQL `INSERT`/`UPSERT`/`UPDATE` mapping to 409. The per-mode
HTTP acceptance harness declares a constraint, races concurrent violating
writes and checks survival across restart. Full local gates run before the
local merge. Not a release.
