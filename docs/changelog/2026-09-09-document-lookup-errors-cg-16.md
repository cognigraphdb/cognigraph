# Document lookup errors (CG-16)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:285-293` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Document lookup errors (CG-16).** Backend failures in `DOCUMENT()` abort
  normal and analyzed queries instead of silently returning null. HTTP query
  routes and uncaught Lua errors preserve forbidden/connection failures as
  403/503; other backend failures remain 500. Missing documents still return
  null, and Lua pcall can handle failures. Formatting, Clippy, all 907 tests,
  injected fault/status checks, 72 release forbidden-lookup cases, 72 controls,
  plain EXPLAIN, Lua catching, and resident/paged restarts passed. See
  [error semantics and verification](../issues/document-errors-2026-09-08.md).
