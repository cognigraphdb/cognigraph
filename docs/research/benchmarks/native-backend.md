# Native Backend Benchmarks

Run with `cargo bench -p cognigraph-native` — a dependency-free harness
(`crates/cognigraph-native/benches/perf.rs`) over a deterministic dataset:
10,000 documents, 128-dim embeddings, 9,999 edges. Median of 30 iterations,
Apple Silicon, 2026-07-02.

| Operation | Baseline | Final | Total |
|---|---|---|---|
| vector_search top-10 (10k × 128) | 15.95 ms | **0.34 ms** | **47×** (ref-scoring + rayon + int8 two-stage) |
| text_search BM25 (10k docs) | 14.95 ms | **0.09 ms** | **159×** (tantivy in-RAM index, lazily rebuilt) |
| vector_search, sidecar mode (mmap) | — | **0.85 ms** | 18.7× vs baseline; **RAM 10.2 MB → 1.4 MB paged file (7.3×)** |
| traverse depth 1..3 | 0.79 ms | **0.016 ms** | **49×** (cached adjacency index) |
| CGQL filter+sort+limit (10k scan) | 20.94 ms | **7.85 ms** | **2.7×** (projection pushdown) |
| CGQL COLLECT aggregate (10k rows) | 24.59 ms | **8.07 ms** | **3.1×** (projection pushdown) |

The table above is the original native optimization arc (2026-07-02). CGQL
has moved much further since (filter/IN/threshold pushdown, EXPLAIN, v2
pipeline, interpreter clone fixes, COLLECT hashing) — see the snapshot
below for current authoritative numbers, and the dated delta sections for
how each landed.

## Current numbers — full-suite re-run (2026-07-06, post-hygiene-arc)

All four harnesses re-run from a clean tree after H1–H8 landed; every
figure within noise of its recorded baseline, no regressions. Apple
Silicon. Gates at time of run: **337 tests green** across 45 suites,
clippy `-D warnings` clean, fmt clean, CI green.

**Native engine** — `cargo bench -p cognigraph-native` (10k docs, dim 128,
9,999 edges; median of 30):

| Operation | median | p90 |
|---|---|---|
| vector_search top-10 | 0.374 ms | 0.444 ms |
| text_search BM25 | 0.095 ms | 0.100 ms |
| traverse depth 1..3 | 0.019 ms | 0.019 ms |
| upsert_edge update (10k edges) | 0.001 ms | 0.001 ms |
| CGQL filter+sort+limit (10k scan) | 0.870 ms | 0.975 ms |
| CGQL IN filter pushdown | 1.788 ms | 1.942 ms |
| CGQL filter+limit full pushdown | 0.020 ms | 0.022 ms |
| CGQL COLLECT aggregate | 4.553 ms | 4.705 ms |
| CGQL COLLECT high cardinality | 5.138 ms | 5.286 ms |
| CGQL RETURN DISTINCT | 3.643 ms | 3.758 ms |
| vector_search sidecar mmap | 1.059 ms | 1.217 ms |
| vector exact brute force (reference) | 0.952 ms | 1.069 ms |
| hybrid quantized+tantivy end-to-end | 0.464 ms | 0.585 ms |

Quality invariants: **quantized recall@10 vs exact = 100.00%** (20 queries);
**hybrid RRF agreement 10/10** (quantized+tantivy vs exact reference).
Memory: f64-in-RAM 10.2 MB → 1.4 MB paged int8 sidecar (7.3×).

The flagship CGQL scan holds at **0.87 ms** (8.54 ms before the July
optimization arc: filter pushdown → v2 restructure → interpreter
allocation discipline — ~10× cumulative, all corpus-guarded).

**Grounding** — `cargo run --release -p cognigraph-construct --example
grounding_bench` (4,000 chunks, 6 relation rules; best of 5):

| vetoes | per chunk | total |
|---|---|---|
| 0 | 6.3 µs | 25.3 ms / 4000 |
| 8 | 6.7 µs | 26.7 ms |
| 64 | 18.6 µs | 74.5 ms |

**Server under concurrent load** — `cargo bench -p cognigraph-server --bench
load` (real server binary, 5k docs dim 64, localhost TCP, 2 s cells; the
bench now sweeps BOTH backend modes plus pure-write and batch workloads):

In-memory:

| workload | c=1 | c=8 | c=32 | c=128 | p99 @128 |
|---|---|---|---|---|---|
| GET point read | 24.6k | 95.0k | 132.7k | **138.3k** | 1.64 ms |
| POST search/query (CGQL) | 17.1k | 81.2k | 120.6k | **131.7k** | 2.24 ms |
| POST search/vector | 2.2k | 12.2k | 14.4k | 14.1k | 24.6 ms |
| POST insert (pure write) | 22.3k | 96.3k | 132.5k | **136.5k** | 1.38 ms |
| mixed 90% read / 10% write | 23.5k | 99.5k | 136.9k | **139.6k** | 1.18 ms |

Persistent (redb write-through) — reads indistinguishable from in-memory
(point read 140.2k @ c=128); writes at the fsync ceiling as recorded in
the H4 section:

| workload (persistent) | c=1 | c=128 | p99 @128 |
|---|---|---|---|
| POST insert (pure write) | 214/s | 239/s | 1293 ms |
| POST batch (100 inserts) | 131/s (13.1k docs/s) | 113/s | 2295 ms |
| mixed 90% read / 10% write | 1.9k | 2.4k | 104 ms |

The read path scales cleanly to 128 clients with p99 < 3 ms; vector
search stays CPU-bound at its ~14k plateau. One first-pass anomaly was
chased down rather than recorded: persistent mixed @ c=128 read 1.3k on
the first run and **2.44k on the verification re-run** — the cell stacks
10% writes on the fsync ceiling inside a 2 s window, making it the
suite's noisiest; treat single readings of it with suspicion.

**Persistent query cache** — `cargo bench -p cognigraph-cache --bench
persistent` (dim 1536, 500 entries): put 4.51 ms (fsync after a provider
call), memory hit 1.4 µs, cold store read-through 14.3 µs — as recorded
in the H7 section.

## What changed

- **vector_search**: scores documents by reference over the raw JSON arrays
  (no per-document `Vec<f64>` allocation) and clones only the top-k
  documents that survive sort + truncate. 9.5× on this workload.
- **rayon parallel scoring**: vector scoring and BM25 tokenization fan out
  across cores (`par_iter` over document refs; result order is preserved,
  so determinism and tie-breaks are unchanged). Vector search gains a
  further 2.5×; BM25 gains 1.4× — sublinear because per-token `String`
  allocation contends on the allocator, reinforcing that the indexed
  implementation is the real full-text win.
- **text_search**: rewritten to a single tokenization pass counting all
  query terms simultaneously (previously df and tf each rescanned the token
  lists per term). Measured neutral at 3 query terms / short documents —
  profiling shows BM25 here is bound by per-token `String` allocation, not
  scanning — but it removes the per-term asymptotics for long queries.
  A per-doc HashMap variant was tried and **rejected**: it measured slower
  (16.6 ms) than the baseline on this workload.

- **CGQL projection pushdown**: the executor computes exactly which
  top-level attributes a collection scan references; the backend's
  `list_documents_projected` capability (default = full documents, so
  every backend stays correct) lets the native backend clone only those
  fields — skipping the embedding arrays that dominated scan cost. Any
  whole-document use (`RETURN d`, `HAS(d, …)`, bare `INTO`) falls back
  automatically; the dual-engine corpus covers both paths.

- **int8 two-stage vector search**: a lazily built quantized index
  (invalidated by a global write version) scans i8 vectors with integer
  dot products, oversamples candidates 4× (min 32), then re-ranks with
  exact f64 cosine — **returned scores and thresholds are always exact**,
  verified against brute force on 300 pseudo-random vectors. Another 2×
  on top of rayon. Note: this buys speed, not memory — the JSON documents
  still hold the f64 arrays; the 4× memory saving arrives when vectors
  move to a dedicated store (see below).

## Quality matrix (quantization + hybrid)

Measured in the same bench run (20 pseudo-random queries, top-10):

| Configuration | Latency | Quality vs exact |
|---|---|---|
| exact brute-force vector scan (reference) | 0.96 ms | — |
| quantized two-stage (embedded) | 0.38 ms | **recall@10 = 100.00%** |
| quantized sidecar (mmap) | 1.08 ms | same two-stage exactness |
| hybrid: exact vector + tantivy BM25 | — | reference fusion |
| hybrid: quantized vector + tantivy BM25 | 0.50 ms end-to-end | **agreement 10/10** |

The 4× candidate oversampling makes the quantized stage lossless in
practice on this workload: identical top-10 sets, identical hybrid
fusion, at 2.5× the speed of the exact scan.

## Honest limits and next steps

- BM25 is tantivy-backed with a **persistent, generation-stamped index
  directory** on persistent backends ({db}.{collection}.{fields-hash}.tantivy):
  warm starts reopen the directory instantly; any data change rebuilds it —
  the same rebuildable-derivative rule as the vector sidecar.
- Traversal uses a cached per-edge-collection adjacency index (vertex →
  edge keys, generation-invalidated): O(degree) per hop. The originally
  sketched redb `edges_from/to` tables are deferred with the same reasoning
  as mmap once was: while the backend is memory-primary they buy nothing —
  they belong with redb-primary reads (storage-model step 2).
- **Sidecar mode (M10)** delivers the mmap + memory goals: embeddings
  are search-only (stripped from in-memory documents, redb keeps the
  truth), stage-1 scans a memory-mapped int8 file zero-copy, stage-2
  re-ranks exactly via redb point-reads. The ~0.5 ms premium over the
  in-RAM quantized index is those truth reads — the price of freeing
  the vector RAM. Incremental writes land in an in-memory delta that
  merges into searches (no file rebuild per write; the immutable base
  rebuilds only past a 10% delta threshold, counted and test-asserted
  via `sidecar_rebuild_count()`). `COGNIGRAPH_VECTOR_MODE=sidecar` (persistent
  backends only; default `embedded` is unchanged).

## Grounding: relation_blocker veto cost + shared casefold (2026-07-03)

Corpus: CrowdStrike article ×100 (4000 chunks), 6 relation rules, synthetic
non-matching vetoes (worst case: every veto scanned, none short-circuits).
Best of 5, release build. `cargo run --release -p cognigraph-construct
--example grounding_bench`.

| vetoes | before (µs/chunk) | after shared casefold | delta |
|---|---|---|---|
| 0 | 9.5 | **5.8** | 1.64x |
| 8 | 9.9 | **6.2** | 1.60x |
| 64 | 21.7 | **17.1** | 1.27x |

- Veto checking itself is cheap at realistic counts: +0.4 µs/chunk (+7%) at
  8 vetoes per space.
- The optimization: `ground_chunk` now casefolds the chunk text ONCE and
  shares it across all rule triggers and veto phrases (`affirms_phrase_cf`);
  previously every trigger check re-lowercased the full chunk text.
- Known scaling limit (not optimized — no workload justifies it yet): veto
  phrase matching is a linear scan per fired triple, so hundreds of vetoes
  per space would want multi-pattern matching (Aho-Corasick-style). Recorded
  in decision_relation_blocker.md.

Quality matrix (deterministic acceptance suite, tests/blockers.rs):

| configuration | forbidden fired | recall |
|---|---|---|
| base config (allegation wording) | 1/1 | 1/1 |
| + proposed blocker | 1/1 (inert) | 1/1 |
| + accepted blocker | **0/1** | **1/1 held** |
| + accepted blocker, clean chunk added | 1/1 (chunk-local by design) | 1/1 |

## CGQL filter pushdown + EXPLAIN (2026-07-03)

`cargo bench -p cognigraph-native` (10k docs, dim 128):

| case | before | after | delta |
|---|---|---|---|
| CGQL filter+sort+limit (10k scan) | 8.54 ms | **2.01 ms** | 4.2x |
| CGQL filter+limit full pushdown (new) | ~8.5 ms class | **0.025 ms** | ~340x |
| CGQL COLLECT aggregate (10k rows) | 8.21 ms | 7.75 ms | ~1x (no filter to push) |

- Filter pushdown: AND-conjuncts of `var.path <op> literal/@bind` run
  by-reference in the backend scan (`list_documents_filtered`), so losers are
  never cloned; the engine re-applies filters afterwards as a correctness
  belt (measured cost: nil — it only sees survivors).
- With all filters pushed and no SORT/COLLECT, LIMIT pushes too: the scan
  stops at `offset+count` matches — that is the 340x case.
- Semantics guarded three ways: `FieldPredicate` unit tests, a new
  `filtered_scan_contract` in the backend conformance suite, and 7 new
  dual-engine corpus cases (numeric value equality, null/missing, mixed-type
  ordering, flipped operands, bind + partial residual, filter+limit).
- Found and fixed in passing: LIMIT was previously pushed past COLLECT
  (grouping saw a truncated source). Pinned by corpus case
  `collect_limit_groups_not_source`.
- COLLECT remains the slowest CGQL path (linear group probing) — next
  candidate if grouping workloads appear.

## COLLECT / DISTINCT hash grouping (2026-07-03)

The linear group probe (`groups.iter_mut().find` with `values_equal`) was
O(rows x groups); DISTINCT had the same quadratic pattern. Replaced with
hash grouping over a canonical byte encoding that matches `values_equal`
exactly: numbers collapse to f64 bits (-0.0 folded, so int 2 and float 2.0
share a group and the FIRST-SEEN value shape wins), object keys sort,
containers length-prefix. First-seen group order preserved. The canonical
buffer is reused across rows — the first version allocated per row and
regressed the low-cardinality case ~10%; buffer reuse removed it (recorded
per the benchmarks-first rule).

`cargo bench -p cognigraph-native` (10k docs):

| case | before | after | delta |
|---|---|---|---|
| CGQL COLLECT high cardinality (10k distinct keys, new) | 210.7 ms | **6.5 ms** | 33x |
| CGQL RETURN DISTINCT (10k distinct, new) | 192.7 ms | **5.0 ms** | 39x |
| CGQL COLLECT aggregate (10k rows, 8 groups) | 7.6 ms | 7.7 ms | noise |

Semantics pinned by two new dual-engine corpus cases over a mixed int/float
collection (`collect_numeric_key_merge`, `distinct_numeric_value_merge`).
Remaining cost in the 8-group case is per-row expression evaluation and Env
handling, not grouping — a different (interpreter-level) battle.

## upsert_edge triple index (2026-07-03)

`upsert_edge` located the existing (from, to, relation_type) edge with a
full collection scan — O(edges) per call, and the paged path re-read every
edge from redb. Now a per-collection triple index maps the triple to its
edge key.

| case | before | after | delta |
|---|---|---|---|
| upsert_edge update (10k edges, new) | 0.345 ms | **0.001 ms** | ~345x |

Design note — invalidation is per-collection and NOT generation-stamped
like the other derivative indexes: upsert-heavy ingest (semantic-neurons
fact ingestion is one upsert per grounded fact) would invalidate its own
index every call under global write versioning. Instead `upsert_edge`
maintains the index across its own writes; any OTHER write touching the
collection (document update/replace/delete, create_edge, batch ops,
import, drop) drops the entry and the next upsert rebuilds it. Duplicate
triples keep the old first-in-scan-order winner (create_edge invalidates
rather than approximating that order). Staleness hazards are pinned by
`upsert_edge_triple_index_survives_out_of_band_writes` (triple retargeted
via the generic document API; out-of-band duplicate with a smaller key).

## Server under concurrent load (2026-07-03)

First concurrency measurement — everything before this row was
single-caller. `cargo bench -p cognigraph-server --bench load`: spawns the
REAL server binary (native in-memory backend; auth/cache/rate-limit off),
seeds 5k docs (dim 64) over HTTP, then drives each workload for 2s per
concurrency level over localhost TCP.

| workload | conc | req/s | p50 ms | p90 ms | p99 ms |
|---|---|---|---|---|---|
| GET point read | 1 | 22,520 | 0.043 | 0.048 | 0.069 |
| GET point read | 32 | 128,650 | 0.237 | 0.324 | 0.483 |
| GET point read | 128 | 135,121 | 0.917 | 1.217 | 1.805 |
| POST search/query (CGQL, pushdown) | 32 | 111,319 | 0.263 | 0.417 | 0.708 |
| POST search/query (CGQL, pushdown) | 128 | 121,655 | 0.953 | 1.645 | 2.665 |
| POST search/vector | 8 | 12,379 | 0.510 | 1.115 | 2.251 |
| POST search/vector | 32 | 13,284 | 2.077 | 3.655 | 10.379 |
| POST search/vector | 128 | 13,105 | 8.898 | 17.135 | 26.711 |
| mixed 90% read / 10% write | 32 | 133,005 | 0.235 | 0.308 | 0.403 |
| mixed 90% read / 10% write | 128 | 134,110 | 0.940 | 1.078 | 1.413 |

Readings:

- **Read path scales.** Point reads and pushdown CGQL both clear 120k+
  req/s with p99 under 3 ms at 128 concurrent clients; the RwLock read
  path is not a bottleneck.
- **The feared write contention is a non-issue at 90/10**: the mixed
  workload is statistically identical to pure reads — in-memory writes
  hold the write lock too briefly to matter.
- **Vector search plateaus ~13k req/s at c=8 and latency grows with queue
  depth** (p99 26.7 ms at c=128). This is CPU saturation, not locking:
  one query already fans out across all cores via rayon, so concurrent
  queries time-share the machine. If tail latency under vector load ever
  matters, the lever is capping rayon parallelism per query (trade
  single-query latency for concurrent throughput headroom), not locks.

Honest limits: in-memory backend (no redb fsync in the write path — a
persistent-mode run would slow writes), client and server share one
machine (client CPU included in the ceiling), localhost TCP, 2 s cells.

## CGQL v2 pipeline restructure (2026-07-04)

The executor became an ordered operator pipeline (multiple FOR, positional
semantics, subqueries — decision_cgql_v2.md). Benchmarks-first caught a
20-35% regression in the first version: materialized documents were CLONED
into row environments, where v1 moved them. Fixed by single-pass site
consumption (a body that executes once takes ownership of its scan sites;
only correlated-subquery bodies, which re-run per row, clone).

| case | v1 | v2 first cut | v2 fixed |
|---|---|---|---|
| CGQL filter+sort+limit (10k) | 2.01 ms | 2.46 ms | **2.15 ms** |
| CGQL filter+limit full pushdown | 0.025 ms | 0.029 ms | **0.025 ms** |
| CGQL COLLECT aggregate (10k) | 7.75 ms | 9.93 ms | **7.97 ms** |
| CGQL COLLECT high cardinality | 6.46 ms | 8.33 ms | **6.62 ms** |
| CGQL RETURN DISTINCT (10k) | 4.98 ms | 6.77 ms | **5.05 ms** |

Post-fix numbers are within run-to-run noise of v1 while the pipeline now
supports joins, un-nesting, and subqueries.

## Interpreter performance, phase 1: allocation discipline (2026-07-04)

Profiling the evaluator found the cost was never the interpretive enum
walk — it was cloning:

1. `eval_identifier` cloned the ENTIRE root value (the whole document) on
   every identifier access, then progressively cloned subtrees while
   walking the path. Now walks by reference and clones only the accessed
   leaf.
2. `SORT` applied its permutation by cloning the full row set
   (`rows.to_vec()` + per-slot clone). Now a Schwartzian sort by move —
   rows travel with their keys, zero row clones (sort_by is stable, so
   tie order is unchanged).

| case | arc start | after leaf-clone | after sort-by-move | total |
|---|---|---|---|---|
| CGQL filter+sort+limit (10k) | 2.15 ms | 1.55 | **0.85** | 2.5x |
| CGQL COLLECT aggregate (10k) | 7.97 ms | 4.41 | **4.39** | 1.8x |
| CGQL COLLECT high cardinality | 6.62 ms | 5.24 | **5.36** | 1.2x |
| CGQL RETURN DISTINCT (10k) | 5.05 ms | 3.49 | **3.62** | 1.4x |
| CGQL filter+limit full pushdown | 0.025 ms | 0.020 | **0.021** | 1.2x |

Semantics guarded by the dual-engine corpus (byte-identical throughout).

**Deferred with rationale — slot-based environments / expression
pre-compilation:** after the clone fixes, per-row interpreter overhead is
~0.4-0.5 µs and no longer dominated by name lookup; the remaining Env
HashMap cost concentrates in high-cardinality COLLECT result construction.
The slot rewrite touches every executor path (COLLECT scope swap, bare
INTO name capture, subquery frames) right after the v2 restructure — poor
risk/reward at current numbers. Revisit trigger: aggregate-heavy workloads
where COLLECT dominates (e.g. >50 ms at 100k rows) or a profile showing
Env handling above ~30%.

## A5 pushdown extensions (2026-07-05)

- IN-predicate pushdown: `d.category IN ["rust", "graph"]` over 10k docs
  with SORT+LIMIT runs at 1.74 ms (new bench case; membership evaluated
  by reference in the backend scan, matching CGQL value equality —
  contract- and corpus-covered on both engines).
- VECTOR_SEARCH threshold pushdown and depth-1..1 traversal
  min_confidence carry no dedicated bench rows (they reduce candidate
  sets; correctness pinned by dual-engine corpus cases). Semantics
  boundaries documented in cgql-v1.md: no model_name pushdown (scoping,
  not filtering), no deep-traversal confidence pushdown (prunes paths
  through weak edges).

## H4: persistent-mode load benchmark (2026-07-05)

The one unmeasured path: redb write-through cost under concurrent HTTP
load. `cargo bench -p cognigraph-server --bench load` now runs the whole
sweep twice — in-memory and `COGNIGRAPH_NATIVE_PATH`-backed — plus a new
pure-write workload in both modes so commit cost is not diluted inside
the mixed cell. Same harness (real binary, env-cleared, 5k docs dim 64,
2 s cells), Apple Silicon local SSD.

**Reads are untouched by persistence** — point read, CGQL, and vector
search all land within run-to-run noise of the in-memory numbers
(e.g. point read 138.9k req/s @ c=128 persistent vs 138.4k in-memory).
Memory-primary means the durable file is never on the read path.

**Writes pay the full durability price:**

| workload (persistent) | c=1 | c=8 | c=32 | c=128 | p99 @128 |
|---|---|---|---|---|---|
| seed 5k docs (64-way) | — | — | — | 244/s | — |
| POST insert (pure write) | 226/s | 255/s | 257/s | 260/s | 1263 ms |
| POST batch (100 inserts) | 138/s (13.8k docs/s) | 115/s | 108/s | 103/s | 2282 ms |
| mixed 90% read / 10% write | 2.0k | 2.4k | 2.4k | 2.4k | 112 ms |

In-memory reference @ c=128: pure write 137.3k req/s, batch 3.1k/s
(≈308k docs/s, lock+serde bound), mixed 140.1k.

**Honest reading.** The ~255 writes/s ceiling is flat across all
concurrency levels: each document write is one redb commit (~4 ms fsync,
p50 at c=1), and commits serialize — added concurrency only queues
(p50 414 ms at c=128). The mixed workload collapses to ~2.4k req/s by
Amdahl: 10% of traffic capped at ~255/s bounds the whole stream at ~2.5k.
This is the write-through contract doing exactly what it promises
(commit BEFORE memory mutates, no acknowledged-but-lost writes) and
paying list price for it.

**The mitigation already exists, measured: `/api/batch`.** `execute_batch`
persists the entire batch in ONE redb transaction, so 100-doc batches
sustain ~13.8k doc writes/s at c=1 — a measured ~54× over single-doc
writes (not the naive 100×: a 100-doc commit costs ~7 ms vs ~4 ms, and
concurrent batchers contend down to ~10.3k docs/s at c=128; ingest
pipelines should prefer few large batches over many concurrent ones).
The ceiling above is specifically for INDEPENDENT single-doc writes,
where each write buys its own durability point.

**Recorded trigger, not an optimization.** The remaining lever is
server-side group commit (coalesce concurrent single-doc writes into one
transaction, acknowledge after the shared fsync) — it would lift the
independent-writer ceiling by roughly the coalescing factor WITHOUT
weakening durability (ack still follows commit), but it changes write-path
concurrency structure and needs a design discussion, not a quiet patch.
Revisit when a real workload of independent writers (not batchable
ingest) needs more than ~250 writes/s on the persistent backend.

## H7: persistent cache backend (2026-07-06)

`cargo bench -p cognigraph-cache --bench persistent` — dim 1536
(text-embedding-3-small shape), 500 entries, Apple Silicon:

| operation | ms/op |
|---|---|
| put_embedding (Immediate commit, fsync) | 4.385 |
| get_embedding — memory hit | 0.0043 |
| get_embedding — cold store read-through | 0.0160 |

Reading: the put cost is the same ~4 ms redb fsync H4 measured, and it
only occurs immediately after a fresh provider embedding call (~hundreds
of ms) — never on the read path. A cold read-through at 16 µs is four
orders of magnitude cheaper than the provider round-trip it replaces,
which is the entire value of persisting this level. Search results are
not persisted (TTL-bounded; see decision_cache_backends.md).

## Construction loop at pilot scale (2026-07-07)

`cargo run --release -p cognigraph-construct --example pilot_dryrun`
(deterministic, no LLM; 10,000 chunks / 40 rules incl. sentence gates /
2,000 proposed neurons, in-memory backend):

| surface | result |
|---|---|
| ingest (grounding + entities + mentions + facts) | 175 ms — 57 chunks/ms, 4,500 fact edges |
| idempotent re-ingest | 137 ms |
| evaluate (30 expected + 30 forbidden, distinct) | < 1 ms — 30/30 recall, 0/30 violations (gates + negation hold at scale) |
| proposal queue: store 2,000 | 5 ms |
| proposal queue: paginate 2,000 in pages of 500 | 2 ms |

The loop's deterministic surfaces carry an order of magnitude of
headroom at pilot scale; the LLM-bound stages (propose/review) are the
budget, which is why `/api/construct/review` drains in resumable `limit`
slices with per-call `judge_calls`/`elapsed_ms` metrics. Persistent
(redb) ingest at this volume goes through the batch import path
(~13k docs/s), not per-document fsync.

## D2: correlated traversal (2026-07-22)

A traversal may now start from the enclosing row. Measured on the
structured-CRM evaluation graph (254,076 documents; `persons` 26,870,
`contract_party` edges), through the HTTP API, best of 3, warm.

The comparison that matters is against the rewrite users had to write before
D2 — a join on `_to`, which D1a/D1b made fast. Both forms return the same
14,253 rows.

| query, 26,870-person outer side | release | (debug) |
|---|---|---|
| bare scan + `COLLECT` (no traversal at all) | 14 ms | 54 ms |
| join on `_to`, **edges only** | 37 ms | 137 ms |
| join on `_to` + `DOCUMENT` — the equivalent work | 156 ms | 489 ms |
| **correlated traversal, no path variable named** | **320 ms** | 957 ms |
| correlated traversal, path variable named | 499 ms | 1,583 ms |

> **Correction, same day.** The first version of this table quoted the debug
> column as if it were the number. The dev server runs `cargo run` without
> `--release`, and an unoptimized build is **3–5x** slower here — so those
> figures were meaningless as absolutes. The release column is authoritative.
> The *ratios* the section argues from were unaffected (traversal / equivalent
> join: 2.05x release, 1.96x debug), so the conclusions below stand as written.
> Anything measured through the dev server needs `--release` before it is
> quoted.

Three things this says, none of them flattering by default:

- **The edges-only join is not the comparison.** It never resolves the far
  vertex; a traversal always does. Against the equivalent join the gap is
  **~2x**, not the ~12x the edges-only row suggests. Quoting 137 ms against
  957 ms would be comparing different work.
- **The path value was 40% of the cost.** Serializing a full path per hit —
  every vertex and edge along the way — happened even when the query named no
  path variable, which most do not. Skipping it took the traversal from
  1,565 ms to 957 ms (debug, the build the optimization was developed against;
  320 ms vs 499 ms release). Name a path variable and the cost comes back,
  which is the honest trade: you pay for what you read.
- **The remaining ~2x is the per-start backend call.** One `traverse` per
  distinct start against the backend's cached adjacency (0.019 ms warm, re-run
  from `benches/perf.rs` at the time of this change), plus the plan running
  twice — once to discover the starts, once to answer. On a whole-collection
  outer side that is ~27k calls. The join instead groups the edge site once
  (D1b) and answers each row by hash lookup.

**Guidance, unchanged by wanting the feature to win:** for a 1-hop question over
a whole collection, the join rewrite is still about twice as fast. The
correlated traversal is the right tool when the outer side is bounded (a
filtered set: 87–130 ms in the same graph), when depth exceeds 1 — which a
single join clause cannot express at all — or when the query should read the
way the question is asked.

Smaller outer side, same graph (`primary_specialty == "Oncology"`, 888 edges),
debug build — kept for the shape, not the absolutes:

| form | time (debug) |
|---|---|
| 1-var (`FOR c IN …`) | 99 ms |
| 2-var (`FOR c, e IN …`) | 87 ms |
| 3-var (`FOR c, e, p IN …`) | 129 ms |
| inside a `LET` subquery | 100 ms |
| depth `1..2` | 155 ms |

## D10: move-calculations-down (2026-07-22)

The person-360 evaluation query was the worst on the board: **6.7x slower than
the reference backend**. Decomposition, not intuition, found the gap: of 26,870
persons, 651 survive the filter and 5 are returned — but the three enrichment
subqueries (`fees`, `engagements`, `orgs`), referenced **only in RETURN**, ran
for all 651. The count-only skeleton of the query was already faster than the
reference (30 ms vs 63 ms); everything else was work the projection threw away.
The reference engine's EXPLAIN names its cure directly: `move-calculations-down`
moves those subqueries past LIMIT, and its plan shows them after the LimitNode.

The planner now does the same: a `LET` whose variable feeds nothing but the
projection is moved to a deferred suffix that the runner executes after SORT and
LIMIT, on surviving rows only. `COLLECT` and mutations disable the pass;
blocking is over-approximated (any identifier read by a surviving op or sort
key blocks that name), which can only cost the optimization, never correctness.
A backend test pins the observable effect: with `LIMIT 2`, a deferred
`DOCUMENT()` fetches exactly the two survivors' targets.

Person-360 against the 254k-document evaluation graph, release builds, best
of 5 (reference backend: 63 ms):

| formulation | before | after |
|---|---|---|
| traversal form (AQL-mirror) | 759 ms | **322 ms** |
| join form, as previously written | 428 ms | 259 ms |
| join form, count-only pre-limit + no `DOCUMENT` | — | **99 ms** |

The last row is the honest best idiom: FILTER/SORT need only the contract
*count*, so the query computes exactly that before LIMIT (the object-building
subquery was dead weight both here and in the reference formulation), and every
lookup is an indexed join, which keeps the plan off the fetch-and-retry path
entirely.

Whole 8-query evaluation board after this change (all answers verified equal,
release builds): reference 921 ms, CogniGraph best-idiom **788 ms (0.86x)** —
from 6.7x behind on the worst query to net faster on the workload.

## D11: retry only the phase that missed (2026-07-22)

Follow-up to D10 and the `DOCUMENT()`/D2 fetch-and-retry design. The loop
re-ran the **whole plan** per round — clone every materialized site, re-scan,
re-filter, re-sort — even when every miss came from the deferred suffix that
D10 had just confined to the surviving rows.

The plan now splits at the deferral boundary. The head (main body, COLLECT,
SORT, LIMIT) runs under its own retry loop only if it can miss at all — decided
statically from where `DOCUMENT()` calls and correlated traversals sit — and
when it cannot, it runs exactly once with no defensive clone. Tail rounds
(deferred LETs + projection) borrow the sites instead of consuming them, so a
round costs a clone of the surviving rows, not of the site map. Three backend
tests pin the behaviour by counting collection scans: one scan, not one per
round, for tail-only and projection-only reads — including the nested
`DOCUMENT(DOCUMENT(x)._id)` case that takes two tail rounds.

Measured on the evaluation graph (release, best of 5):

| query | before | after |
|---|---|---|
| person 360, `DOCUMENT` in deferred LETs | 259 ms | **81 ms** |
| `FOR e IN contract_party RETURN DOCUMENT(e._to).country` (17.6k rows) | ~2 full passes | **77 ms** |
| person 360, traversal mirror (head rounds still needed) | 322 ms | **247 ms** |

The first row changed which formulation is the best idiom: with the loop no
longer punishing `DOCUMENT`, the `DOCUMENT`-bearing form (81 ms) now beats the
join-only rewrite (99 ms) that existed to avoid it. Board total after this
change: reference 927 ms, CogniGraph best-idiom **780 ms (0.84x)**; the worst
query stands at 1.3x, from 6.7x at the start of the day.
