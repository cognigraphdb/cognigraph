# Decision: LIMIT operands may be bind variables, resolved before execution

Status: accepted and implemented 2026-09-22 for [CG-85](../issues/CG-85.md).

## Context

CGQL accepted only literal integers in `LIMIT`, so paginated application
queries had to splice user-controlled numbers into query text, the one place
the bind-variable design did not protect. Migrating AQL callers already
write `LIMIT @offset, @count`.

The plan is built without bind values; values are only seen at execution,
after the bind-variable presence check and before the first scan. Vector
thresholds already resolve bind names at that point.

## Decision

1. **Grammar.** Each `LIMIT` operand is an unsigned literal or a bind
   variable: `LIMIT @n`, `LIMIT @o, @n`, `LIMIT 1, @n`. Expressions
   (`@n + 1`, `-@n`, `d.count`) and a third operand stay parse errors.
2. **Values.** A bound operand must be a JSON number that is a non-negative
   integer. Strings such as `"5"`, floats including `2.0`, negatives,
   booleans, null, arrays and objects are rejected with
   `LIMIT count bind variable `@n` must be a non-negative integer` (or
   `offset`). Strictness was chosen over coercion so a client bug cannot
   silently page wrongly.
3. **Same rules as literals.** A bound count of zero and a bound count above
   the configured maximum raise the existing `ZeroLimit` and
   `LimitTooLarge` errors. The plan now carries the maximum it was validated
   against, so the same ceiling applies at execution. Offsets keep their
   literal semantics: any non-negative integer, including past the end.
4. **Before any storage access.** Presence of every bind variable is checked
   first, exactly as today (`missing bind variables: n` wins over an invalid
   offset). Then every `LIMIT` in the plan tree, including `LET` subquery
   plans, is resolved before the first scan. The errors are plan-validation
   errors, so the HTTP routes keep returning 400 for them.
5. **Pushdown is unchanged.** A bound `LIMIT` is pushed to the backend under
   the same conditions as a literal (single `FOR`, no `SORT`/`COLLECT`, all
   filters pushed), with the resolved offset plus count; vector search sizes
   its candidate list the same way.
6. **EXPLAIN renders names.** Plain `EXPLAIN` takes no bind values, so the
   pipeline shows `LIMIT @o, @n` and pushdown fields show `"@o + @n"`;
   literal forms render exactly as before (`LIMIT 3, 2`, `5`). `EXPLAIN
   ANALYZE` executes with the values but keeps the same rendering.
7. **AST shape.** Literal operands keep their bare JSON number; a bound
   operand serializes as `{"bind": "name"}`; both names appear in
   `bind_vars`. The serialized plan gains `max_limit`.

## Consequences

- Clients reuse one query string across pages and stop interpolating.
- The corpus gains parse-ok, parse-error and execution cases for the bound
  forms; both engines run them. Error paths and pushdown are covered by
  dedicated test files.
- The CGQL reference and the AQL migration guide describe the new forms;
  the guide no longer says to interpolate client-side.
- Revisit if a caller needs coercion of numeric strings, or a configured
  maximum offset; both were deliberately left out.
