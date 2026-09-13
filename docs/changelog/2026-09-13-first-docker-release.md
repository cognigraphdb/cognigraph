# First Docker images and Railway v2.7.11 accepted

- Date: 2026-09-13
- Status: Unreleased
- Kind: Release evidence and operations

## Changes

The corrected v2.7.11 candidate reached protected main, passed both main and
manual publishing CI, and published immutable Linux/amd64 Community and
Enterprise images to Docker Hub. Each registry digest was independently pulled
and passed packaged authentication, API, CLI, license, restart and CGQL checks.
The maintained Docker Hub overviews now provide exact pull commands.

Railway automatically deployed the same commit after CI. Hosted authentication,
CRUD/CGQL and exact pre/post deployment data equality pass. Phone-size login
centering, short-screen scrolling/validation and desktop rendering pass in
Chromium viewport emulation. [CG-74](../issues/CG-74.md) is resolved.

The [release report](../operations/docker-hub/release-2.7.11.md) records commits,
runs, digests, deployment and cleanup. Owned test containers/images were removed;
unrelated Docker resources and persistent data were preserved. This is
post-release documentation for v2.7.11, not another product release or image upload.
