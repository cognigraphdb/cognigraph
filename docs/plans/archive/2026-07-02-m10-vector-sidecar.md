# M10 — Vector Sidecar + mmap (accepted design, all recommendations)

> Executed inline 2026-07-02. Design: docs/vector-sidecar-design.md (ACCEPTED).

Decisions: search-only embeddings behind COGNIGRAPH_VECTOR_MODE=sidecar (default embedded, unchanged); persistent backends only; memmap2; exact re-rank via redb point-reads; one sidecar file per collection.

- [ ] redb: persisted `data_generation` in meta (bumped every apply), `get_document_raw`, per-collection scan.
- [ ] Sidecar file `{path}.{collection}.vectors`: header (magic CGVEC1, generation, dim, count) + fixed slots (f32 scale, f32 norm, i8[dim]); slot order = BTreeMap key order (generation match guarantees reproducibility — keys never stored). v1 writes = full rebuild on generation mismatch (lazy, at search), not incremental slots/tombstones; incremental appends are a future optimization if write-heavy workloads demand.
- [ ] Sidecar mode semantics: docs stripped of `embedding` in memory (redb keeps truth); update/replace merge against redb truth; stage-1 scans mmap'd i8 slots (rayon), stage-2 exact re-rank via redb point-reads.
- [ ] Server: COGNIGRAPH_VECTOR_MODE env (sidecar requires COGNIGRAPH_NATIVE_PATH).
- [ ] Tests: get_document lacks embedding; brute-force equivalence in sidecar mode; update preserves vectors; reopen warm-start; write invalidation. Bench: sidecar latency + measured bytes (f64-in-RAM eliminated vs sidecar file size).
- [ ] Docs (storage model v1.1, README env), gates, push.
