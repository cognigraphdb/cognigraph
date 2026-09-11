# Running

## Build editions

Cargo and Docker build **Community** by default. Its server and CLI exclude
Enterprise crates, accept only the default tenant and keep ordinary database,
query, search, Lua, authentication and snapshot operations. `/health` reports
`edition`; `/openapi.yaml` advertises the operations in that build.

```sh
# Community server and CLI
cargo build --release -p cognigraph-server -p cognigraph-cli
# Enterprise server and CLI (production use requires the commercial license)
cargo build --release -p cognigraph-server -p cognigraph-cli --features enterprise
# Enterprise container
docker build --build-arg COGNIGRAPH_EDITION=enterprise -t cognigraph:enterprise .
```

Cargo features are additive: build only the desired binary packages and feature
set when producing a Community artifact. Building/testing every workspace crate
still compiles the Enterprise libraries as standalone workspace members. CI
checks each binary's normal dependency graph separately.

Community rejects `COGNIGRAPH_DATA_DIR`, a governance root or configured artifact
CAS before opening storage. Tenant lifecycle and non-default identity requests
return HTTP 403 with `code: enterprise_feature_required`; governed routes return
404. The CLI rejects Enterprise commands locally. No runtime license key is used.

Ordinary Native stores and snapshots share their format between editions.
Community refuses nonempty managed/generated and `_cognigraph_*` collections at
startup/import because their consistency needs Enterprise lifecycle handling.
This includes the already-reserved construction collections such as `entities`,
`chunks` and `facts`; see the [collection boundary](../decisions/decision_system_collections.md).
Keep using Enterprise for such stores; this is not an automatic downgrade tool.

## Running

### Docker (recommended)

```sh
docker build -t cognigraph .
docker run -d -p 3000:3000 -v cognigraph-data:/data cognigraph
```

or, production-shaped (auth + cache + budgets + JSON logs):

```sh
COGNIGRAPH_ADMIN_PASSWORD=change-me \
COGNIGRAPH_HOST_ADMIN_PASSWORD=use-another-secret \
COGNIGRAPH_JWT_SECRET=$(openssl rand -hex 32) docker compose up -d
```

The image runs as a non-root user with the persistent native backend at
`/data/cognigraph.redb` and a `/health` HEALTHCHECK.

### Bare binary

```sh
cargo build --release -p cognigraph-server
COGNIGRAPH_NATIVE_PATH=/var/lib/cognigraph/cognigraph.redb \
    target/release/cognigraph-server
```

A `.env` file in the working directory is loaded at startup. The server
drains connections on SIGTERM/SIGINT (systemd `Type=exec` works as-is):

```ini
[Unit]
Description=CogniGraph
After=network.target

[Service]
Type=exec
User=cognigraph
ExecStart=/usr/local/bin/cognigraph-server
Environment=COGNIGRAPH_NATIVE_PATH=/var/lib/cognigraph/cognigraph.redb
Environment=COGNIGRAPH_AUTH_ENABLED=true
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

### Kubernetes

A single-node Helm chart lives in [`deploy/helm/cognigraph`](../../deploy/helm/cognigraph/README.md):
StatefulSet with a persistent volume, auth Secret, probes on `/health` and
`/health/database`, and an optional snapshot-backup CronJob. `replicaCount`
is fixed at 1; read scaling is a [proposed design](../architecture/design-notes/read-replicas.md).

```sh
helm install cognigraph ./deploy/helm/cognigraph -n cognigraph --create-namespace \
  --set auth.adminPassword=... --set auth.hostAdminPassword=... --set auth.jwtSecret=...
```

## Endpoint namespaces

All application endpoints are served under **`/api`** (`/api/documents`,
`/api/graph`, `/api/neurons`, `/api/semantic-repairs`, `/api/query`, …) so the
web UI can own `/` and serve its built assets without shadowing an API route.
The operational routes stay at the **root** by convention: `GET /health`,
`GET /health/database`, `GET /health/jobs`, `GET /health/promotions`,
`GET /metrics`, and `GET /openapi.yaml`. The router/spec agreement is enforced by the
`openapi_drift` tests.

## Health and monitoring

- `GET /health` — process liveness (no auth). `GET /health/database` — backend
  readiness: HTTP 200 with `database: connected`, or HTTP 503 with
  `database: disconnected`. Use the latter for platform readiness probes. In
  multi-tenant mode this unauthenticated probe resolves the implicit `default`
  tenant; it is not an aggregate check of every tenant store.
- `GET /health/jobs` and `GET /health/promotions` — generic subsystem
  readiness (Enterprise only); detailed tenant-local diagnostics stay behind Admin endpoints.
- `GET /metrics` — request counts/latencies in Prometheus text format.
- Load envelope (single node, in-memory, 2026-07-03 benchmarks): ~135k
  point reads/s and ~120k pushdown CGQL queries/s at 128 concurrent
  clients with p99 < 3 ms; vector search saturates CPU around 13k req/s
  (rayon parallelizes each query across all cores). See the [dated benchmark results](../research/benchmarks/native-backend.md).

### Errors and logs

- Every **non-2xx response is logged to stdout** (structured `tracing`;
  set `COGNIGRAPH_LOG_FORMAT=json` for JSON lines) with method, path,
  status, latency, tenant, and the error message — `warn` for 5xx,
  `info` for 4xx. In production, capture stdout (docker/journald/etc.).
- The same events also fill a bounded in-memory ring (last 200), exposed
  at **`GET /api/admin/logs`** (Admin scope) and shown in the console's
  Operations screen. It is scoped to the caller's tenant plus untenanted
  events (pre-auth 401s, unknown routes) — a tenant admin never sees
  another tenant's request paths. It is a live operational aid, not a
  durable audit log: it is process-local and lossy, and clears on restart.
- To also see a span per request (all statuses), raise the log filter:
  `RUST_LOG=cognigraph=info,tower_http=info` turns on the mounted
  `TraceLayer`.
- Streaming these events to an external HTTP consumer (Datadog Vector,
  etc.) is a planned extension — the ring's `record` is the fan-out point.

## Capacity notes

- Resident mode holds all documents in RAM; with embedded vectors, RAM ~=
  dataset size. `sidecar` cuts vector RAM ~7x; `paged` keeps only keys + an
  LRU cache resident for datasets larger than RAM.
- Budgets (`COGNIGRAPH_CGQL_*`, `COGNIGRAPH_LUA_INSTRUCTION_LIMIT`) are the guardrails against
  untrusted queries; the request timeout backstops everything else.
- Single-node by design. HA/replication is out of scope for this phase;
  disaster recovery composes tested backend recovery, an M24 artifact bundle,
  and separately retained configuration/trust/secrets. No one component is a
  complete disaster-recovery story.

## Management console

Build the optional React console from `ui/` with Bun:

```sh
bun install --frozen-lockfile
bun run build
```

Set `COGNIGRAPH_UI_DIST` to the absolute path of `ui/dist` when starting the Rust
server. The production console calls the origin it was loaded from, preserving
the scheme, host and port. Direct page URLs and reloads use the server's SPA
fallback. The build explicitly removes the development API-port override.

For the split-port development setup, start the Rust API on port 3001, then run
`bun run dev` from `ui/`. Bun serves the UI on its advertised localhost URL
(normally port 3000); the dev command explicitly selects API port 3001 while
keeping the browser's host and protocol. To select different ports:

```sh
# From ui/: UI on 3020, an already-running Rust API on 38471.
COGNIGRAPH_UI_DEV_API_PORT=38471 bun --port=3020 ./index.html
```

`ui/bunfig.toml` allows Bun's HTML dev server to inline only the public
`COGNIGRAPH_UI_DEV_*` variables. Do not use this prefix for credentials or enable
unrestricted environment inlining. See [Bun's HTML environment configuration](https://bun.com/docs/bundler/html-static#inline-environment-variables).

The console verifies a protected catalog read before opening its screens.
It checks collections first; on 403 it checks the tenant catalog, which is the
permitted surface for host administrators. Either path requires a recognizable
successful response. A 401 requests login; unreachable servers, denied catalogs, server
errors or invalid responses show an unverified connection screen with Retry.
A saved session can retain an older API target. If verification fails,
**Use default server** removes that target and its token/session together before
returning to the current production origin (or explicitly configured dev port).
A fresh session on an authentication-disabled server is labelled accordingly.
