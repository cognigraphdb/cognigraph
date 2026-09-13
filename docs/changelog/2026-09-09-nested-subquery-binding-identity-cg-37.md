# Nested subquery binding identity (CG-37)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:168-177` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Nested subquery binding identity (CG-37).** One generated-name counter spans
  each parsed query, preventing nested/sibling expressions from shadowing outer
  synthetic bindings. Correlation, user-variable restrictions, and depth/budget
  limits are preserved. Five execution fixtures and three new Rust tests cover
  the regression. Formatting, strict Clippy, and all 953 reported Rust tests
  passed; eight unconfigured Arango entries early-returned. The saved release
  reproduced 40 collisions; the rebuilt release corrected them. Its 180 HTTP/Lua
  observations include four known CG-38 status mismatches. See the
  [verification report](../issues/subquery-bindings-2026-09-09.md).
