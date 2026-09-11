# CGQL gains `SORTED` and `SORTED_UNIQUE` (D9)

- Date: 2026-07-22
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:444-455` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CGQL gains `SORTED` and `SORTED_UNIQUE` (D9).** Found when a ranked
  evaluation query needed "the first three of a UNIQUE set" and there was no
  way to sort an array first — the question had no single right answer. The
  design constraint was self-consistency: same-type ordering is exactly the
  SORT clause's (numbers by value, strings byte-wise, `false < true`), pinned
  by a test asserting `SORTED` matches the clause's output; dedup equality is
  exactly `UNIQUE`'s, so `1` and `1.0` are one value with the first of each
  tie kept. The SORT clause leaves mixed types unordered, but a function
  returning an array cannot, so mixed types take AQL's ladder:
  null < bool < number < string < array < object. Non-array input is null,
  matching the D6 array helpers.
