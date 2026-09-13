# Subquery specification consistency (CG-28)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:178-187` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Subquery specification consistency (CG-28).** Replaced obsolete LET-only
  exclusions with supported positions and exact grouping/mutation restrictions.
  Eighteen corpus cases now back the examples. Formatting, strict Clippy, all
  950 reported Rust tests passed; 148 release observations matched the documented
  contract and known failures. A nested generated-name collision (CG-37) and
  read-only HTTP error classification (CG-38) were recorded separately;
  CG-37 and CG-38 are resolved above. No Rust implementation changed
  in this batch. See the
  [verification report](../issues/subquery-contract-2026-09-09.md).
