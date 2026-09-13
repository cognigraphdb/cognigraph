# CG-32 semantic and graph cache identity — 2026-09-08

The completed review/remediation through CG-31 was committed as `291fbb1`
before this batch. CG-32 changes were subsequently implemented and verified
locally; they remain uncommitted. No push was performed, and existing
research/draft changes and the `CLAUDE.md` deletion were preserved.

## Change and contract

Semantic and graph-augmented search now use the same structural approach as
hybrid search: a route-specific version plus JSON serialization of every
result-shaping request field. Query text has a separate normalized component,
so similarity matching can vary the query but cannot vary parameters.
Optional values and complete collection names retain their boundaries.

Semantic `model_name` preserves its existing execution meaning: omitted or
JSON null accepts any stored model; an empty string matches only the literal
empty stored model name. Their cache keys now differ. The OpenAPI definition
documents this behavior and the previously omitted semantic model filter.

Existing cache fixtures call the production key builders. The implementation
does not change graph traversal, rank-hint handling, RRF fusion, cache weights,
or invalidation: weak graph hits still recompute graph facts from live
traversal, and strong hits retain the correct entry's trace metadata.

## Verification

Required Rust gates passed:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

The full suite passed **876 tests**, zero failed or ignored, across 66 result
summaries; the server passed 267 tests. Four new tests cover all four semantic
and eleven graph result parameters, optional/default equivalence, and real
route-level exact/strong/assisted collisions. The focused search suite passed
23 tests, including existing weak graph-metadata freshness, rank-hint,
deleted-document, and below-floor behavior checks. The release build and
OpenAPI drift checks also passed.

The initial HTTP fixture tried to create the managed `entities` collection
through the generic API and correctly received 403. It was changed to the
ordinary synthetic `graphdocs` collection; no production authorization rule
was relaxed. All subsequent baseline and fixed HTTP probes passed.

## Real release HTTP evidence

A saved pre-fix release and the fixed release ran with authenticated,
disposable databases and a synthetic loopback embedding HTTP provider.
The query vectors produce cosine 1.0 for a strong similar-query match and
0.8 for a weak assisted match. No external provider or production data was
used. Native resident and paged storage were each exercised with both memory
and persistent query caches.

| Scenario | Pre-fix release | Fixed release |
|---|---|---|
| Omitted → empty model filter, exact/strong hit | Reuses unfiltered results | Fresh search returns only the literal empty-model document |
| Empty → omitted model filter, exact/strong hit | Reuses the narrow answer | Fresh search returns all matching models |
| Changed model filter, weak match | Uses the other filter's assisted cache entry | Parameter mismatch produces fresh results |
| Colliding edge/neuron names, exact/strong hit | Returns the previous traversal's documents and graph facts | Returns the requested traversal's documents and graph facts |
| Colliding edge/neuron names, weak match | Adds an unrelated prior document, although graph facts are fresh | Returns only the requested traversal; graph facts remain fresh |

Each executable ran 44 changed-parameter probes: six semantic and three graph
cases per configuration, plus one semantic and one graph case after restart,
across four configurations. Exact repeats and valid same-parameter strong
similarity worked throughout. The fixed executable also verified valid
same-parameter assisted reuse and cached graph-fact round trips.

Restarts discarded result entries. Previously warmed embeddings required zero
provider calls with the persistent cache and one with the memory cache. The
key change requires no data or cache migration: result entries are memory-only,
and embedding keys/storage are unchanged. ArangoDB was not live-tested; these
request key builders are backend-neutral.

- [Pre-fix observations and executable hash](../evidence/engineering-historical-checks.md#artifact-948dca0a6be8010c9af4)
- [Fixed observations and executable hash](../evidence/engineering-historical-checks.md#artifact-f0a9f30384601ae94285)
- [Standalone HTTP regression](../evidence/engineering-historical-checks.md#artifact-0d3007e97d268a33d54e)

```bash
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/search-cache-http.py
```

Use `--binary PATH --expect-vulnerable` to check a saved pre-fix executable.
The script prints its temporary results path and stops its test servers.

The registry now has **14 Resolved and 18 Open issues**: 17 P2 and one P3,
with no open P1. The next bounded quick win is CG-29's completion-provider
override handling.
