# @cognigraph/client

Typed HTTP client for [CogniGraph](https://github.com/cognigraphdb/cognigraph)
for Bun and Node 20.3+. Zero runtime dependencies: it uses the platform
`fetch`. Compiled ESM with type declarations; no bundler needed.

```ts
import { CogniGraph, UniqueViolationError } from "@cognigraph/client";

const anonymous = new CogniGraph({ baseUrl: "http://127.0.0.1:3000", retry: { attempts: 3 } });
const db = anonymous.withToken((await anonymous.login("admin", process.env.PASSWORD!)).token);

await db.indexes.ensure("users", { fields: ["email"] });
try {
  await db.documents.create("users", { _key: "ana", email: "ana@example.test" });
} catch (error) {
  if (error instanceof UniqueViolationError) console.log("email taken");
  else throw error;
}
const page = await db.query<{ _key: string }>(
  "FOR u IN users SORT u._key LIMIT @offset, @count RETURN u",
  { offset: 0, count: 20 },
);
```

A complete runnable example is [docs/examples/typescript-client.ts](../../docs/examples/typescript-client.ts).

## API

| Call | Route | Retried when `retry` is set |
|---|---|---|
| `login(username, password)` → `{ token, expiresIn }` | `POST /api/auth/login` | no |
| `withToken(token)` → new client | — | — |
| `query<T>(cgql, bindVars?)` → `T[]` | `POST /api/search/query` (read-only) | yes |
| `mutate<T>(cgql, bindVars?)` → `T[]` | `POST /api/query` | no |
| `batch(ops)` → results | `POST /api/batch`, all-or-nothing | no |
| `documents.get(collection, key)` → document or `null` | `GET /api/documents/{c}/{k}` | yes |
| `documents.list(collection, { limit, offset })` | `GET /api/documents` | yes |
| `documents.create(collection, doc)` | `POST /api/documents` | no |
| `documents.update` / `replace` / `delete` | `PATCH` / `PUT` / `DELETE /api/documents/{c}/{k}` | no |
| `indexes.list` / `ensure` / `drop` | `/api/collections/{c}/indexes` | list only |
| `health()`, `databaseHealth()` → `{ ok, status, body }` | `/health`, `/health/database` | yes |
| `request(method, path, body?, { idempotent })` | any route | only with `idempotent: true` |

Collection names and keys are percent-encoded, so every key character
ArangoDB allows round-trips. A document field named `collection` is refused
by `documents.create` because the API reserves that name; use a CGQL `INSERT`
for such documents. Every call accepts `{ signal, timeoutMs }`.

Always pass values as bind variables, including `LIMIT @offset, @count`.
Never splice them into CGQL text.

## Errors

Every failure is a `CogniGraphError` with `status` (0 when no response
arrived), the server's `message` and `code`, the parsed `body`, `retryable`
and `retryAfterMs`.

| Class | When | `retryable` |
|---|---|---|
| `BadRequestError` | 400: CGQL syntax or validation, malformed input | no |
| `AuthenticationError` | 401 | no |
| `ForbiddenError` | 403; `code` may be `enterprise_feature_required` | no |
| `NotFoundError` | 404 (except `documents.get`, which returns `null`) | no |
| `ConflictError` / `UniqueViolationError` | 409; the latter for `code: "unique_violation"` | no |
| `TimeoutError` | 408, or no response within `timeoutMs` (status 0) | yes |
| `RateLimitError` | 429, with `retryAfterMs` from `Retry-After` | yes |
| `ServerError` | 500 and other 5xx | no |
| `UnavailableError` | 503 | yes |
| `NetworkError` | connection refused, reset or DNS failure (status 0) | yes |
| `ProtocolError` | a success status whose body is not the documented JSON | no |

## Retries

Off by default. `retry: { attempts, baseMs = 200, maxMs = 5000 }` retries
retryable failures of reads (`query`, document reads, index listing, health,
and `request` with `idempotent: true`) with full-jitter exponential backoff.
It never waits less than the server's `Retry-After` and never more than
`maxMs`. Mutations, batches, document writes, index changes and login are
never retried automatically: a timed-out write may still have committed. For
those, catch the typed error and decide, for example by re-reading the
document or using an idempotent `UPSERT`. A caller's `AbortSignal` stops a
call and its retries at once.

## Development

```sh
bun install --frozen-lockfile
bun run check          # Biome + TypeScript
bun test test/unit
bun run build          # dist/ with .d.ts
CG_CLIENT_SERVER_BIN=../../target/release/cognigraph-server bun test test/live
```

The live tests start a disposable server, cover every call against it, run
the compiled package under Node and run the documentation example.
`python3 scripts/verify.py --suite client` runs all of it and is part of CI.

## License

FSL-1.1-Apache-2.0, like the Community server. Publishing to npm is a
separate step and has not happened yet.
