# Read-only query plan errors (CG-38)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:160-167` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Read-only query plan errors (CG-38).** `/api/search/query` now returns HTTP
  400 for parsing and semantic-validation failures, matching `/api/query` for
  normal queries, EXPLAIN, and analysis. Typed backend/forbidden mappings are
  preserved. Formatting, strict Clippy, and all 955 reported Rust tests passed;
  eight unconfigured Arango entries early-returned. All 288 release HTTP/Lua
  checks passed across resident/paged Native stores, correcting 24 reproduced
  status mismatches. See the [verification report](../issues/query-error-status-2026-09-09.md).
