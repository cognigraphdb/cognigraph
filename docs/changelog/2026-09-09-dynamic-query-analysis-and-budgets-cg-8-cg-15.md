# Dynamic query analysis and budgets (CG-8 / CG-15)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:294-304` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Dynamic query analysis and budgets (CG-8 / CG-15).** Normal execution and
  EXPLAIN ANALYZE share document/traversal resolution. Reports separate settled
  rows from attempted retry work and expose backend fetch counts. One row
  counter spans materialization, distinct document lookups, correlated path
  fetches, and runtime source expansions across retries. COLLECT INTO document
  projections now resolve correctly. Formatting, Clippy, all 901 tests, 60
  release analysis-parity comparisons, 240 budget-boundary executions, and
  resident/paged restart checks passed. Configured caps may now reject dynamic
  queries that previously received a fresh allowance per phase. See
  [accounting semantics and verification](../issues/dynamic-query-2026-09-08.md).
