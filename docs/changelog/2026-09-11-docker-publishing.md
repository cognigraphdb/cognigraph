# Opt-in Docker Hub publication

- Date: 2026-09-11
- Status: Unreleased
- Kind: Maintenance

## Changes

Extend manual CI with a default-off publishing input. Full shared gates must
pass before the Docker job builds and checks Community and Enterprise images.
Publication is restricted to the official main branch and dedicated `cognigraph`
Docker Hub account, with separate repositories and stable version tags for
Linux amd64. Builds use Cargo.lock, copy only Cargo manifests and Rust crates
into the builder, include license files and label source, version, revision
and edition. Local generated data no longer enters the builder.

The shared Docker suite now runs packaged-image HTTP/CLI, authentication,
edition, license and restart/persistence probes. Publication tests immutable
local image IDs, rechecks the current main head and unused version tags, then
uploads those same images. It records each accepted registry digest, never
rebuilds after testing, and does not update aliases or overwrite release tags.
Manual runs are serialized and registry errors fail closed. Partial publication
requires explicit review; there is no automatic deletion or rollback.

The [operating guide](../operations/docker-publishing.md) documents account
setup, license boundaries, token rotation, supported architecture, first-run
instructions and failure recovery. Existing source-build onboarding remains
valid before the first remote publication.

## Verification

The full local CI suite passed formatting, strict Clippy in both editions,
edition dependency checks, module/documentation/issue guards, and 776 Community
and 967 Enterprise reported Rust test entries. As at the previous checkpoint,
each edition includes eight Arango entries that return early without the
exported password; no new external-service or model coverage is claimed.
An initial Community completion wire test exceeded its 10-second timeout during
concurrent Docker compilation. The isolated test and full CI rerun passed
without changing Rust source or increasing the timeout.

All 59 Python workflow tests passed, including 16 new publication cases covering
candidate/version identity, private/missing repositories, existing tags,
registry errors, edition/platform identity, failed runtime checks, container
cleanup, remote changes and partial upload reporting. Actionlint 1.7.12 passed.
Combined code/product documentation checks passed with 369 documents and 1,487
local links; the decision index and whitespace checks also passed.

The upload helper pushed the initial local packaging build into a disposable nested Docker
daemon and registry, verified the raw registry manifest digest and pulled back
the same image identity and configuration. The checked digest was
`sha256:4f25f02d5882217210631ce02ee09fade5ad0eb3723d3890367a7642aa4c25b0`.
This exercised the helper's actual tag/push commands without Docker Hub writes
or changes to Docker Desktop's registry configuration. An initial host-loopback
attempt could not reach the registry from Docker Desktop; the isolated daemon
resolved that environment boundary. All probe containers, volumes and newly
downloaded registry/daemon images were removed afterward.

The final shared Docker suite passed both locked release builds and both
packaged-container probes: non-root UID, version/edition, authentication,
edition API, licenses, persisted HTTP/CLI queries and restart. Probe containers
and their volumes were removed. Local image checks ran on Linux arm64.
The real platform guard also refused
these images when Linux amd64 was required. CI's amd64 build/runtime execution,
Docker Hub authentication/upload and remote Actions result remain untested.
No commit, push, remote workflow dispatch or Docker Hub image upload is part
of this change's local preparation.
