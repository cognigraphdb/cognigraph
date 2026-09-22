# CogniGraph Enterprise

The Enterprise edition of CogniGraph, an evidence-linked multi-model database in Rust. It includes the Community database plus Semantic Neurons, signed governance workflows and multi-tenancy. Native storage requires no external database.

## License

Enterprise Components use the [CogniGraph Enterprise License](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSE-COMMERCIAL). Evaluation, development and testing are available under its terms; production use requires a commercial subscription. A public image does not grant unrestricted production or redistribution rights. Community Components retain their [FSL-1.1-Apache-2.0 license](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSE). See the [component licensing guide](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSING.md).

## Image availability

Pull the current stable release:

```sh
docker pull cognigraph/cognigraph-enterprise:latest
```

The `latest` alias advances after both editions pass release checks and their numbered images are published. Explicit `x.y.z` tags remain immutable. Select a version from the Tags page and pin its digest for reproducible deployment. Releases published after 2.7.27 are multi-architecture images for `linux/amd64` and `linux/arm64` (members also tagged `x.y.z-amd64` and `x.y.z-arm64`); earlier tags are `linux/amd64` only and need `--platform linux/amd64` on ARM hosts.

## Runtime and operations

Each release image includes the HTTP API, React console and `cognigraph` CLI. The server listens on port 3000, uses persistent storage mounted at `/data`, and runs as UID/GID 10001. The current database supports one writer; automatic failover and multi-writer clustering are not shipped capabilities.

Configure [authentication and tenant access](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/authentication.md) and follow the [running guide](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/running.md). Governed operations may also use external artifact storage: include those artifacts and authority material in your [backup and recovery procedure](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/recovery.md).

For a single-tenant database without governed construction, use [CogniGraph Community](https://hub.docker.com/r/cognigraph/cognigraph).

[Website and licensing contact](https://cognigraphdb.com) · [Source and issues](https://github.com/cognigraphdb/cognigraph) · [Engineering documentation](https://github.com/cognigraphdb/cognigraph/blob/main/docs/README.md)
