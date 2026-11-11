# v2.7.55 — Compatible dependency drift before the P1 batch is qualified

- Date: 2026-11-11
- Status: v2.7.55
- Kind: Dependencies

## Changes

The freshness gate refused the v2.7.44–v2.7.54 batch because three
compatible resolutions moved after the v2.7.43 refresh earlier the same day.
Two were found when the batch was qualified and the third when the push gate
ran half an hour later:

- Cargo: `annotate-snippets` 0.12.17 → 0.12.18 (transitive).
- UI: `compute-scroll-into-view` 3.1.1 → 3.2.0 (transitive, through
  `scroll-into-view-if-needed`).
- UI: `scroll-into-view-if-needed` 3.1.0 → 3.2.0 (transitive).

`cargo update` and `bun update --lockfile-only --ignore-scripts` changed
only these three identities. No direct dependency requirement, feature or
source changed, and the TypeScript client lockfile is unchanged.

The workspace version moves to 2.7.55.

## Validation

`scripts/dependency_freshness.py` passes on these lockfiles. The full CI and
Docker suites passed on the candidate with the first two identities, and the
push gate reruns both on the final lockfiles before the branch leaves the
machine. Nothing was pushed, released or
deployed.
