# CGQL gains `DOCUMENT()`, completing D4

- Date: 2026-07-22
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:519-539` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CGQL gains `DOCUMENT()`, completing D4.** `DOCUMENT("persons/p1")` resolves a
  document by identifier in any expression position, and accepts an array of ids
  (results in the same order). With `COALESCE` and `IS_SAME_COLLECTION` it makes a
  heterogeneous edge collection — one whose `_to` points into several collections
  — readable in a single expression, which no `FOR` can express. Missing and
  non-identifier arguments both answer null.

  Row evaluation is synchronous, so calling an async backend mid-row was the
  design problem. Rather than make every expression path async, or give the
  `fn(&[Value])` registry a backend handle for one function, a run records the
  ids it could not resolve, fetches them in one batch, and repeats: reads are
  side-effect free, each distinct id costs one fetch for the whole query however
  many rows ask for it, and nested `DOCUMENT(DOCUMENT(x)._id)` converges because a
  later round sees the earlier round's ids. Four rounds are allowed, then the
  query is rejected rather than looped on; any wall-clock budget spans all rounds.
  A plan takes the retry path only if it actually calls `DOCUMENT`.

  **Postfix field access** shipped alongside, because it had to: `DOCUMENT(e._to).name`
  did not parse — the grammar allowed `.field` only off a variable. A field can now
  be read off any parenthesised expression or call result.
