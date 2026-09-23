# v2.7.32 — Commit the dump fixtures' dotfile case

- Date: 2026-09-23
- Status: v2.7.32
- Kind: Fixtures and CI

## Changes

The first remote CI run of the CG-64/CG-66 integration failed
`scripts/check-arangodump-fixtures.py`: the `derived/dotfile` fixture's
`.DS_Store` matches the repository `.gitignore`, so it was never committed,
while local checks passed because the file existed on disk
([CG-64](../issues/CG-64.md)).

- The fixture's dotfile is now `.hidden`; the contract covers every file
  starting with `.`, not that name. The manifest, the fixture's expected
  `ignored` list and the generator changed accordingly; no other fixture bytes
  changed.
- The validator now fails when any manifest-listed fixture file is ignored by
  git, so the same mistake is caught before a push.

The workspace version moves to 2.7.32.

## Validation

All 25 fixtures pass the validator and the importer's fixture parity test. A
new script test proves the guard reports an ignored fixture file and stays
quiet otherwise.
