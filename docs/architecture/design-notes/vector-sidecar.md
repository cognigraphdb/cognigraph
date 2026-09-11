# Vector Sidecar + mmap — Design Proposal (for discussion)

Status: ACCEPTED & IMPLEMENTED 2026-07-02 (all five recommendations). v2 adds incremental writes: an in-memory delta (quantized upserts/deletes) shadows base slots and merges into searches; the immutable base file rebuilds only when the delta exceeds max(10% of base, 64) entries. The delta needs no persistence — like the file itself, it is rebuilt from redb truth. This replaces the originally sketched in-file tombstones: the base stays immutable, so there are no torn-read or in-place-write concerns. Follow-up to the Phase 10 int8 work; this is
storage-model migration step 3 (docs/native-storage-model.md). The proposal
below is retained as the decision input; the status paragraph
above records the implemented design, including the later immutable-base plus
in-memory-delta refinement.

## Goal

Move embeddings out of the JSON documents into a dedicated int8 sidecar
file that is memory-mapped for zero-copy scanning. This is where the two
deferred wins land together:

- **4× memory**: today the JSON documents carry f64 arrays *and* the
  quantized index duplicates them as i8. In sidecar mode, RAM holds only
  the i8 sidecar mapping (and the OS page cache manages residency).
- **Real mmap**: stage-1 scanning reads i8 slices directly from the
  mapped file — no deserialization, no copies, warm-start without
  rebuilding the quantized index.
- **Scale**: datasets where vectors dominate memory (e.g. 1M × 768-dim
  ≈ 6 GB f64 → 768 MB i8, paged on demand) stop being RAM-bound.

## Proposed design

### The sidecar is a rebuildable derivative, not primary data

The redb `documents` table (full JSON, f64 embeddings) remains the single
source of truth. Each collection sidecar (`{path}.{sha256(identity)}.vectors`,
where identity is the JSON tuple `[database_uuid, collection]`)
is a derived structure:

- No crash-consistency machinery: on open, the header generation and commit
  revision UUID are compared against redb metadata; any mismatch (crash mid-write,
  deleted file, version bump) triggers a full rebuild from redb. Rebuild
  is one linear pass — the same work the in-memory quantized index does
  today at first query.
- In implemented v2, writes update the in-memory delta and its applied
  generation; deletes add a delta tombstone. The mmap base remains immutable
  and is rebuilt after the delta exceeds `max(64, base_keys / 10)` entries.
- Commit and delta publication hold the backend state write lock. Snapshot,
  build, and publication of a sidecar hold the corresponding read lock; cold
  builders serialize and recheck after waiting. Collection drop removes the
  loaded sidecar under the write lock. These CG-10/CG-14 repairs prevent a
  delayed writer or builder from publishing an obsolete vector view as
  current; see [verification](../../issues/paged-cache-2026-09-08.md).

### File layout (fixed-width slots)

```
header:  8-byte magic "CGVEC2\0\0" | u32 generation | u32 dim | u32 slot_count | 16-byte revision UUID
slot i:  f32 scale | f32 norm | i8[dim]
```

- Slot order follows the sorted redb keys that contain vectors; the exact
  revision binds that key list to the file. There is no `vector_slots` table
  or on-disk tombstone flag; deletions live in the in-memory delta.
- Fixed dimension per base file, checked during build and delta application.
- One sidecar file per collection with vectors
  (`{path}.{sha256(identity)}.vectors`). Database UUID and collection are
  hashed as structured data so separators cannot escape the database directory.
  Earlier raw-name or collection-only names are left untouched during ordinary
  upgrade and rebuilt under the new identity on demand. Tenant retirement
  quarantines those legacy files too. `CGVEC1` files cannot warm-open as `CGVEC2`.
- mmap via the `memmap2` crate (pure-Rust wrapper, MIT/Apache — same
  dependency class as redb).

### Search path

Identical two-stage shape as today: stage 1 scans the mapped i8 slots
(parallel integer dot products, 4× oversampling), stage 2 re-ranks
exactly. Exactness of returned scores requires the f64 vector — fetched
per candidate from the in-memory document (mode A below) or from redb
(mode B). Candidate counts are small (≤ 4× limit), so a redb point-read
per candidate is acceptable.

## Open decisions

1. **Embedding visibility (the breaking one).** In sidecar mode, is the
   embedding still part of the document that `get_document` /
   `RETURN d` returns?
   - **A — keep embeddings in the in-memory documents** (unchanged
     behavior): no RAM saving, sidecar buys warm-start + mmap scan only.
   - **B — search-only embeddings (recommended)**: in-memory documents
     drop the `embedding` field; it lives in redb (truth) + sidecar
     (scan). `RETURN d` no longer includes `embedding`; a projected read
     can fetch it explicitly from redb on demand. This is the option that
     actually delivers the 4× memory goal. Breaking, opt-in via mode.
   - Mode selection: `COGNIGRAPH_VECTOR_MODE=embedded` (default, today's
     behavior) | `sidecar`.
2. **Scope**: sidecar mode only for persistent backends (in-memory mode
   keeps today's quantized index)? Recommended: yes — a sidecar without a
   redb source of truth has nothing to rebuild from.
3. **Dependency**: `memmap2` acceptable? (The alternative is raw libc
   mmap — more code, no practical benefit.)
4. **Exact re-rank source in sidecar mode**: redb point-reads per
   candidate (recommended; ≤ 4× limit reads) vs storing f32 copies in the
   sidecar (larger file, no redb reads, scores exact only to f32).
5. **Naming/config**: one `.vectors` file per collection as proposed, or
   a single multi-collection file with a collection table in the header?
   (Per-collection recommended: drop/compact independently.)
