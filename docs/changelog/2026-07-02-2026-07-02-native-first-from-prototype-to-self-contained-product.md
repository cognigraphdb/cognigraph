# 2026-07-02 — Native-First: from prototype to self-contained product

- Date: 2026-07-02
- Status: Historical
- Kind: History
- Date source: Original section heading
- Source: `CHANGELOG.md:2318-2376` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

One session, 21 commits. ArangoDB demoted to maintenance-mode reference;
CogniGraph now runs entirely on its own stack. 263 tests.

- **M1 — CGQL semantics hardening**: null semantics (missing paths → null,
  null-falsy filters), depth-0 traversal, positional parse errors, strict
  bind variables, keyword boundaries, non-associative comparisons, full
  JSON escapes.
- **M2 — Native-first server**: `GraphBackend::ping()` capability,
  `DocumentConflict` (409), shared conformance suite
  (`cognigraph_core::contract`), explicit hybrid-search fallback, O(E)
  traversal. Conformance verified against live ArangoDB; two real Arango
  bugs found and fixed (vector_search projection + dedup collapse).
- **M3 — Native persistence**: redb write-through storage (memory-primary,
  commit-before-mutate), `COGNIGRAPH_NATIVE_PATH`, JSON export/import as
  the migration path, durability tests. Model: docs/native-storage-model.md.
- **M4 — CGQL v2**: 22-function registry with validation-time arity checks,
  LIMIT pushdown, `text_search` capability + native exact BM25, hybrid
  search on native, **native becomes the default backend**, file-driven
  dual-engine test corpus (`crates/cognigraph-query/tests/corpus/`).
- **Multilingual coverage**: German/Russian/Spanish/Hebrew/mixed-script
  across corpus, BM25, persistence; Unicode decisions (collation, NFC)
  documented as tracked open items.
- **M5 — CGQL ergonomics**: LET, multi-key SORT, RETURN DISTINCT,
  null-falsy booleans, date functions; numeric int/float equality bug fixed.
- **M6 — COLLECT family**: grouping, AGGREGATE (SUM/MIN/MAX/AVG),
  WITH COUNT INTO, INTO group capture.
- **M7 — Mutations**: INSERT / UPDATE (merge) / REPLACE / REMOVE with
  NEW/OLD, FOR-driven bulk, QueryMode read/write split, gated POST /query.
  Design record: docs/cgql-mutations-design.md.
- **M8 — Auth & RBAC**: cognigraph-auth crate (argon2, hashed cg_ tokens,
  users/tokens stored through GraphBackend), four roles with scope matrix,
  bearer middleware, /users API, RBAC-gated Lua write mode.
- **M9 — Production hardening**: per-IP rate limiting, /metrics (Prometheus
  text), request timeouts, JSON logs, graceful shutdown, Docker image.
- **Performance (benchmarks first)**: dependency-free bench harness;
  **vector_search 47× faster** (15.95 ms → 0.34 ms: reference scoring + rayon + int8 two-stage with exact re-rank), **CGQL scans 2.7–3.1× faster** (projection pushdown via `list_documents_projected`), **BM25 159× via tantivy** (in-RAM index, lazily rebuilt); quantized recall@10 measured at 100% with 10/10 hybrid agreement; **traversal 49×** (cached adjacency index); persistent tantivy directories warm-start across restarts;
  results and rejected approaches in docs/benchmarks.md.
- **Embedding providers**: Google Gemini support (`EMBEDDING_PROVIDER=gemini`,
  `gemini-embedding-2` default, Matryoshka `outputDimensionality` supported);
  live env-gated provider tests over the SOTU corpus (OpenAI verified live,
  Gemini pending a key).
- **M10 — Vector sidecar + mmap**: embeddings move to a memory-mapped
  int8 sidecar file (`COGNIGRAPH_VECTOR_MODE=sidecar`) — search-only
  embeddings, redb truth, exact re-rank scores, 7.3× vector RAM
  reduction at 0.85 ms search; design record in
  docs/vector-sidecar-design.md. Incremental delta writes (v2): steady
  writes merge into searches without file rebuilds; base rebuilds only
  past a 10% delta threshold.
- **M11 — redb-primary reads** (`COGNIGRAPH_STORAGE_MODE=paged`): documents
  page in from redb through a bytes-bounded LRU; RAM keeps only key sets.
  Composes with sidecar vectors and persistent tantivy; the shared
  conformance suite passes identically in both storage modes. The
  storage-model migration path is complete.
- **Test data**: classic State of the Union corpus (fixtures/sotu.txt,
  public domain) with an end-to-end ingest/BM25/vector/CGQL suite over
  real prose (`crates/cognigraph-native/tests/sotu.rs`).
