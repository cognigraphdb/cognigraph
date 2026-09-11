# Multi-tenancy M2: per-tenant stores, routed at one structural choke point

- Date: 2026-07-13
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1477-1497` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Multi-tenancy M2: per-tenant stores, routed at one structural choke
  point.** With `COGNIGRAPH_DATA_DIR` set, every tenant gets its own
  redb store (`<dir>/<tenant>.redb`, lazily opened, filesystem-safe
  names enforced) and auth moves to a separate control store
  (`<dir>/_control.redb`) — restoring or deleting a tenant's data never
  touches credentials. Rather than threading a per-request backend
  through ~40 handlers (where one missed call site IS the cross-tenant
  leak), `AppState.backend` becomes the `RoutedBackend` facade: the
  auth middleware wraps each handler future in a task-local tenant
  scope, and every backend call resolves through it — handlers are
  untouched, CGQL needs no rewriter, and isolation is structural. The
  query/embedding cache gets the same facade (`RoutedCache`,
  per-tenant in-memory — a shared cache is a cross-tenant side
  channel, D3). No task-local scope (auth disabled) resolves to the
  implicit default tenant; without `COGNIGRAPH_DATA_DIR` nothing
  changes at all. **The D1 overhead question, measured:** 100 real
  per-tenant stores open in 3.0 s total (~30 ms one-time lazy cost per
  store) with write+read verified in each (test prints timings).
  Isolation pinned by test: tenant A's document is invisible to tenant
  B and to the default tenant — different stores, not a filter. 379
  tests.
