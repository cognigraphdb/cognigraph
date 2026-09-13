# Decision: query-cache backends — persist embeddings on redb (H7)

**Status:** Decided and landed 2026-07-06.

## Context

The `QueryCache` trait was built for pluggable backends and the Phase-8
plan sketched "LMDB-backed cache layer for hot embeddings" plus a possible
Redis backend. Only the in-memory LRU existed. The cache has two levels
with very different persistence value:

- **Embedding cache** (text+model → vector): deterministic, never goes
  stale, and every miss costs a provider round-trip (~hundreds of ms and
  real money). Losing it on restart is pure waste.
- **Result cache** (query → hits): TTL-bounded (15 min default) and
  invalidated on writes — after a restart nearly everything in it would
  be expired or distrusted anyway.

## Decision (owner: agent, within the approved H7 scope)

1. **`PersistentCache` = in-memory semantics + a redb embedding store.**
   Same lookup behavior as `InMemoryCache` (it wraps one); embeddings
   additionally write through to a redb table and read through with
   promotion on a memory miss. Results, similarity index, stats, TTL,
   and LRU stay exactly as before — memory-only.
2. **redb, not LMDB.** The stack already ships redb (native backend);
   a second embedded KV store (heed/LMDB) would duplicate a dependency
   class for no capability gain. The Phase-8 "LMDB layer for hot
   embeddings" intent is delivered, on the engine we already operate.
3. **Immediate durability is fine here.** A store write happens ONLY
   after a fresh provider embedding call; the ~4 ms fsync (H4) rides on
   a ~300 ms network round-trip. Measured: put 4.4 ms, memory hit
   4.3 µs, cold read-through 16 µs (benchmarks.md). The hot read path
   never touches the store. (redb 4.x offers only None/Immediate;
   None-durability commits are not flush-on-close safe, so Immediate is
   also the only honest choice for data meant to survive restarts.)
4. **Selection:** `COGNIGRAPH_QUERY_CACHE_BACKEND=persistent` +
   `COGNIGRAPH_QUERY_CACHE_PATH` (required — a configured-but-broken
   path panics at startup rather than silently degrading; unknown
   backend names warn and fall back to memory).
5. **Redis DEFERRED with trigger.** Its one advantage — a cache shared
   across instances — has no consumer while the platform is single-node
   (Phase 7 dropped, HA parked). Similarity-scan-over-Redis also needs
   RediSearch or full-bucket fetches; not worth designing for now.
   Trigger: a real multi-instance deployment.

## Outcome

Landed 2026-07-06: `persistent.rs` in cognigraph-cache, backend selection
in the server, cross-restart tests (embeddings survive reopen, results
deliberately do not, clear wipes the store, bad path fails loudly),
bench row in benchmarks.md. Unbounded store growth accepted for now —
entries are ~12 KB each at dim 1536; revisit if a store file ever
actually gets large (compaction = clear).

## Addendum (2026-07-16): request params partition the cache

Building the console's search tabs exposed a correctness hole: `CacheKey`
was only {collection, mode, normalized query}, so a semantic query warmed
at threshold 0.3 answered the identical query at threshold 0.9 with all
ten sub-threshold results verbatim (`"cached": true`), and a `limit=3`
repeat returned the full cached set. Hybrid and graph-augmented entries
additionally ignored fusion weights, `rrf_k`, edge collection, depth, and
every other result-shaping knob. Latent in practice only because the
cache defaults off.

Decision: **exact on parameters, fuzzy on text.** `CacheKey` gains a
`params` fingerprint (each route encodes its result-shaping fields), and
the similarity index buckets by (collection, mode, params) so a similar
query can never borrow results computed under different parameters. The
alternative — re-filtering cached results on the way out — only works for
semantic (cosine scores); hybrid/graph-augmented scores are RRF/decayed
and not comparable to a threshold post hoc, so keying is the uniform fix.
Trade-off accepted: entries no longer shim across thresholds/limits at
all (t=0.3 never serves t=0.35); hit rates only drop for clients that
vary parameters per query, which the console does not.

Landed 2026-07-16 (`e2c274f`): regression tests prove params partition
both the exact and similarity lookup paths, plus a route-level
warm-loose/re-query-strict flow; verified live with the cache enabled.

## Addendum (2026-09-08): hybrid parameter identity is structural

CG-31 found that comma-joining field arrays and delimiter-joining names made
distinct hybrid requests share the same fingerprint. Hybrid search now uses
versioned JSON serialization of every deserialized request parameter except
the separately normalized query. Field order and string boundaries survive
encoding, and cache fixtures call the production builder.

All 872 tests, formatting, and Clippy passed. Saved pre-fix and fixed release
HTTP probes covered exact, strong-similarity, and assisted lookup with four
collision families in resident/paged storage and memory/persistent caches.
Restarts correctly discarded results while persistent embeddings avoided a
provider call. No migration is needed: result entries remain memory-only.
See [reproduction and evidence](../issues/hybrid-cache-2026-09-08.md).
Separate semantic and graph-augmented encoding gaps discovered during this
work were recorded as [CG-32](../issues/CG-32.md), resolved below.

## Addendum (2026-09-08): semantic optional filters and graph collection names

CG-32 extends structural parameter identity to semantic and graph-augmented
search. JSON preserves `None` versus a literal empty string and boundaries
between edge/neuron collection names. Every result-shaping request field is
serialized; query text retains its separate normalized component. Existing
cache fixtures now use each route's production builder.

The semantic model contract is unchanged: absent/null accepts any model;
an empty string matches only that literal stored model name. OpenAPI now
documents this filter. Valid strong/assisted reuse, fresh weak-hit graph facts,
and result invalidation retain their existing behavior.

Formatting, Clippy, all 876 tests, and OpenAPI checks passed. Saved pre-fix and
fixed releases were tested over real HTTP for exact, strong-similarity, and
assisted collisions in resident/paged storage with memory/persistent caches.
Graph traversal and trace contamination were reproduced before the fix and
absent afterward. Restarts correctly discarded results while persistent
embeddings avoided a provider call. No migration is required. See
[the reproduction script and observations](../issues/search-cache-2026-09-08.md).
