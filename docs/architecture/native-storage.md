# Native Backend Storage Model

Status: implemented with M3 (2026-07-02); identity metadata upgraded to schema
2 by CG-11 (2026-09-08). Applies to `cognigraph-native`.

## Design

In the default resident mode, the native backend is **memory-primary,
redb-durable**:

- The full working set lives in memory (`NativeState`: collection → ordered
  key → JSON document), exactly as in the pure in-memory backend. All reads —
  document lookups, listing, traversal, vector search, CGQL execution — are
  served from memory and never touch disk.
- When a storage path is configured, every write operation is **written
  through** to a [redb](https://github.com/cberner/redb) database and
  committed *before* the in-memory state is mutated. If the commit fails, the
  operation fails and memory is untouched.
- On startup the entire database is loaded into memory.

In opt-in paged mode (`COGNIGRAPH_STORAGE_MODE=paged`), redb is primary for
document bodies: collection types and key sets remain resident, documents are
loaded through a bytes-bounded LRU, vectors use the mmap sidecar, and BM25 uses
generation-stamped Tantivy directories. Both modes implement the same
`GraphBackend` contract.

Paged point reads validate the resident key/catalog state and hold its read
lock through any redb fetch and cache fill. Writes hold the exclusive lock
through durable commit and cache/sidecar publication. Collection drop clears
its cached documents and loaded sidecar before releasing that lock, preventing
stale fills and reads across drop/recreate. Cold vector-sidecar builds use
the same state boundary and serialize publication. See the
[CG-10/CG-14 verification](../issues/paged-cache-2026-09-08.md).

Why this shape first:

- The `GraphBackend` contract and shared conformance/corpus tests keep passing
  unchanged; persistence is an additive property, not a rewrite.
- Correctness is easy to reason about: one write transaction per backend
  operation, committed under the same write lock that guards memory, so disk
  and memory can never reorder relative to each other.
- redb is a pure-Rust embedded library (MIT/Apache, pinned) — a dependency in
  a different risk class from a third-party database server.

Accepted limitation (resident mode, the default): **the dataset must fit in RAM.** Paged mode lifts this to a keys-resident ceiling. That is consistent with
the product's current scale target and is revisited in "Migration path" below.

## redb schema (version 2)

Two data tables plus metadata and identity:

| Table | Key | Value |
|-------|-----|-------|
| `meta` | `&str` (`"schema_version"`, `"data_generation"`) | `u32` — schema `2`, numeric mutation generation |
| `identity` | `&str` (`"database_id"`, `"data_revision"`) | UUID strings — immutable database identity and unique token for the latest commit |
| `collections` | `&str` collection name | `u8` — `0` = Document, `1` = Edge |
| `documents` | `&str` composite `"{collection}\u{0}{key}"` | `&str` — the document as compact JSON |

Notes:

- One `documents` table with a composite key (NUL separator) instead of one
  redb table per collection. Prefix ranges (`"{name}\u{0}"` ..
  `"{name}\u{1}"`) give per-collection iteration in key order, which matches
  the in-memory `BTreeMap` ordering exactly.
- Consequently, collection names and document keys **must not contain
  `\u{0}`**. Writes reject offending names with a validation error.
- Edges are stored identically to documents (they are JSON documents with
  `_from`/`_to`); the collection's type lives in `collections`.
- Durability: redb's default commit durability (fsync per commit). One
  backend write operation = one transaction.
- Every successful mutation updates `data_revision` in that same transaction.
  Failed mutations leave it unchanged. Unlike a numeric generation, this UUID
  does not repeat when a backup is restored and different writes follow.
- Persistent derivative filenames include the database identity in their
  hashed input. Warm text/vector loading additionally validates the exact
  committed revision. Reopening an unchanged database preserves both IDs.

### Collection ensure idempotency (CG-35)

`ensure_collection` checks the catalog under the same exclusive state lock as
creation. An existing collection is a true no-op: its original type, documents,
process write version, durable generation, and revision are preserved. As
before, requesting a different type does not convert an existing collection.
Concurrent ensures of a missing name commit only one creation; a real creation
still advances the version and revision before publishing its catalog entry.

This allows authenticated startup to ensure its existing control collections
without invalidating current vector files and Tantivy indexes. Ordinary writes
and real collection creation still invalidate derivatives. Vector warm loading
still scans redb to reconstruct key/model metadata before checking and opening
the existing mmap file; this fix avoids its unnecessary rewrite, not that scan.
See [CG-35 verification](../issues/collection-ensure-2026-09-09.md) for type/version
tests, concurrent creation, and authenticated release-server restart evidence.

## Configuration

- `COGNIGRAPH_NATIVE_PATH=/path/to/data.redb` — the server opens the native
  backend persistently at this path (file is created if absent).
- Unset → pure in-memory backend, exactly as before.

## Import / export

`NativeBackend` exposes JSON import/export independent of the storage engine:

```json
{
  "collections": {
    "documents": { "type": "document", "documents": { "a": { "...": "..." } } },
    "relationships": { "type": "edge", "documents": { "e1": { "...": "..." } } }
  }
}
```

- `export_json()` produces the snapshot above from memory.
- `import_json(value)` creates the collections and inserts the documents
  (write-through when persistent), overwriting existing keys.

JSON remains the portable migration path. Export omits physical database and
revision identity; import writes into the destination's own identity and
advances its revision.

CG-11 adds an atomic metadata-only exception: opening schema 1 creates the
identity records and changes the schema marker to 2 without rewriting user
rows or advancing their numeric generation. Existing derivatives rebuild
under the new identity. **Schema-1 binaries reject upgraded databases.**
Downgrade uses a pre-upgrade backup or imports a JSON export into a separate
database using the older binary. A schema-2 store with missing or malformed
identity records refuses to open instead of silently generating another identity.
See [the migration decision](../decisions/decision_native_derivative_identity.md).

## Migration path beyond v1

Planned evolution, in order, each behind the same `GraphBackend` contract:

0. **Upsert triple index (2026-07-03):** (from, to, relation_type) → edge
   key per edge collection; per-collection invalidation, self-maintained by
   upsert_edge so ingest loops keep it warm (see benchmarks.md). O(1)
   upsert lookup, ~345x on 10k edges.
1. ~~Persistent adjacency index~~ **Delivered as a cached in-memory
   adjacency index** (generation-invalidated derivative; traversal is
   O(degree)). redb `edges_from/to` tables deferred to redb-primary reads,
   where they become load-bearing.
2. **redb-primary reads — delivered** (`COGNIGRAPH_STORAGE_MODE=paged`):
   RAM holds key sets + a bytes-bounded LRU cache; documents page in from
   redb via the ordered composite-key range scans. Requires sidecar
   vectors. The migration path is complete.
3. **Vector storage** as fixed-width int8 arrays in a dedicated sidecar
   (the in-memory quantized index from the Phase 10 work is exactly this
   shape), memory-mapped for zero-copy scanning — this is also where the
   4× memory saving and mmap land together.
4. **Full-text (tantivy)** — delivered: persistent generation-stamped index
   directories beside the redb file, warm-started on reopen.

Derivative filenames use SHA-256 identities under the database directory;
collection names are opaque data and never become path components. Text-index
identity encodes the collection and ordered field list as a JSON tuple. A warm
open requires matching identity, exact Tantivy schema, and data generation.
Vector sidecars use the same safe filename helper. Previously named derivatives
are left untouched and rebuilt under the new names on demand; this can temporarily
increase disk usage and make the first search slower. The redb truth is unchanged.
Native text search rejects duplicate field names and the reserved `_key` field
with a validation error; the HTTP text and hybrid endpoints return 400.
