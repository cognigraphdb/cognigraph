# API and operator contract parity (CG-22)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:196-206` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **API and operator contract parity (CG-22).** OpenAPI, architecture, operator
  configuration, and executable examples now cover all four durable job kinds,
  matching kind/input schemas, asynchronous drafting, side-view defaults and
  independent providers, and directed limits and replacement semantics.
  Corrected ambiguous inline YAML descriptions and a missing collection path
  parameter. Five new semantic drift tests join the existing route checks;
  all 15 focused checks and 57 release HTTP/CLI/schema checks passed. Formatting,
  strict Clippy, and all 950 reported Rust tests passed. Initial YAML-edit and harness
  header-casing errors were corrected before final verification. See the
  [report and validation evidence](../issues/api-contract-2026-09-09.md).
