# Correlated subquery equality is now index-assisted (D1b) — the workload goes from 4 of 8 queries completing to 8 of 8

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:579-594` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Correlated subquery equality is now index-assisted (D1b) — the workload goes
  from 4 of 8 queries completing to 8 of 8.** A correlated subquery re-runs its
  body per outer row against a site materialized once, and the engine was
  expanding every document into an environment before `FILTER` discarded almost
  all of them — O(outer x site) clones. `SiteRows::IndexedDocs` now carries the
  documents plus a hash index on one attribute path, built the first time a
  correlated equality filters that site, so the per-row cost becomes a lookup.
  Only string values are bucketed: a string can never equal a non-string under
  `values_equal`, so the index may skip every other document without changing
  which rows survive, and a non-string key falls back to the full expansion. The
  surviving `FILTER` still applies the whole expression — the index only narrows,
  it never decides. Measured on the 254k-document workload: worst case
  **152,782 ms → 132 ms (1,157x)**, entity profiling and the two coverage-gap
  queries went from "did not complete" to 175–301 ms, and row counts agree with
  the reference backend on every query.
