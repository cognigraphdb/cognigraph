# v2.7.55 — Compatible dependency drift before the P1 batch is qualified

- Date: 2026-11-11
- Status: v2.7.55
- Kind: Dependencies

## Changes

The freshness gate refused the v2.7.44–v2.7.54 batch because two compatible
resolutions had moved since the v2.7.43 refresh earlier the same day:

- Cargo: `annotate-snippets` 0.12.17 → 0.12.18 (transitive).
- UI: `compute-scroll-into-view` 3.1.1 → 3.2.0 (transitive, through
  `scroll-into-view-if-needed`).

`cargo update` and `bun update --lockfile-only --ignore-scripts` changed
only these two identities. No direct dependency requirement, feature or
source changed, and the TypeScript client lockfile is unchanged.

The workspace version moves to 2.7.55.

## Validation

`scripts/dependency_freshness.py` passes on these lockfiles. The full CI and
Docker suites run on this commit as the candidate for the P1 batch; their
result is recorded with the batch, not here. Nothing was pushed, released or
deployed.
