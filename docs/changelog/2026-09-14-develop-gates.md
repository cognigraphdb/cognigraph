# Require dependency freshness and incoming-work review before integration

- Date: 2026-09-14
- Status: Unreleased
- Kind: CI and workflow policy

## Changes

[CG-82](../issues/CG-82.md) adds a read-only Cargo/UI freshness gate, using
current direct release channels and fresh compatible transitive resolutions.
Exact-version, dated owner decisions are required for intentional direct pins;
they cannot suppress advisory findings or compatible resolver drift.

The local push hook and GitHub workflow share incoming-PR accounting, capture
reviews/checks, reject outstanding requested changes on included work, and
recheck candidate/develop/PR identities after verification. Source CI and the
Docker job enforce the checks before the required aggregate can pass. The
[develop gate guide](../operations/develop-gate.md) and agent/release instructions
make the final pre-merge recheck explicit.

## Validation and activation

Live GitHub inspection finds no incoming PRs. The dependency check correctly
rejects five outdated direct Rust dependencies, six direct UI dependencies and
compatible transitive drift. It leaves candidate manifests and lockfiles intact.
[CG-83](../issues/CG-83.md) owns the refresh; the newly required CI gate is
intentionally failing until that work is qualified. All 112 script regressions,
Actionlint, the release-skill validator and documentation checks pass for the
gate implementation independently.
The changes have not been committed, pushed or activated on GitHub.
