# CogniGraph

A multi-model database — documents, graphs, full-text and vector search in
one binary — with a query language that AQL users already know how to write. Rust, one file on disk, no external services. [cognigraphdb.com](https://cognigraphdb.com) · [github.com/cognigraphdb](https://github.com/cognigraphdb)

```cgql
FOR p IN products
  FILTER p.category == @cat
  FOR v IN 1..2 OUTBOUND p._id supplied_by
    COLLECT supplier = v.name WITH COUNT INTO n
    SORT n DESC
    LIMIT 10
    RETURN { supplier, n }
```

If you built on ArangoDB Community Edition and need somewhere to go, start
with [Migrating from ArangoDB](docs/reference/aql-to-cgql.md). The single-node
server is free to run in production under the
[Functional Source License](LICENSE); see [what is free and what is paid](#free-and-paid).

## Five-minute start

**Docker**

```sh
docker build -t cognigraph .
docker run -d --name cognigraph -p 127.0.0.1:3000:3000 -v cognigraph-data:/data \
  -e COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true cognigraph
curl --fail http://127.0.0.1:3000/health/database
```

**From source** (stable Rust, a C/C++ toolchain; Bun only for the optional
[console](ui/)). Build here, run in a scratch directory so a stray `.env` or
database does not become part of the demo:

```sh
cargo build --release -p cognigraph-server
COGNIGRAPH_BINARY="$PWD/target/release/cognigraph-server"
COGNIGRAPH_DEMO_DIR="$(mktemp -d)"
cd "$COGNIGRAPH_DEMO_DIR"
COGNIGRAPH_HOST=127.0.0.1 \
COGNIGRAPH_PORT=3000 \
COGNIGRAPH_BACKEND=native \
COGNIGRAPH_NATIVE_PATH="$COGNIGRAPH_DEMO_DIR/cognigraph.redb" \
COGNIGRAPH_AUTH_ENABLED=false \
COGNIGRAPH_EMBEDDING_PROVIDER=none \
COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true \
"$COGNIGRAPH_BINARY"
```

**Kubernetes**: follow the [chart installation instructions](deploy/helm/cognigraph/README.md#install),
including the required authentication Secret.

Both demo paths run without authentication on loopback. Before anything
shared, read [running](docs/operations/running.md) and
[authentication](docs/operations/authentication.md).

## First data, first query

```sh
Q=http://127.0.0.1:3000/api/query
H='content-type: application/json'

curl -s $Q -H "$H" -d '{"query":"INSERT { _key: \"apple\", name: \"Apple\", category: \"fruit\" } INTO products"}'
curl -s $Q -H "$H" -d '{"query":"INSERT { _key: \"orchard\", name: \"Orchard Co\" } INTO suppliers"}'
curl -s http://127.0.0.1:3000/api/graph/relationships -H "$H" \
  -d '{"from":"products/apple","to":"suppliers/orchard","relation_type":"supplied_by","collection":"supplied_by"}'

curl -s $Q -H "$H" -d '{
  "query": "FOR p IN products FILTER p.category == @cat FOR v IN 1..1 OUTBOUND p._id supplied_by RETURN { product: p.name, supplier: v.name }",
  "bind_vars": { "cat": "fruit" }
}'
```

Prefix any query with `EXPLAIN` to see the plan and what was pushed down to
storage. `POST /api/search/query` is the read-only twin of `/api/query` and
uses the same authentication and access controls. Read-only queries can still
return sensitive data; configure access before sharing either route. The full route list is in the
[HTTP API reference](docs/reference/http-api.md) and served live at
`/openapi.yaml`.

## What is in the box

- **Native storage** — redb single-file durability, resident or paged mode,
  int8 memory-mapped vectors, tantivy BM25. Everything but the primary file
  is a rebuildable cache.
- **CGQL** — `FOR`/`FILTER`/`LET`/`COLLECT`/`SORT`/`LIMIT`/`RETURN`, joins by
  nested `FOR`, subqueries, bounded-depth traversals, `VECTOR_SEARCH` as a
  source, mutations with `OLD`/`NEW`, `EXPLAIN ANALYZE`. [Specification](docs/reference/cgql.md).
- **Search** — text, vector, hybrid (reciprocal rank fusion), semantic, and
  graph-augmented retrieval as HTTP endpoints; server-side embedding via
  OpenAI-compatible, Ollama, Gemini or ONNX.
- **Lua** — sandboxed scripts with `graph.query()`, instruction and time
  budgets, scoped write access. [Reference](docs/examples/lua-scripting.md).
- **Operations** — auth with roles and API tokens, rate limits, request
  timeouts, Prometheus `/metrics`, hot JSON snapshots, an admin CLI, a React
  console, Docker image and Helm chart.
- **Enterprise** — Semantic Neurons (evidence-bound construction with
  recall/restraint gates), the signed governance chain (M15–M26),
  multi-tenancy. These are in this repository under a separate license.

ArangoDB remains available as a conformance backend for the query corpus. It
is not a production target and does not support Native batches, governed
construction or snapshots.

## Free and paid

| | Community (FSL) | Enterprise |
|---|---|---|
| Native storage, CGQL, Lua, search, auth, CLI, console, chart | Free, production, commercial products included | — |
| Read replicas and manual standby (planned) | Included when released | — |
| Semantic Neurons, signed governance, multi-tenancy, clustering | Evaluation only | Subscription |
| Offering CogniGraph itself as a hosted or competing database | Not permitted | Not permitted |
| Converts to Apache 2.0 | Two years after each release | Never |

Cargo and Docker default to Community; see [building either edition](docs/operations/running.md#build-editions).
Details, crate map and FAQ: [LICENSING.md](LICENSING.md). Contributions need
the [CLA](CLA.md) via `git commit -s`; see [CONTRIBUTING.md](CONTRIBUTING.md).

## Repository map

| Path | Purpose |
|---|---|
| `crates/cognigraph-server/` | HTTP service and runtime orchestration |
| `crates/cognigraph-cli/` | Administration and offline repair CLI |
| `crates/` | Storage, query, auth, retrieval, construction and governance libraries |
| `ui/` | Bun/React management console |
| `deploy/helm/` | Kubernetes chart |
| `fixtures/` | Reproducible examples, corpora and sealed experiment captures |
| `docs/` | Engineering guides, decisions, issues and changelog |

## Documentation

- [Engineering index](docs/README.md) and [current implementation plan](docs/implementation-plan.md)
- [Architecture](docs/architecture/README.md), [CGQL](docs/reference/cgql.md), [AQL migration](docs/reference/aql-to-cgql.md) and [HTTP API](docs/reference/http-api.md)
- [Operations](docs/operations/README.md), [configuration](docs/operations/configuration.md) and [DataOps](docs/dataops/README.md)
- [Examples](docs/examples/README.md), [research](docs/research/README.md), [issues](docs/issues/README.md) and [decisions](docs/decisions/README.md)
- [Changelog records](docs/changelog/README.md) and [agent instructions](AGENTS.md)
- [What is CogniGraph](../docs/product/what-is-cognigraph.md) (one page) and the [product overview](../docs/product/overview.md) live in the sibling docs checkout, with sales and publication material.

## License

Community Components: [FSL-1.1-Apache-2.0](LICENSE). Enterprise Components:
[CogniGraph Enterprise License](LICENSE-COMMERCIAL). Which is which:
[LICENSING.md](LICENSING.md). Copyright 2026 Gedank Rayze, LDA.
