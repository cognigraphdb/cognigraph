# First issue-registry fixes (CG-2, CG-4, CG-9, CG-17, CG-18)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:413-422` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **First issue-registry fixes (CG-2, CG-4, CG-9, CG-17, CG-18).** Native text
  and vector derivatives use safe hashed filenames; BM25 index identity is
  structured and checked against persisted schema metadata. Duplicate or
  reserved text-search fields return validation errors. Directed construction
  rejects malformed provider output without replacing stored facts. Keyed
  UPSERT now checks every search predicate. The release server passed synthetic
  loopback HTTP regressions in resident/paged modes, including restarts and
  prior-fact preservation. See [verification and compatibility details](../issues/fixes-2026-09-08.md).
  Old derivative filenames are left untouched and rebuilt under new names on demand.
