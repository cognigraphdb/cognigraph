# 2026-07-03 — M12: UPSERT + atomic batch transactions

- Date: 2026-07-03
- Status: Historical
- Kind: History
- Date source: Original section heading
- Source: `CHANGELOG.md:2289-2317` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- CGQL `UPSERT search INSERT doc UPDATE merge IN collection` with NEW/OLD:
  `_key` fast path, otherwise first all-fields match in key order; update
  branch merges, insert branch creates as-is.
- `GraphBackend::execute_batch` capability (default: unsupported): the
  native backend validates the whole batch, commits it in ONE redb
  transaction, and applies memory/cache/sidecar only after — proven
  all-or-nothing by a mid-batch-conflict rollback test; later ops see
  earlier ops' effects. Exposed as `POST /batch` (documents:write scope).
- M7's two deferrals are now closed.
- **Unicode decisions closed**: `SORT ... COLLATE "de"` locale collation
  (ICU4X) and default NFC normalization (query literals at parse, stored
  strings at write) — the corpus pins flipped deliberately. New function:
  `COSINE_SIMILARITY(a, b)`.
- **Lua surface walkthrough**: bindings caught up with the backend —
  `graph.update_document`, `graph.replace_document`, `graph.text_search`
  (BM25), and `graph.batch` (atomic); direction typos now error instead
  of silently meaning outbound; binding errors carry the operation name;
  `LUA_INSTRUCTION_LIMIT` makes the instruction budget configurable.
  Python-version benchmarking dropped from the plan (research-grade
  original, not a meaningful baseline).
- **Per-query execution budgets**: `ExecutionBudget` (source-row cap +
  wall-clock deadline checked per row) on the CGQL executor, applied to
  both HTTP CGQL endpoints via `CGQL_MAX_SOURCE_ROWS` /
  `CGQL_TIME_BUDGET_MS`. Docs revision: spec retitled, stale UPSERT
  removed from the unsupported list, examples caught up (CGQL-first Lua,
  UPSERT//batch//auth/login API examples).
