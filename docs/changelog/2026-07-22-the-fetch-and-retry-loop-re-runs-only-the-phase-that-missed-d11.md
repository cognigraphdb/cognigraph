# The fetch-and-retry loop re-runs only the phase that missed (D11)

- Date: 2026-07-22
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:456-471` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **The fetch-and-retry loop re-runs only the phase that missed (D11).**
  A plan that reads the backend mid-row (`DOCUMENT()`, correlated traversals)
  used to re-execute in full per retry round — every materialized site cloned,
  every scan and sort repeated — even when all misses came from LETs the
  planner had just deferred past LIMIT. The plan now splits at the deferral
  boundary: the head retries only if a miss can arise there at all (decided
  statically from where the backend reads sit), and otherwise runs exactly
  once, with no defensive site clone; tail rounds replay just the deferred
  LETs and the projection over the surviving rows, borrowing sites rather
  than consuming them. Backend tests count collection scans to pin the
  contract: one head run for tail-only and projection-only reads, nested
  lookups included. Person 360 with `DOCUMENT` in its enrichment went
  259 → 81 ms, and a whole-scan `RETURN DOCUMENT(e._to).country` over 17.6k
  edges runs in 77 ms — the `DOCUMENT`-bearing formulation now beats the
  join rewrite that existed to avoid it.
