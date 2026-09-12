# First Native deployment

CogniGraph runs its own Native store in both editions. One process owns a store;
replication, failover and multiple writers are not implemented. Use the
[Native-only acceptance record](../issues/native-readiness-2026-09-12.md) to
identify the locally qualified revision, platform and exclusions. Source or
image publication does not establish a live deployment.

## Fresh local setup

Build the Community binary from the code root:

```sh
cargo build --release -p cognigraph-server -p cognigraph-cli
```

Start in an empty, operator-owned directory so another checkout's `.env` cannot
select providers or existing stores. The following foreground process uses a
new persistent directory, loopback HTTP, authentication and no hosted providers.
Keep the generated Admin password available in this shell for login; do not
commit it. The directory remains after shutdown so you can test restart.

```sh
cognigraph_binary="$PWD/target/release/cognigraph-server"
cognigraph_store="$(mktemp -d)"
export COGNIGRAPH_ADMIN_PASSWORD="$(openssl rand -hex 24)"
export COGNIGRAPH_JWT_SECRET="$(openssl rand -hex 32)"
cd "$cognigraph_store"
env -i PATH="$PATH" \
  COGNIGRAPH_HOST=127.0.0.1 COGNIGRAPH_PORT=3000 \
  COGNIGRAPH_NATIVE_PATH="$cognigraph_store/cognigraph.redb" \
  COGNIGRAPH_AUTH_ENABLED=true \
  COGNIGRAPH_ADMIN_PASSWORD="$COGNIGRAPH_ADMIN_PASSWORD" \
  COGNIGRAPH_JWT_SECRET="$COGNIGRAPH_JWT_SECRET" \
  COGNIGRAPH_EMBEDDING_PROVIDER=none \
  COGNIGRAPH_CGQL_MAX_SOURCE_ROWS=100000 \
  COGNIGRAPH_CGQL_TIME_BUDGET_MS=5000 \
  "$cognigraph_binary"
```

`GET /health` reports version and edition. `GET /health/database` must report
connected; an unauthenticated `/api/documents` read must return 401. Use
[`POST /api/auth/login`](authentication.md) for a bearer token, then exercise
document CRUD and parsed [CGQL](../reference/cgql.md). `/api/search/query` is
read-only. Enable `COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true` only when the separate
mutation endpoint is wanted. Lua writes additionally require a write scope.

For a repeatable disposable acceptance run, build each edition into separate
directories and run [the Native harness](../../scripts/check-native-readiness.py)
with `--binary`, `--edition` and a new `--output` path. It exercises authentication,
CRUD, query/Lua permissions and budgets, search, snapshots, restart and invalid
bootstrap/provider settings with a local embedding fixture.

## Choose deployment settings

1. Select Community or Enterprise from [the edition guide](running.md#build-editions).
   Enterprise is required for governed construction and multiple tenants. Set
   only the single-store path or the Enterprise tenant directory. Validate the
   chosen edition through `/health` and its `/openapi.yaml` surface.
2. Choose durable storage with one writer and enough capacity for the database
   and derived indexes. Resident/embedded is the default; resident/sidecar and
   paged/sidecar are supported persistent modes. Paged requires sidecar. An unset
   single-store path means memory-only operation and loses data on restart.
3. Configure authentication, distinct operator credentials, JWT secret, query
   budgets and request limits using the [configuration guide](configuration.md).
   Terminate TLS at the trusted reverse proxy and keep the raw API private.
   Add the optional console through `COGNIGRAPH_UI_DIST`; the Docker image is
   API/CLI-only and does not bundle the React assets.
4. Plan a [backup and restore drill](recovery.md) on a fresh store. A hot import
   is additive; use an empty target for exact restoration. Cold-copy the redb
   file only after stopping its writer. Preserve external CAS bytes, trust and
   configuration/secrets separately when Enterprise governance uses them.
5. For containers, select the intended edition and exact qualified image digest.
   The local acceptance covers Linux/amd64. The [Helm guide](../../deploy/helm/cognigraph/README.md)
   requires an explicitly available image; local tags are not published images.
   Keep one replica. Helm rendering and Docker backup checks do not qualify a
   particular cluster, storage class, ingress or operator recovery procedure.

Before publication, follow the incoming-PR, version-increment and full-validation
[push workflow](push.md). Before first live deployment, review the acceptance
record, choose the target environment and execute that environment's readiness,
authentication, persistence and recovery checks. No customer migration or dump
importer is a prerequisite for a new Native installation.
