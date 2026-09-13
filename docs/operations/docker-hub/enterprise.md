# CogniGraph Enterprise

The Enterprise edition of CogniGraph, an evidence-linked multi-model database in Rust. It includes the Community database plus Semantic Neurons, signed governance workflows and multi-tenancy. Native storage requires no external database.

## License

Enterprise Components use the [CogniGraph Enterprise License](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSE-COMMERCIAL). Evaluation, development and testing are available under its terms; production use requires a commercial subscription. A public image does not grant unrestricted production or redistribution rights. Community Components retain their [FSL-1.1-Apache-2.0 license](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSE). See the [component licensing guide](https://github.com/cognigraphdb/cognigraph/blob/main/LICENSING.md).

## Image availability

Version **2.7.11** is available for `linux/amd64`.

```sh
docker pull --platform linux/amd64 cognigraph/cognigraph-enterprise:2.7.11
```

Releases use explicit `x.y.z` tags for `linux/amd64`. Tags are immutable; no `latest` alias or native ARM image is currently published. Select a version from the Tags page and pin its digest for deployment.

## Runtime and operations

Each release image includes the HTTP API, React console and `cognigraph` CLI. The server listens on port 3000, uses persistent storage mounted at `/data`, and runs as UID/GID 10001. The current database supports one writer; automatic failover and multi-writer clustering are not shipped capabilities.

Configure [authentication and tenant access](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/authentication.md) and follow the [running guide](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/running.md). Governed operations may also use external artifact storage: include those artifacts and authority material in your [backup and recovery procedure](https://github.com/cognigraphdb/cognigraph/blob/main/docs/operations/recovery.md).

For a single-tenant database without governed construction, use [CogniGraph Community](https://hub.docker.com/r/cognigraph/cognigraph).

[Website and licensing contact](https://cognigraphdb.com) · [Source and issues](https://github.com/cognigraphdb/cognigraph) · [Engineering documentation](https://github.com/cognigraphdb/cognigraph/blob/main/docs/README.md)
