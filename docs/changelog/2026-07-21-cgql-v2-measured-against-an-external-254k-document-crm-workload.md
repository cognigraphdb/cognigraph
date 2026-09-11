# CGQL v2 measured against an external 254k-document CRM workload

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:611-627` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CGQL v2 measured against an external 254k-document CRM workload.** First CGQL
  run on a third party's dataset with their questions, head-to-head against a mature
  graph database. Single-collection aggregations are competitive and return
  byte-identical answers; **every question needing a per-entity graph hop takes minutes
  or does not complete**. The blocker is a planner gap rather than the language: the
  edge index exists and literal-start traversal uses it (**8 ms**), but an equality
  filter on `_from`/`_to` inside a correlated subquery falls back to a full collection
  scan (**142 ms per outer row** — ~64 minutes across 27k rows). Compounding it,
  traversal sources must be the outermost first `FOR`, so traversal is indexed but
  cannot be correlated while correlation is possible but not indexed — and real
  workloads need the intersection. Secondary gaps: no conditional expression
  (ternary/`COALESCE`), no identifier helpers for heterogeneous edge collections, and
  three AQL syntax reflexes that fail with parse errors an evaluator reads as missing
  capability. Prioritized D1–D8 in
  `docs/decisions/decision_cgql_v2_workload_gaps.md`; nothing in
  `decision_cgql_v2.md` is reversed.
