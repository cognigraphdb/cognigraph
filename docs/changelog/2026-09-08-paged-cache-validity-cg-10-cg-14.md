# Paged-cache validity (CG-10, CG-14)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:385-393` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Paged-cache validity (CG-10, CG-14).** Paged writes retain the state lock
  through cache/vector publication; point reads hold it through catalog
  checks and cache fills. Dropping a collection clears its cached documents
  and loaded vector sidecar. Cold sidecar builds serialize and cannot publish
  across an intervening write. Controlled race regressions and release HTTP
  checks passed with paged caching enabled/disabled and resident sidecars,
  including concurrent access, drop/recreate, durable-export comparisons, and
  restarts. See [verification and tradeoffs](../issues/paged-cache-2026-09-08.md).
