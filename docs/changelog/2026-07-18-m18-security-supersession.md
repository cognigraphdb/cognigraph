# M18 security supersession

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1162-1171` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M18 security supersession.** Public opaque backend-native query text is no
  longer an Admin capability: a live backtick Unicode-escape bypass proved
  textual AQL screening unsuitable as a security boundary.
  `/api/search/query` now accepts parsed read-only CGQL only for every role, and
  Lua `graph.query()` is disabled whenever the active backend language is AQL.
  On a parsed-CGQL backend, Lua query access remains read-only for
  `lua:execute` callers and gains permission-gated mutations only with
  `documents:write`. Protected-name/handle checks and dangerous AQL capability
  filtering remain internal defence-in-depth, not public raw-AQL authorization.
