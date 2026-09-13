# Community and Enterprise build editions

- Date: 2026-09-10
- Status: Unreleased
- Kind: Build / API

## Change

Implement [CG-45](../issues/CG-45.md): server and CLI builds default to Community;
`--features enterprise` includes governed construction, jobs, promotions,
artifacts and multi-tenant execution. Community excludes the three Enterprise
crate dependencies. Generic graph edge ranking moves into Core, while accepted
neuron boosts stay Enterprise.

Community keeps ordinary database, search, Lua, auth and snapshot operations.
Tenant lifecycle and non-default identities receive a structured
`enterprise_feature_required` error. Governed operations are absent from its
routes and generated OpenAPI document; `/health` reports the build edition.
Community rejects Enterprise configuration and refuses nonempty governed or
generated state at startup/import. Ordinary data keeps the same on-disk format.

Docker defaults to Community and accepts `COGNIGRAPH_EDITION=enterprise` at build
time. Helm chart 0.2.0 adds `edition`, selects matching default image tags and
rejects Enterprise settings for Community. CI checks dependencies, strict Clippy
and workspace tests in both configurations and builds both images. No runtime
license-key mechanism or read replication is added.

## Validation

Formatting, strict Clippy and workspace tests passed for both builds (776
Community and 967 Enterprise passing entries, including eight early-returning
Arango entries per run without exported credentials). The
[issue record](../issues/CG-45.md) owns the command results and their limits. Reusable [real-binary probes](../../scripts/check-editions-live.py) cover
ordinary database operations, isolated tenant access, cross-edition data,
absent governed routes and refusal before import writes. They use disposable
Native stores and a local deterministic embedding fixture. Helm probes run the
rendered server and packaged backup in isolated hardened Docker containers;
no Kubernetes deployment is implied.
