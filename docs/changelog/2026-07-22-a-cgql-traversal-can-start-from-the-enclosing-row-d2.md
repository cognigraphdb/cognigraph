# A CGQL traversal can start from the enclosing row (D2)

- Date: 2026-07-22
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:488-518` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **A CGQL traversal can start from the enclosing row (D2).** `FOR p IN persons
  FOR c IN 1..1 INBOUND p._id contract_party` — the phrasing every graph question
  wants — previously could not be written at all: validation refused a traversal
  anywhere but the first `FOR`, and the start was hard-limited to a bind variable
  or a string literal. Both restrictions are gone. A traversal is now an ordinary
  `FOR`, allowed in any position and inside a `LET` subquery, and its start is an
  ordinary expression validated against the variables bound above it. `VECTOR_SEARCH`
  stays first-only: one index probe per outer row is a different decision.

  A correlated start resolves through the same fetch-and-retry loop as
  `DOCUMENT()` — chosen after measuring, not assumed: a warm traversal against
  the backend's cached adjacency is 0.019 ms, so one call per **distinct start**
  is affordable and the second index the query engine could have built over the
  edge collection was unnecessary. Starts dedupe, so the cost is per start value,
  not per outer row.

  Two changes came along because writing the first honest test query demanded
  them. **One- and two-variable traversals** now parse (`FOR v IN …`,
  `FOR v, e IN …`); all three were required before, which made an AQL user's
  reflex a parse error. And an **uncorrelated traversal in a non-first `FOR`**
  now merges with the rows above it instead of replacing them — the old code
  could not, because validation had guaranteed there were none.

  Naming no path variable now skips building the path value, which copies every
  vertex and edge along the way: 1,565 ms → 957 ms on a 27k-row outer side. The
  honest comparison is in `benchmarks.md` and is not flattering by default —
  against the equivalent join the correlated traversal is about **2x** slower on
  a whole-collection outer side, so the join rewrite remains the better tool for
  a 1-hop question at that scale. `EXPLAIN` now reports `correlated` per
  traversal, and lists only the variables the query actually named.
