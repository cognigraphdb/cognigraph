# CGQL gains a conditional expression, identifier helpers, and two AQL reflexes (D3/D4/D5)

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:555-578` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CGQL gains a conditional expression, identifier helpers, and two AQL
  reflexes (D3/D4/D5).** `cond ? then : else` plus `COALESCE`/`NOT_NULL` — a
  non-boolean condition takes the else branch, the same rule `FILTER` uses,
  rather than inventing truthiness. `IS_SAME_COLLECTION(collection, doc_or_id)`
  and `PARSE_IDENTIFIER(doc_or_id)` give a heterogeneous edge collection a real
  discriminator instead of `STARTS_WITH(e._to, "persons/")`, and both accept an
  id string or a document. Shorthand object fields (`{ x }` = `{ x: x }`) and
  bare `COLLECT WITH COUNT INTO n` now parse. On the evaluation workload the
  null-defaulting query could finally be written with the same semantics as the
  reference backend instead of excluding null-valued rows.
  **Function-over-subquery** (`LENGTH((FOR …))`) now works too, by desugaring:
  a subquery in expression position is lifted into a synthetic `LET` before the
  clause that uses it, so the validator, planner and executor still only see the
  LET-position shape — no new execution form, correlation analysis unchanged, and
  a test asserts the sugared query returns exactly what the hand-written LET
  returns. Synthetic bindings are named `$sqN`; `$` is not a legal identifier
  character, so they cannot collide with a user variable. A subquery in `SORT` or
  `RETURN` **after** a `COLLECT` is refused with an explanation instead of lifted,
  because `COLLECT` drops row variables and hoisting past it would silently
  answer from the wrong scope. This narrows the "subqueries outside LET values"
  exclusion in `decision_cgql_v2.md`, amended there.
  **Still not** included: `DOCUMENT()`, which needs a backend fetch the
  pure-function registry cannot express.
