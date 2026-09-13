# Decision: Memory-primary, redb-durable; every index is a rebuildable derivative

Date: 2026-07-02 (M3, extended M10) · Status: ACCEPTED

## Context
Persistence, then vector and text indexes, each threatened to import
crash-consistency machinery and invariant maintenance across every write path.

## Decision
redb holds the single source of truth (full JSON documents), committed
*before* memory mutates, under the same lock. Everything else — quantized
vector index, mmap sidecar file, sidecar delta, tantivy text index — is a
derivative: stamped with a generation, validated lazily, and rebuilt from
truth on any mismatch. Derivatives never need their own durability.

## Outcome
Zero crash-consistency code outside redb itself. The sidecar survives crashes
by definition (mismatch → rebuild); the incremental delta needed no
persistence; tantivy adoption took ~150 lines because invalidation already
existed. The accepted cost — datasets must fit in RAM except vectors — is
documented in docs/native-storage-model.md with the redb-primary escape path.
