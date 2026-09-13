# Query cache: request parameters partition the cache

- Date: 2026-07-16
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1296-1308` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Query cache: request parameters partition the cache.** The cache key
  was only {collection, mode, normalized query}, so a query warmed at
  threshold 0.3 answered the identical query at threshold 0.9 with all
  ten sub-threshold results verbatim, and a limit=3 repeat got the full
  cached set (latent — the cache defaults off). `CacheKey` gains a params
  fingerprint and the similarity index buckets by (collection, mode,
  params): the fuzzy lookup stays fuzzy on query text but exact on
  parameters. Each search route encodes its result-shaping fields
  (threshold/limit/model; fusion weights, `rrf_k`, search fields;
  edge collection, depth, direction, decay, seed limit, neuron knobs).
  Regression tests cover both lookup paths plus the route-level
  warm-loose/re-query-strict flow.
