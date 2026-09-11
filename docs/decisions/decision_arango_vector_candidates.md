# Decision: model-filtered Arango vector candidates and distinct-parent limits

Status: accepted and implemented 2026-09-09 for CG-20.

## Contract and implementation

`model_name` restricts the candidate population before truncation, using exact,
case-sensitive equality. No filter includes every model; an empty-string filter
matches only an explicitly empty model name. Arango applies this filter before
ranking/limiting in both indexed and fallback modes.

The indexed query uses Arango's pre-filter during `APPROX_NEAR_COSINE` lookup.
This capability requires ArangoDB 3.12.6 or newer. Older servers can use
`COGNIGRAPH_VECTOR_SEARCH_MODE=fallback`, whose exact cosine scan applies the
same filter before ranking. See the
[official vector function documentation](https://docs.arango.ai/arangodb/3.12/aql/functions/vector/).

Arango preserves its existing parent-level result identity: a string
`document_id` groups embedding chunks; otherwise the stored row's `_id` is the
identity. Keep the highest-scoring chunk for each identity, sort the resulting
hits by descending score, and return at most `limit` hits.

A fixed candidate multiplier cannot guarantee enough distinct parents. Candidate
queries now start at `limit` and double the window until enough distinct parents
are found or the query returns fewer eligible rows than the window. Thresholds
retain the existing inclusive behavior. Since candidates are sorted by score,
a threshold that cuts the candidate window cannot make later, lower-scoring
rows eligible. `limit = 0` returns no results without a database query, and window
growth saturates instead of overflowing.

This expansion removes deterministic underfill caused by duplicate parents.
Indexed search remains approximate: the chosen index and its configured search
effort still control recall, and searched cells may be exhausted before every
collection row is considered. The fix does not increase `nProbe` or promise
exact nearest-neighbor recall. Duplicate-heavy searches can require multiple
increasing queries; fallback mode repeats its scan for each window.

No storage migration or index replacement is required on supported servers.
The Arango vector implementation lives in a separate module. Native algorithms,
query-language routing, embedding providers, and model defaults are unchanged.
The Native sidecar contract discrepancy discovered during this work is tracked
separately as [CG-34](../issues/CG-34.md).

## Verification

The shared indexed/fallback fixture deliberately puts 80 model-A rows before
model B and includes 40 chunks for one model-B parent. It specifies expected
row identities and scores for 12 cases, including absent/empty model filters,
case sensitivity, thresholds, exhaustion, zero limit, duplicate parents, and
ordinary rows without a string parent identity.

Live verification uses ArangoDB 3.12.11 with trained cosine vector indexes and
one list, isolating filtering and parent selection from inter-cell approximate
recall. A loopback proxy captures the release process's actual AQL; `EXPLAIN`
verifies vector-index execution. HTTP and Lua repeat the fixture after restarting
the CogniGraph process. See the
[implementation and verification report](../issues/vector-model-filter-2026-09-09.md).

Formatting, strict Clippy, all 925 workspace tests, the release build, and all
88 Arango HTTP/Lua checks passed. All eight live Arango integration tests ran,
including indexed and fallback model/parent cases. The captured indexed plan
contains the model filter inside its vector-index node; both modes expanded
the duplicate-parent candidate windows through 4, 8, 16, 32, and 64 rows.
The disposable container and its volumes were removed after validation.
