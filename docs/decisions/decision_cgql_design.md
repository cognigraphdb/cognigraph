# Decision: CGQL is AQL/XQuery-shaped, one entry point, spec-first

Date: 2026-07-02 (heritage decision predates; formalized today) · Status: ACCEPTED

## Context
The founder's fluency is XQuery/AQL, not SQL. A query language survives by
matching its author's instincts and staying testable.

## Decision (owner: skitsanos)
FOR/FILTER/COLLECT/RETURN iteration shape; case-insensitive keywords,
case-sensitive identifiers; `graph.query()` as the single user-facing entry
point; every semantic documented in docs/cgql-v1.md before or with the code;
document-store null semantics (missing → null, null falsy).

## Outcome
The language grew from read-only v1 to LET, multi-key SORT, DISTINCT, full
COLLECT (AGGREGATE/INTO/COUNT), 30+ functions, and gated mutations in one
day without a single revert — because each addition landed against the spec
and the dual-engine corpus (see decision_dual_engine_corpus.md).
