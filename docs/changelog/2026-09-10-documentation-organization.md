# Documentation organization

- Date: 2026-09-10
- Status: Unreleased
- Kind: Documentation

## Change

Group engineering guides by purpose, reduce the root README, and move product,
sales, papers and publishing assets into the optional sibling product docs
directory. Preserve editorial drafts and dated delivery history in archives.
Split the old changelog into 105 dated records and add index/link validation.
Update necessary skill/CI paths while preserving release approval gates.

The relocated sales exporter now resolves its directory, detects all 11 slides
and uses an isolated browser profile. Existing publication assets and sealed
experiments retain their original bytes.

## Validation

See the [migration and verification report](../plans/documentation-migration-2026-09-10.md)
for hash checks, source reconciliation, changelog body preservation, registry
counts, real-server HTTP verification and local paper/deck build checks.
