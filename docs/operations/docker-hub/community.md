# CogniGraph Community

An evidence-linked multi-model database in Rust. Store documents, connect them with typed relationships, and query across graph, full-text and vector search using CGQL. Native storage requires no external database.

## Image availability

Pull the current stable release for `linux/amd64`:

```sh
docker pull cognigraph/cognigraph:latest
```

The `latest` alias advances after both editions pass release checks and their numbered images are published. Explicit `x.y.z` tags remain immutable. Select a version from the Tags page and pin its digest for reproducible deployment. Native ARM images are not currently published; on an ARM host, add `--platform linux/amd64` to use emulation.

## Runtime

Each release image includes the HTTP API, React console and `cognigraph` CLI. The server listens on port 3000 and uses persistent storage mounted at `/data`. It runs as UID/GID 10001. The current database supports one writer.

Follow the [running guide](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/running.md), configure [authentication](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/authentication.md), and review [backup and recovery](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/recovery.md) before a shared deployment. The quick-start example is a local, loopback-only demonstration.

## Editions and license

Community includes document and graph storage, full-text and vector search, CGQL, sandboxed Lua, authentication, the CLI and console. It stores the relationships and evidence references your application supplies.

Community Components are source-available under [FSL-1.1-Apache-2.0](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSE). See the [licensing guide](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSING.md) for permitted production uses and restrictions.

Semantic Neurons, signed governance workflows and multi-tenancy are available in [CogniGraph Enterprise](https://hub.docker.com/r/cognigraph/cognigraph-enterprise).

[Website](https://cognigraphdb.com) · [Source and issues](https://github.com/cognigraphdb/cognigraph) · [Engineering documentation](https://github.com/cognigraphdb/cognigraph/blob/main/docs/README.md)
