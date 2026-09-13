# CGQL gains array helpers, date arithmetic and casts/regex (D6/D7/D8)

- Date: 2026-07-22
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:540-554` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CGQL gains array helpers, date arithmetic and casts/regex (D6/D7/D8).**
  `SLICE` (negative start counts from the end, negative length drops from the
  end, out-of-range clamps), `FLATTEN`, `INTERSECTION` and `MINUS` — the set
  operations use `UNIQUE`'s equality rule, so `1` and `1.0` are one value, and a
  non-array argument returns null rather than a silently empty result.
  `DATE_ADD(date, amount, unit)` shares `DATE_DIFF`'s units so the two are
  inverses; calendar units (months, years) are deliberately absent because they
  are not a fixed number of seconds. `TO_NUMBER` / `TO_STRING` / `TO_BOOL` and
  `REGEX_TEST` / `REGEX_REPLACE`, with patterns compiled once into a bounded
  cache since a `FILTER` calls the function once per row. Casts return **null**
  for unconvertible input rather than AQL's `0`/`""` — a silent 0 inside a `SUM`
  is a wrong answer, not a missing one. Note recorded against D7: `DATE_DIFF`
  already existed and was documented; the claim that it was missing came from
  reading one of the spec's two function tables instead of the registry.
