# CG-9: UPSERT ignores additional match fields when _key is present

- Status: Resolved
- Priority: P2
- Area: CGQL mutation correctness
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Live HTTP reproduction

## Problem

The UPSERT executor uses direct key lookup when the search object includes `_key`, but does not check the remaining search fields against the fetched document. It can update a document that does not match the caller's search object, changing the wrong record instead of taking the INSERT branch.

## Evidence

- `crates/cognigraph-query/src/executor/mutation.rs:110-151` — `_key` branch versus all-fields scan.
- `docs/cgql-mutations-design.md` — UPSERT search-object semantics.

## Reproduction / failure sequence

With existing `notes/a` containing `v:1`, run `UPSERT {_key:"a",v:999} INSERT {_key:"c",v:3} UPDATE {wrong_match:true} IN notes RETURN NEW._key`. The endpoint returned `a` and updated it, although its `v` did not match 999.

## Acceptance criteria

- [x] Apply every search predicate after the optimized key lookup.
- [x] Keep comparison semantics identical between indexed and scanned matching.
- [x] Cover matching/nonmatching extra fields, null/missing values, and numeric equality without mutating nonmatching records.

## Resolution — 2026-09-08

Keyed and scanned UPSERT use one all-fields predicate. Tests exercise both paths
with matching/mismatching numbers and null/missing fields and assert that
nonmatching documents remain unchanged. The live release-server reproduction
now inserts `c` and leaves `a` untouched. The CGQL reference documents the
corrected matching semantics. All Rust gates passed; see
[batch verification](fixes-2026-09-08.md).

Run the repository Rust gates and a focused runtime regression before closing this issue. See [review evidence](review-2026-09-08.md).
