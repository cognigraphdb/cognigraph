# Bounded dependency patch

CG-83 refreshes the bounded CG-70 patch to the **Tantivy 0.26.2** release,
which still needs the `lru ^0.18.2` requirement correction.
The lockfile selects the same fixed lru 0.18.4 as CogniGraph's direct cache.
This is the one-line dependency change accepted upstream in
[commit 5ca39332002c2c87fb5d2abc707cf527b3319d42](https://github.com/quickwit-oss/tantivy/commit/5ca39332002c2c87fb5d2abc707cf527b3319d42).
Our manifest-only patch changes no Tantivy Rust source or storage format.
The released 0.26.2 aggregation/scorer fixes are included without local changes.

The snapshot preserves the published release archive's files, including its
MIT license, except for its unused standalone Cargo.lock. The root Cargo.lock
owns workspace resolution. The published Cargo.toml.orig is retained unchanged
as provenance; Cargo builds with the patched normalized Cargo.toml.
`tantivy-0.26.2.provenance.json` records the archive URL, registry checksum and
original hashes. The archive checksum was verified before extraction.

`python3 scripts/check-vendored.py` verifies all release bytes offline after
reversing exactly the allowed manifest change. It runs in the shared CI suite.
Treat provenance changes as dependency changes requiring review; checksums do
not substitute for review of a changed manifest or provenance record.
The snapshot is excluded from workspace membership and first-party formatting
and lint ownership, but is built as the actual Native dependency in both
editions and Docker images.

Remove the snapshot, provenance, patch declaration, workspace exclusion,
Docker copy and integrity check when a reviewed published Tantivy release uses
fixed lru. Verify the stored-document cache, Native text search/persistence,
both Rust editions and packaged images before retiring it. Do not move to the
unreleased Tantivy main branch just to remove this patch.

The maintained dependency strategy and optional ONNX maintenance exception are
recorded in [CG-70](../docs/issues/CG-70.md) and the
[CG-83 refresh decision](../docs/decisions/decision_dependency_refresh_2026_09_14.md).
