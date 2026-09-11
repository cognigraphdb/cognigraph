# Error visibility: non-2xx request logging + a console logs panel

- Date: 2026-07-17
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1256-1271` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Error visibility: non-2xx request logging + a console logs panel.**
  Every non-2xx response is now logged to stdout (structured tracing,
  `warn` for 5xx / `info` for 4xx) with method, path, status, latency,
  tenant, and error message — previously a failed request left no server
  trace at all. The same events fill a bounded in-memory ring (200)
  exposed at `GET /api/admin/logs` (Admin scope) and shown as a "Recent
  errors" table in the console's Operations screen, alongside a metrics
  band parsed from `/metrics`. The log is scoped to the caller's tenant
  plus untenanted events (pre-auth 401s, unknown routes): the auth
  middleware stamps the resolved tenant onto the response, the metrics
  layer records it, and the endpoint filters by it — a tenant admin never
  sees another tenant's request paths (regression-tested). A non-admin
  session gets an honest gated state, not an error. Streaming these events
  to an external HTTP consumer (Datadog Vector, etc.) is a planned
  extension; the ring's `record` is the fan-out point.
