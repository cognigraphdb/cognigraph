# redb-Primary Reads — Design Proposal (for discussion)

Status: ACCEPTED & IMPLEMENTED 2026-07-02 (all five recommendations). Delivered as designed: keys + bytes-bounded LRU resident, redb range scans with mid-scan limit/offset, sidecar coupling enforced at startup, full conformance suite green in paged mode.

## Context

The memory-primary model (decision_rebuildable_derivatives.md) requires the
dataset to fit in RAM. Vectors escaped that ceiling in M10 (sidecar); text
indexes live on disk (tantivy). Documents themselves are the last resident
class. This step makes document residency optional, letting datasets outgrow
RAM entirely.

## Proposed design — the "resident-set" model

New mode: `COGNIGRAPH_STORAGE_MODE = resident` (today's behavior, default)
| `paged`. In paged mode, RAM holds only:

1. **Collection types and key sets** (a `BTreeSet<String>` per collection).
   Keys stay fully resident — this preserves O(1) existence/conflict checks,
   key-ordered iteration, and adjacency/sidecar/tantivy index builds without
   scans, at ~few dozen bytes per document (fine to ~100M docs).
2. **A bytes-bounded LRU document cache** (`COGNIGRAPH_CACHE_BYTES`,
   proposed default 256 MB): `get_document` hits the cache, misses read
   redb; writes write through and upsert the cache.

Read-path changes:
- `list_documents(_projected)`: redb range scans over the existing ordered
  composite keys, with limit/offset applied during the scan — no full
  materialization. CGQL collection scans stream the same way.
- **Traversal**: the adjacency index (keys-only, already lazy) is unchanged;
  vertex documents resolve through the LRU cache.
- **Vector search**: paged mode composes with (and requires) sidecar mode —
  mmap stage-1, redb stage-2, unchanged.
- **Text search**: persistent tantivy directories, unchanged.
- Writes: identical write-through semantics; the truth was always redb.

Expected and accepted costs (to be measured and published per the
benchmarks-first rule): CGQL full-collection scans in paged mode pay JSON
parse per document (redb → serde) — likely returning scans to roughly
pre-projection-pushdown latencies for cold data. Point reads stay fast via
the cache. The GraphBackend contract and every conformance/corpus test must
pass identically in both modes.

## Cache concurrency — 2026-09-08

CG-10 and CG-14 repair gaps in the original write-through implementation.
The backend state lock now covers durable commit through document-cache and
vector-delta publication for paged update, replacement, and deletion. Point
reads hold its shared side through key/catalog validation, the redb read, and
cache insertion. A delayed fill therefore completes before a waiting write
or drop, and cannot repopulate the cache afterward.

Collection drop purges that collection's LRU entries, recency records, byte
accounting, and loaded vector sidecar under the same exclusive state lock.
Recreating the collection starts with no retained document-cache entries.
This catalog check plus ordered purge is the cache-validity boundary within
one backend instance; cross-database/tenant incarnation binding remains CG-11
and CG-12 work.

Vector-sidecar snapshot/build/publication also holds the shared state lock.
Cold builders serialize and recheck the loaded sidecar after waiting, so
they cannot overwrite newer deltas or collide on a temporary file. Search
releases the build lock before resolving candidates through point reads.

These sections trade some write concurrency for correctness: a cold point
read or sidecar build can delay a writer. No concurrency throughput claim is
made. See [verification and scope](../../issues/paged-cache-2026-09-08.md).

## Original design decisions

1. **Default**: `resident` stays the default; `paged` is opt-in — agreed?
2. **Cache bound**: bytes-based LRU (`COGNIGRAPH_CACHE_BYTES`, 256 MB
   default) rather than entry-count — agreed?
3. **Composition**: paged mode *requires* `COGNIGRAPH_VECTOR_MODE=sidecar`
   (embedded f64 vectors in a paged backend would defeat the purpose) —
   enforce at startup?
4. **Scan regression**: accept the measured cold-scan cost in paged mode
   (published in benchmarks.md) rather than keeping any collection
   force-resident? A per-collection pin list could come later if real
   workloads demand it.
5. **Keys-resident invariant**: full key sets stay in RAM as the new
   (much higher) ceiling — acceptable, or do you want key paging designed
   in from the start (significantly more complexity)?
