# Start from the published Docker image in the README

- Date: 2026-09-14
- Status: Unreleased
- Kind: Documentation

## Changes

The [five-minute start](../../README.md#five-minute-start) pulls the published
Community `cognigraph/cognigraph:latest` image directly. It documents the included
console, persistent volume, stop/start commands, `linux/amd64` platform and fixed
version alternatives. The local demo retains loopback-only exposure, disabled
authentication and no embedding provider. The optional Rust instructions now
include their own clone step.

The readiness request retries transient empty responses as well as connection
failures, covering the interval between Docker opening its port and the server
accepting requests.

## Validation

Executed the README pull/run and HTTP example against the published Community
v2.7.14 Linux/amd64 image on a local ARM Docker host, using session-specific
container and volume names. Startup, served console HTML/JavaScript, the exact
document/relationship/query result and persistence after stop/start all pass.
The initial empty-response failure and successful retry are retained in the
private `cognigraph-evidence/runs/2026-09-14-readme-docker-start/` package.
Its `manifest.json` seals 26 artifacts; verified SHA-256:
`e6dcf5360cc0f7b62e52718acfc05458cf19a8d08dded4769bab048486f07c8a`.

Removed the test container, disposable volume and pulled image; before/after
inventories confirm pre-existing Docker resources were preserved. This is
documentation/example acceptance, not a new image release or Railway test.
