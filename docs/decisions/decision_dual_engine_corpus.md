# Decision: File-driven CGQL corpus executed on BOTH engines

Date: 2026-07-02 (M4) · Status: ACCEPTED

## Context
Rich edge-case testing was demanded explicitly; two executors (in-memory +
GraphBackend) risk semantic drift.

## Decision
`crates/cognigraph-query/tests/corpus/` holds ~170 `.cgql` files
(parse_ok / parse_err / validate_err / exec + expected JSON + shared
dataset in import_json format, per-file `// binds:` headers). Exec cases run
on the in-memory executor AND through `NativeBackend::query()`, byte-identical
results required. Dropping a file in extends coverage; no Rust changes.

## Outcome
Caught, among others: the int/float numeric-equality bug, a silently
no-op'd COLLECT AGGREGATE executor edit, and one wrong expectation of the
author's (traversal cycle-guard). Multilingual and Unicode semantics are
pinned by corpus files rather than prose.
