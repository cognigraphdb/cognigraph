# Require requested Railway tests and clean local Docker resources

- Date: 2026-09-13
- Status: v2.7.12
- Kind: Engineering workflow

## Behavior

Railway testing now explicitly requires the owner's request for a hosted QA
session. Builds, CI, pushes and releases use local verification without starting
or testing Railway. Local container checks can emulate its mount behavior.

The [Docker cleanup guide](../operations/docker-cleanup.md) and agent instructions
require removing CogniGraph test containers, images, networks and disposable
volumes after every session. Only identified reusable Evidence data volumes may
be retained from those tests. The owner scoped cleanup to CogniGraph; unrelated
projects and unidentified data are preserved pending ownership identification.

## Verification

This updates workflow documentation. No Railway tests, database deployment or
runtime behavior changes are part of it. Documentation and decision checks
validate the amended guides; local Docker inventory verifies scoped cleanup.
