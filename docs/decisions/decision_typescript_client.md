# Decision: an in-repo, zero-dependency TypeScript client with opt-in read retries

Status: accepted and implemented 2026-09-23 for [CG-84](../issues/CG-84.md).

## Context

Bun and Node applications, including the PLU Finder migration from ArangoDB,
reach CogniGraph through a thin query wrapper. Without a client each
application re-decides bind-variable handling, error mapping, timeouts and
retries, and drifts from the HTTP contract silently.

## Decision

1. **In-repo and npm-ready (owner choice).** The package lives in
   `clients/typescript/` as `@cognigraph/client`, versioned independently
   (0.1.0), FSL-1.1-Apache-2.0 like the Community server. It ships compiled
   ESM with declarations so Node 20.3+ needs no bundler or TypeScript loader.
   Publishing to npm is a separate, later decision.
2. **Zero runtime dependencies.** The platform `fetch` and `AbortSignal`
   helpers are enough; development tools share the UI's versions and join
   the dependency-freshness gate and `bun audit`.
3. **Scope (owner choice).** The acceptance list (read and mutation CGQL,
   documents, batch, health) plus login and the CG-86 index calls, and a
   `request()` escape hatch for other routes. Search, graph, Lua and
   Enterprise routes are not wrapped yet.
4. **Typed errors.** One class per documented status, a
   `UniqueViolationError` subclass of `ConflictError` for
   `code: "unique_violation"`, and `NetworkError`, client-side `TimeoutError`
   (status 0) and `ProtocolError` for failures without a documented response.
   Every error keeps the server's message, `code`, body, `retryable` and
   `retryAfterMs`. `documents.get` returns `null` for 404 and
   `databaseHealth` reports 503 instead of throwing, because both are normal
   answers rather than failures.
5. **Retries are opt-in and read-only (owner choice).** Off by default. With
   `retry` set, only reads and explicitly idempotent raw requests retry 408,
   429, 503 and network failures, with full-jitter exponential backoff that
   respects `Retry-After` and is capped by `maxMs`. Writes are never retried,
   because a timed-out write may have committed. `Retry-After` dates must be
   IMF-fixdate: `Date.parse` alone accepts `-1` and `1.5`.
6. **Qualification.** Unit tests use a scripted `fetch`. Live tests start a
   real Community server with a disposable store, cover every call, run the
   compiled package under Node and run the documentation example; they run in
   the shared CI suite.

## Consequences

- Applications migrating from `arangojs` replace their wrapper with typed
  calls and get the same error vocabulary as the HTTP reference.
- A server change that alters a wrapped route's shape fails the client's
  live tests in CI.
- Revisit for npm publication, wrappers for search, graph and Lua routes,
  Enterprise calls, or a generated client from the OpenAPI document.
