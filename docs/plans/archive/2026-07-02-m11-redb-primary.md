# M11 — redb-primary reads (paged mode). Accepted: all five recommendations.

- [x] StorageMode::{Resident,Paged}; open_with_modes; paged enforces sidecar at startup.
- [x] Paged RAM = key sets (BTreeSet per collection, loaded without JSON parse) + bytes-bounded LRU doc cache (COGNIGRAPH_CACHE_BYTES, 256MB default).
- [x] fetch_document (cache→redb, embedding stripped); list/projected via redb range scans with limit/offset mid-scan; get_edges via store scan; traversal adjacency + text index build from store scans (data_generation-stamped); sidecar stage-2 + export/import paged-aware.
- [x] Server: COGNIGRAPH_STORAGE_MODE + COGNIGRAPH_CACHE_BYTES.
- [x] Tests: contract::run_all on paged backend; paged lifecycle (CRUD, CGQL, BM25, vector, reopen); cache-bound eviction. Bench: paged point-read + cold scan rows.
- [x] Docs (storage-model v2 status, benchmarks, README, decision outcome), gates, push.
