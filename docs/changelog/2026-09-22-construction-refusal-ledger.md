# v2.7.22 — Construction gate refusals are recorded in a ledger

- Date: 2026-09-22
- Status: v2.7.22
- Kind: HTTP contract and storage

## Changes

`POST /api/construct/directed` and `POST /api/construct/propose` returned
their gate rejections only as strings in the response; the nominated triple,
chunk and evidence a gate judged were discarded. [CG-90](../issues/CG-90.md)
resolves that under the
[refusal ledger decision](../decisions/decision_construction_refusal_ledger.md):

- The directed gate returns structured `Refusal` records with a stable gate
  code per gate (`cognigraph_construct::refusals::DirectedGate`); proposal
  skips carry a gate code. Reason text is unchanged.
- Both responses keep `skips` / `skipped` exactly as before and add
  `refusals[]` rows with their stored keys, plus `refusals_stored` and
  `refusals_dropped`. A ledger write failure sets `refusals_error` and never
  fails the construction that already happened.
- Rows are written in the same request to the new generated collection
  `construction_refusals`: readable through generic reads, CGQL and
  `cognigraph export`, publicly write-protected, outside promotion and
  attestation. Keys are deterministic over origin, space, gate, triple,
  chunk and evidence, so a resubmitted nomination records once.
- Each tenant store keeps at most 10,000 rows; at the cap new refusals are
  reported but not stored. No retention sweeper exists yet.

The dataops guide documents the row shape, gate codes and CGQL queries per
space and per gate. The directed module's inline tests moved to
`directed_tests.rs`. The workspace version moves to 2.7.22.

## Validation

Construct-crate tests cover each directed gate, canonical (NFC) fields,
duplicate nominations and refusal order. Ledger tests cover deterministic
keys and their partitioning, first-writer-wins immutability, in-batch
duplicates, the cap boundary (drop newest, existing rows preserved, a known
row at the cap still counted as stored, a zero cap), optional and Unicode
fields, the generated read-only guard and export inclusion, and the
error-path of the response attach. Route tests cover directed refusals with
unchanged skip strings, zero-refusal requests writing nothing, a request
grounding nothing, resubmission, and proposal duplicate-id and validation
refusals. Full local gates run before the local merge. Not a release.
