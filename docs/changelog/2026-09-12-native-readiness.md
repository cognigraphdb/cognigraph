# Qualify Native-only operation before first deployment

- Date: 2026-09-12
- Status: Unreleased
- Kind: Documentation and qualification

## Change

[CG-68](../issues/CG-68.md) reconciles active architecture, recovery, governance,
API, data preparation and verification guides with Native-only storage. Earlier
decisions receive scoped amendments while preserving their measured outcomes.
The new [first-deployment guide](../operations/first-deployment.md) explains fresh
setup, editions, persistence, credentials, backups and the deployment boundary.

The Native readiness harness reuses CG-67's sealed HTTP/Lua/storage protocol and
adds pre-storage bootstrap/provider/edition configuration rejection checks.
The [acceptance record](../issues/native-readiness-2026-09-12.md) tracks current
results and remaining publication prerequisites. Optional import work is deferred.

## Validation

The full shared CI suite passed: formatting, strict Clippy in both editions,
722/913 reported passing Rust tests, 161 UI tests, nine Chromium journeys and
script/edition/modularity/documentation gates. Provider skips are recorded
separately. Fresh release-binary checks passed 514 runtime assertions and 12
startup rejection cases, plus cross-edition HTTP/CLI and the setup example.
Both Linux/amd64 Docker images and both Helm live backup checks passed.

The acceptance record binds source, binaries, images and captures and identifies
separate sibling product/website follow-up. Local Native-only readiness is
complete; publication and deployment remain separate. The normal version
increment and final-candidate checks apply before an authorized push.
