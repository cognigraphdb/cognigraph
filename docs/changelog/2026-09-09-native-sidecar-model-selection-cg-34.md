# Native sidecar model selection (CG-34)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:235-244` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Native sidecar model selection (CG-34).** Base and delta vectors match the
  requested model before candidate truncation in both sidecar configurations.
  Model-only writes, batch updates, field removal, deletion/recreation, and
  rebuilds preserve filtering without changing the sidecar file format.
  Formatting, strict Clippy, all 928 reported Rust tests, and 162 release
  HTTP/Lua checks passed, including stores seeded by the previous release and
  server restarts. An initial harness assumption about restart file reuse
  exposed unnecessary startup revision changes, tracked separately as CG-35.
  See the [verification report](../issues/sidecar-model-filter-2026-09-09.md).
