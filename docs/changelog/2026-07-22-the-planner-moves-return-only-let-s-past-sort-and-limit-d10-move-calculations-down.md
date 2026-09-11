# The planner moves RETURN-only `LET`s past SORT and LIMIT (D10, move-calculations-down)

- Date: 2026-07-22
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:472-487` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **The planner moves RETURN-only `LET`s past SORT and LIMIT (D10,
  move-calculations-down).** A `LET` whose variable feeds nothing but the
  projection now runs after the tail has decided which rows survive — for the
  motivating evaluation query, that meant three enrichment subqueries running
  for 5 rows instead of 651. `COLLECT` and mutations disable the pass; blocking
  is over-approximated, so a shadowed or ambiguous name merely fails to defer.
  Deferred ops keep their positional stage and site ids, so EXPLAIN ANALYZE
  accounting and the materializer are unchanged, and EXPLAIN labels them
  `[deferred past LIMIT]`. Deferral also skips evaluation on rows LIMIT
  discards, so an expression that would have errored on a discarded row no
  longer runs — strictly fewer errors, the same trade AQL makes. A backend test
  pins the observable effect: a deferred `DOCUMENT()` under `LIMIT 2` fetches
  exactly the two survivors' targets. The worst evaluation query went from
  6.7x behind the reference backend to 1.6x, and the 8-query board total to
  0.86x — net faster.
