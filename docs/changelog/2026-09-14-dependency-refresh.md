# Refresh application dependencies and preserve stored-data compatibility

- Date: 2026-09-14
- Status: Unreleased
- Kind: Dependencies and compatibility

## Changes

[CG-83](../issues/CG-83.md) updates Argon2, redb, reqwest, Tantivy, tower-http,
the UI libraries/tooling and compatible transitive dependencies for local
candidate v2.7.15. Argon2's new API uses OS-generated salts while retaining PHC
password storage. The bounded Tantivy patch moves to the checksummed 0.26.2
release and preserves the fixed lru requirement. No freshness exception is added.

The updated Biome linter exposed three selector-order warnings. Reordering the
existing CSS rules/selectors clears them without changing their declarations.
Browser coverage now includes real graph rendering, resizing and view remounts
alongside login, document CRUD, access boundaries and CodeMirror execution.

The [refresh decision](../decisions/decision_dependency_refresh_2026_09_14.md)
records compatibility requirements and the optional ONNX maintenance review.

## Validation

The full shared CI and Docker suites pass for both editions, as do old-to-new
password/store compatibility, optional local ONNX inference and all 17 browser
cases. Freshness reports 58 current direct identities and no resolution drift
or exceptions; image scans report zero fixable findings. The
[verification record](../verification/dependency-refresh-2026-09-14.md) preserves
the evidence, unfixed findings and coverage boundaries. Test resources were
cleaned up. No commit, push, merge or package publication is claimed by this
unreleased record.
