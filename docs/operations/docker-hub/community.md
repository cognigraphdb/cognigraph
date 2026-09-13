# CogniGraph Community

An evidence-linked multi-model database in Rust. Store documents, connect them with typed relationships, and query across graph, full-text and vector search using CGQL. Native storage requires no external database.

## Image availability

The Docker Hub repositories are being prepared for the first image release. No image tags have been published yet. Until tags appear, use the [source-build quick start](https://github.com/cognigraphdb/cognigraph/blob/main/README.md#five-minute-start).

Releases use explicit `x.y.z` tags for `linux/amd64`. Tags are immutable; no `latest` alias or native ARM image is currently published. Select a version from the Tags page and pin its digest for deployment.

## Runtime

Each release image includes the HTTP API, React console and `cognigraph` CLI. The server listens on port 3000 and uses persistent storage mounted at `/data`. It runs as UID/GID 10001. The current database supports one writer.

Follow the [running guide](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/running.md), configure [authentication](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/authentication.md), and review [backup and recovery](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/recovery.md) before a shared deployment. The quick-start example is a local, loopback-only demonstration.

## Editions and license

Community includes document and graph storage, full-text and vector search, CGQL, sandboxed Lua, authentication, the CLI and console. It stores the relationships and evidence references your application supplies.

Community Components are source-available under [FSL-1.1-Apache-2.0](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSE). See the [licensing guide](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSING.md) for permitted production uses and restrictions.

Semantic Neurons, signed governance workflows and multi-tenancy are available in [CogniGraph Enterprise](https://hub.docker.com/r/cognigraph/cognigraph-enterprise).

[Website](https://cognigraphdb.com) · [Source and issues](https://github.com/cognigraphdb/cognigraph) · [Engineering documentation](https://github.com/cognigraphdb/cognigraph/blob/main/docs/README.md)
