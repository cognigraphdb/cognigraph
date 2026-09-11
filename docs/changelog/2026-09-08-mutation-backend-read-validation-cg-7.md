# Mutation backend-read validation (CG-7)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:305-314` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Mutation backend-read validation (CG-7).** Queries containing mutations
  reject `DOCUMENT()` and correlated traversal starts before any write,
  including nested read subqueries and OLD/NEW return expressions. Full
  dynamic-read support inside mutations remains future capability work. The
  query endpoint returns HTTP 400 for parsing/planning validation failures.
  Formatting, Clippy, all 893 tests, and 256 release HTTP/Lua rejection cases
  passed, with unchanged documents, supported mutation lifecycles, and
  resident/paged restart checks. The initial live run caught a corrected
  500/400 mapping defect. See [verification and compatibility](../issues/mutation-backend-reads-2026-09-08.md).
