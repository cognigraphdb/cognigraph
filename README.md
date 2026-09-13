# CogniGraph

An **evidence-linked multi-model database in Rust**. Store documents, connect
them with typed relationships, and query across graph, full-text and vector
search—with links back to supporting source text.

One server binary, Native storage backed by redb, and no external database or
model service required for the example below.
[Website](https://cognigraphdb.com) · [Engineering docs](docs/README.md) · [License](LICENSING.md)

```cgql
FOR a IN accounts
  FILTER a.case == @case
  FOR h, e IN 1..1 OUTBOUND a._id registered_on
    LET source = DOCUMENT(e.source_id)
    RETURN { account: a.handle, handset: h.label, evidence: source.text }
```

Community stores the relationships and evidence references your application
supplies. Enterprise adds evidence-bound construction through Semantic Neurons
and signed governance workflows.

Coming from ArangoDB? CGQL uses familiar AQL-style syntax for document queries,
joins and traversals. The [migration guide](docs/reference/aql-to-cgql.md) maps
supported behavior and compatibility limits.

## Five-minute start

Clone the public source and build the Community Docker image:

```sh
git clone https://github.com/cognigraphdb/cognigraph.git
cd cognigraph
docker build -t cognigraph .
docker run -d --name cognigraph -p 127.0.0.1:3000:3000 -v cognigraph-data:/data \
  -e COGNIGRAPH_AUTH_ENABLED=false -e COGNIGRAPH_EMBEDDING_PROVIDER=none \
  -e COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true cognigraph
curl --fail --retry 10 --retry-connrefused --retry-delay 1 http://127.0.0.1:3000/health/database
```

The demo runs without authentication and is exposed only on loopback. For a
shared deployment, configure [authentication](docs/operations/authentication.md).
See [running](docs/operations/running.md) for Docker Compose and production
configuration, or the [Helm chart](deploy/helm/cognigraph/README.md#install) for Kubernetes.

<details>
<summary>Build and run directly with Rust</summary>

Requires stable Rust and a C/C++ toolchain. From the cloned repository, build
the server and run in a scratch directory to isolate its database and `.env`:

```sh
cargo build --release -p cognigraph-server
COGNIGRAPH_BINARY="$PWD/target/release/cognigraph-server"
COGNIGRAPH_DEMO_DIR="$(mktemp -d)"
cd "$COGNIGRAPH_DEMO_DIR"
COGNIGRAPH_HOST=127.0.0.1 \
COGNIGRAPH_PORT=3000 \
COGNIGRAPH_NATIVE_PATH="$COGNIGRAPH_DEMO_DIR/cognigraph.redb" \
COGNIGRAPH_AUTH_ENABLED=false \
COGNIGRAPH_EMBEDDING_PROVIDER=none \
COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true \
"$COGNIGRAPH_BINARY"
```

Leave the server running and use another terminal for the requests below.
Bun is needed only for the optional [React console](docs/evidence/README.md).

</details>

## Follow a relationship to its evidence

This small digital-forensics example is entirely synthetic: no real case,
account, device or person. Start with a fresh demo database. Create an account,
a handset and its extraction report, then link the account to the handset with
an explicit reference to that report:

```sh
API=http://127.0.0.1:3000/api
H='content-type: application/json'

curl --fail-with-body -sS "$API/documents" -H "$H" -d '{
  "collection":"accounts", "_key":"rook_77", "handle":"rook_77", "case":"CS-0412"
}'
curl --fail-with-body -sS "$API/documents" -H "$H" -d '{
  "collection":"handsets", "_key":"8813", "label":"Handset 8813"
}'
curl --fail-with-body -sS "$API/documents" -H "$H" -d '{
  "collection":"extractions", "_key":"handset-8813",
  "text":"Accounts rook_77 and marla.v are registered on this handset."
}'
curl --fail-with-body -sS "$API/graph/relationships" -H "$H" -d '{
  "from":"accounts/rook_77", "to":"handsets/8813", "relation_type":"registered_on",
  "collection":"registered_on", "metadata":{"source_id":"extractions/handset-8813"}
}'

curl --fail-with-body -sS "$API/search/query" -H "$H" -d '{
  "query":"FOR a IN accounts FILTER a.case == @case FOR h, e IN 1..1 OUTBOUND a._id registered_on LET source = DOCUMENT(e.source_id) RETURN { account: a.handle, handset: h.label, evidence: source.text }",
  "bind_vars":{"case":"CS-0412"}
}'
```

The query response contains:

```json
{
  "results": [{
    "account": "rook_77",
    "handset": "Handset 8813",
    "evidence": "Accounts rook_77 and marla.v are registered on this handset."
  }],
  "count": 1
}
```

Here, `source_id` is application-defined edge metadata and `DOCUMENT()` reads
the referenced report. The application supplies the relationship; storing its
source reference does not validate the claim or create a signed attestation.
Prefix the query with `EXPLAIN` to inspect its plan. Use `/api/query` for CGQL
mutations when enabled; `/api/search/query` accepts read-only queries. Both
routes enforce their [access controls](docs/operations/authentication.md).

## Capabilities and editions

Community includes document storage, typed edges, BM25 text search, vector and
hybrid retrieval, CGQL, sandboxed Lua, authentication, an admin CLI and the console.
Native stores primary database state in a redb file; text indexes and optional
vector sidecars are rebuildable derivatives. Enterprise artifact storage has
separate [backup requirements](docs/operations/recovery.md).

| Edition | Available today | Planned |
|---|---|---|
| Community | Native storage, CGQL, search, Lua, auth, CLI, console, Docker and Helm | Read replicas and manual standby |
| Enterprise | Community capabilities plus Semantic Neurons, signed governance and multi-tenancy | Automatic failover, sharding and multiple writers |

Today the server is a single-writer deployment. Cargo and Docker default to
Community; [build editions](docs/operations/running.md#build-editions) explains
the Enterprise feature flag. Native is the only runtime storage backend.

Community Components are free to run in production under the
[FSL-1.1-Apache-2.0](LICENSE), with conversion to Apache 2.0 two years after each
release. Enterprise production use requires a commercial license.
[LICENSING.md](LICENSING.md) defines the component split and use restrictions.

## Rust workspace

| Crate | Responsibility |
|---|---|
| [cognigraph-core](crates/cognigraph-core/) | Shared types and `GraphBackend` contract |
| [cognigraph-query](crates/cognigraph-query/) | CGQL grammar, validation, planning and execution |
| [cognigraph-native](crates/cognigraph-native/) | Native persistence, indexes, traversal and vector search |
| [cognigraph-server](crates/cognigraph-server/) | HTTP service and runtime orchestration |
| [cognigraph-cli](crates/cognigraph-cli/) | Administration and offline repair |
| [cognigraph-construct](crates/cognigraph-construct/), [cognigraph-governance](crates/cognigraph-governance/), [cognigraph-artifacts](crates/cognigraph-artifacts/) | Enterprise construction, authority and artifact custody |

The [architecture guide](docs/architecture/README.md) covers the other libraries.
Start with [CONTRIBUTING.md](CONTRIBUTING.md) for development, local checks and
the contributor agreement. [AGENTS.md](AGENTS.md) owns repository workflow rules.

## Documentation

- [Engineering index](docs/README.md) and [implementation plan](docs/implementation-plan.md)
- [CGQL specification](docs/reference/cgql.md), [AQL migration](docs/reference/aql-to-cgql.md) and [HTTP API](docs/reference/http-api.md) (also served at `/openapi.yaml`)
- [Configuration](docs/operations/configuration.md), [operations](docs/operations/README.md) and [DataOps](docs/dataops/README.md)
- [HTTP and Lua examples](docs/examples/README.md), [research evidence](docs/research/README.md), [issues](docs/issues/README.md) and [decisions](docs/decisions/README.md)
- [Changelog](docs/changelog/README.md) and [Enterprise license](LICENSE-COMMERCIAL)

Copyright 2026 Gedank Rayze, LDA.
