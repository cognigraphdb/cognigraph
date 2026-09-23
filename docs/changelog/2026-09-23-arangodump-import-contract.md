# v2.7.30 — ArangoDB dump import contract and fixtures

- Date: 2026-09-23
- Status: v2.7.30
- Kind: Migration contract, fixtures and CI

## Changes

The offline ArangoDB dump import is now a frozen, fixture-qualified contract
([CG-64](../issues/CG-64.md), [contract](../reference/arangodump-import.md),
[decision record](../decisions/decision_arangodump_import_contract.md)). No
importer ships yet; [CG-66](../issues/CG-66.md) builds it against this.

- `fixtures/arangodump/` holds 25 fixtures: 15 real `arangodump` runs from
  `arangodb:3.11.14` and `arangodb/enterprise:3.12.11` over invented data
  (gzip, plain, split, envelope, system collections, a dataset of refusable
  problems, structure-only, all-databases, VPack, encrypted) and 10 recorded
  corruptions of them. `manifest.json` pins image digests, options, expected
  outcomes and every file's SHA-256; `expected/` holds results derived from the
  dataset definitions alone.
- `scripts/arangodump_reader.py` is the executable contract: support matrix,
  layout rules, name rules read from the server sources, document and edge
  mapping, unique indexes carried over as enforced constraints, every other
  metadata item reported, limits and error codes.
- `scripts/check-arangodump-fixtures.py` verifies fixture bytes, the expected
  results and each fixture's outcome; it joins the shared CI suite.
  `scripts/arangodump_fixtures.py` regenerates the fixtures with Docker.
- Findings from real exporter output changed the original design: 3.12 has no
  envelope option, split dumps omit empty collections' data files, VPack has
  its own suffix, encryption hides `dump.json`, and structure-only dumps import
  as empty collections with a warning because they cannot be detected.
- The AQL migration guide, implementation plan and original design now point
  to the contract.

The workspace version moves to 2.7.30.

## Validation

All 25 fixtures reach their recorded outcomes through the reference reader.
Nineteen script tests cover symlinks, stray directories and files, orphan
data, corrupt metadata and structure files, VPack by file name, split parts
and split-mode empty collections, record/expansion/file-count limits, key and
edge shape, blank lines, unresolved edges, index dispositions, name rules and
unique-value semantics, and validator tamper detection. No credentials or
real data appear in the fixtures; the distribution guard passes.
