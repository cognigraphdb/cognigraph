# Hybrid cache identity (CG-31)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:355-364` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Hybrid cache identity (CG-31).** Hybrid result-cache keys now serialize
  all result parameters as versioned JSON, preserving collection/view names
  and ordered field lists. Delimiter-bearing requests cannot share exact,
  strong-similarity, or assisted results across different parameters.
  Formatting, Clippy, all 872 tests, and release HTTP regressions passed for
  resident/paged storage and memory/persistent caches, including restarts.
  No persistent-cache migration is required. Related semantic and graph search
  fingerprint defects are tracked as CG-32. See
  [verification](../issues/hybrid-cache-2026-09-08.md).
