# M5 — CGQL LET / multi-key SORT / DISTINCT / date functions

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Executed inline 2026-07-02.

**Goal:** Everyday-ergonomics CGQL features, all corpus-tested on both engines.

- [x] Task 1 — Grammar+AST: `LET name = expr` (interleavable with FILTER, evaluated per row in order before filters), `SORT key [dir], key [dir], ...` (`SortClause { keys: Vec<SortKey> }`), `RETURN DISTINCT expr` (`Query.distinct`), new reserved keywords LET/DISTINCT with boundary guards.
- [x] Task 2 — Validation: LET names extend scope in order (reserved/duplicate checked); sort keys validated individually.
- [x] Task 3 — Executor: per-row LET evaluation before filters; lexicographic multi-key sort with per-key direction; DISTINCT dedupe via numeric-aware `values_equal` after projection; `NOT`/`AND`/`OR` treat null as false (consistent with filters).
- [x] Task 4 — Date functions (chrono): `NOW()` (RFC3339), `DATE_YEAR/MONTH/DAY/HOUR/MINUTE/SECOND` (integers; accepts RFC3339 or `YYYY-MM-DD`), `DATE_TIMESTAMP` (unix millis), `DATE_DIFF(a, b, unit)` (signed, fractional; days/hours/minutes/seconds). Bad input → null.
- [x] Task 5 — Corpus: ~16 new files across exec (LET chains, LET in sort/return, multi-key sort with ties, DISTINCT incl. null, null-falsy booleans, date parts/diff/timestamp, NOW smoke via TYPENAME), parse_ok, parse_err, validate_err. Both engines must agree.
- [x] Task 6 — Spec (`docs/cgql-v1.md`), implementation-plan M5 row, full validation, commit.

**Decisions:** LET bindings are per-row and evaluated in declaration order *before* any filter (interleaved syntax is allowed but has no lazy-evaluation semantics — documented). NOW() is intentionally not exactness-tested in the corpus.
