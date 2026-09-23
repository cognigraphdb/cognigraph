# v2.7.25 — Bind variables in LIMIT

- Date: 2026-09-22
- Status: v2.7.25
- Kind: CGQL language

## Changes

`LIMIT` operands may now be bind variables: `LIMIT @n`, `LIMIT @o, @n` and
mixed forms such as `LIMIT 1, @n`. Bound values must be non-negative integer
JSON numbers and are resolved before any storage access, after the existing
bind-variable presence check; a bound count of zero or above the configured
maximum raises the same errors as a literal. Pushdown and vector candidate
sizing use the resolved values under unchanged conditions. Plain `EXPLAIN`
renders the bind names (`LIMIT @o, @n`, `fetch_limit: "@o + @n"`); literal
forms render as before. Expressions and a third operand remain parse
errors. [CG-85](../issues/CG-85.md) is resolved under the
[decision record](../decisions/decision_limit_bind_variables.md).

AST: a bound operand serializes as `{"bind": "name"}`, literals keep their
number shape, and the serialized plan gains `max_limit`. The CGQL reference
and the AQL migration guide describe the new forms. The workspace version
moves to 2.7.25.

## Validation

Corpus cases cover parse-ok, parse-error and execution forms (count, offset
and count, literal offset with bound count, zero offset, one bind reused,
a bind inside a `LET` subquery, after `COLLECT`, and a bind shared with the
projection) on both engines. A dedicated test file covers missing and
unexpected binds, every rejected value type for count and offset, zero and
too-large counts, offsets past the end, EXPLAIN rendering and EXPLAIN
ANALYZE. Backend-executor tests cover pushdown of the resolved value, the
unchanged disabling conditions, and failure before any scan. Full local
gates run before the local merge. Not a release.
