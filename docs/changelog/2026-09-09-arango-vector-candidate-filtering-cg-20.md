# Arango vector candidate filtering (CG-20)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:245-254` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Arango vector candidate filtering (CG-20).** Indexed searches filter model
  identity before candidate truncation. Indexed and fallback searches expand
  candidates until enough distinct parents are found or candidates are
  exhausted. Filtered indexed queries require ArangoDB ≥3.12.6; older servers
  can select fallback. Formatting, strict Clippy, all 925 Rust tests, eight live
  Arango integration tests, and 88 release HTTP/Lua checks passed, including
  server restarts and an actual vector-index query plan. Native sidecar model
  filtering was reproduced separately and is resolved above as CG-34. See the
  [verification report](../issues/vector-model-filter-2026-09-09.md).
