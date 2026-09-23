# v2.7.33 — TypeScript client for Bun and Node

- Date: 2026-09-23
- Status: v2.7.33
- Kind: Client SDK, CI and dependencies

## Changes

`clients/typescript` adds `@cognigraph/client` 0.1.0, a typed HTTP client for
Bun and Node 20.3+ with zero runtime dependencies ([CG-84](../issues/CG-84.md),
[decision record](../decisions/decision_typescript_client.md)).

- Calls: `login`, `withToken`, `query` (read-only CGQL), `mutate`, `batch`,
  document `get`/`list`/`create`/`update`/`replace`/`delete`, unique-constraint
  `indexes.list`/`ensure`/`drop`, `health`, `databaseHealth`, and a raw
  `request` for other routes. Collection names and keys are percent-encoded.
- Errors: one class per documented status, `UniqueViolationError` for
  `code: "unique_violation"`, and `NetworkError`, client-side `TimeoutError`
  and `ProtocolError`, each with status, message, code, body, `retryable`
  and `retryAfterMs`. `documents.get` returns `null` on 404.
- Retries: opt-in, reads only, full-jitter exponential backoff honoring
  `Retry-After` (seconds or IMF-fixdate) up to `maxMs`; writes are never
  retried.
- `docs/examples/typescript-client.ts` is a runnable example; the migration
  guide's `arangojs` row, the HTTP reference and the examples index link the
  client.
- CI: a new `client` verification suite (frozen install, Biome, TypeScript,
  unit tests, build, Community server build, live tests) runs in the shared
  CI suite; the dependency-freshness gate covers every Bun project and `bun
  audit` covers the client.

The workspace version moves to 2.7.33. The package is not published to npm.

## Validation

32 unit tests against a scripted `fetch` cover request shapes and encoding,
every error class, client timeouts, caller aborts, the retry rules and the
backoff and `Retry-After` math; the latter found and fixed lenient date
parsing that accepted `-1` and `1.5`. 8 live tests start a real Community
server and cover every call, typed 400/401/403/409 responses, CG-85 `LIMIT`
binds, CG-86 constraints through documents and CGQL, atomic batch rollback,
the compiled package under Node and the runnable example. The CI policy and
dependency-gate tests cover the new suite and projects.
