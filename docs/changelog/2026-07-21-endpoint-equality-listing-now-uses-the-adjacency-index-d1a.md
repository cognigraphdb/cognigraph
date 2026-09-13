# Endpoint-equality listing now uses the adjacency index (D1a)

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:595-610` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Endpoint-equality listing now uses the adjacency index (D1a).** The native
  backend maintained a per-edge-collection adjacency index for traversal but never
  consulted it in `list_documents_filtered`, so `FILTER e._to == <vertex>` cost a
  full collection scan. It now serves an `_from`/`_to` equality from that index in
  both storage modes: on a 17.6k-edge collection an endpoint-equality listing went
  **49 ms → 1 ms warm** (traversal for the same lookup is ~2 ms, so the paths now
  agree). Deliberately narrow — only `Eq` against a string on an EDGE collection,
  and every other predicate, offset and limit is still applied, so it can only
  make queries faster and never change which rows they return (a test pins a
  document collection with a field literally named `_to` to the scan path).
  **This does not fix correlated subqueries**, and the reason is recorded as a
  correction in `decision_cgql_v2_workload_gaps.md` F1: CGQL materializes a
  subquery's scan once with only bind variables in scope, so the correlated value
  never reaches the backend and the engine re-filters in memory per outer row.
  That remainder is D1b.
