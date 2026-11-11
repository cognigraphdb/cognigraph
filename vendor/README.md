# Vendored dependencies

Nothing is vendored at present.

The bounded Tantivy snapshot kept here from CG-70 to CG-192 carried a one-line
dependency-manifest correction (`lru ^0.18.2`). Tantivy 0.27.0 publishes that
requirement itself, so the snapshot, its provenance record, the Cargo patch
declaration, the Docker copy and the integrity check were removed on
2026-11-11 under the [refresh decision](../docs/decisions/decision_dependency_refresh_2026_11_11.md).
The Git history keeps the snapshot and its provenance.

A future patch of this kind needs its own reviewed decision: a published
release archive verified against the registry checksum, a recorded provenance
file, the smallest possible change, an integrity check in the shared CI suite
and stated removal criteria. Do not vendor an unreleased upstream branch.
