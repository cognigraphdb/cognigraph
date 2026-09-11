# Traversal confidence parity (CG-19)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:255-263` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Traversal confidence parity (CG-19).** Arango applies the inclusive
  confidence threshold to every edge, including before the minimum depth,
  defaults missing/nonnumeric confidence to 1.0, and preserves depth zero.
  Shared contract cases, six live ArangoDB 3.12.11 integration tests, formatting,
  Clippy, all 923 workspace tests, and 240 release HTTP/Lua checks passed across
  Arango and all persistent Native configurations, including server restarts.
  The pre-fix release reproduced 48 Arango mismatches across the two server
  lifetimes. See the [verification report](../issues/traversal-confidence-2026-09-09.md).
