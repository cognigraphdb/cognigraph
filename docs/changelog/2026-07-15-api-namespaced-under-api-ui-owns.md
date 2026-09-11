# API namespaced under `/api`; UI owns `/`

- Date: 2026-07-15
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1317-1331` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **API namespaced under `/api`; UI owns `/`.** All application endpoints moved
  from the root to `/api` (`/api/documents`, `/api/graph`, `/api/neurons`,
  `/api/query`, `/api/construct/*`, `/api/auth/login`, …) so the web UI can serve
  its built assets from `/` without shadowing an API route (static serving is not
  wired yet; this carves out the namespace ahead of it). Operational routes stay
  at the root by convention — `/health`, `/health/database`, `/metrics`,
  `/openapi.yaml` — as fixed paths that Prometheus/load-balancers expect and that
  are matched before any future SPA fallback. main.rs builds an `/api` sub-router;
  `openapi.yaml` re-prefixed (38 paths); the `openapi_drift` tests updated to
  special-case `/health` at root and tolerate the `/api` wrapper, and still assert
  router↔spec agreement both ways. UI `client.ts` applies the `/api` prefix at the
  single request choke point (feature call sites unchanged) and keeps `health()`
  at the root. Current-usage docs (operations, dataops guides, CGQL) updated;
  historical logs left as dated records.
