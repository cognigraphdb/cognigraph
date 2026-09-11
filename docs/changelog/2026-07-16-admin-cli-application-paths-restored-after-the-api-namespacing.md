# Admin CLI: application paths restored after the `/api` namespacing

- Date: 2026-07-16
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1272-1282` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Admin CLI: application paths restored after the `/api` namespacing.**
  The `cognigraph` CLI kept requesting application endpoints at the
  server root, where it now received the console's `index.html` with a
  200 (when `COGNIGRAPH_UI_DIST` is set) or a 404 — silently, since the
  body parsed as `<non-JSON response>`. The `/api` prefix is applied at
  the client's one URL choke point (`api_url()`), `COGNIGRAPH_URL` stays
  the plain server base, and `health` keeps its root paths via
  `get_root()`. Regression test on URL construction; verified live:
  `health`, `login`, `doc list`, and a CGQL `query` against a running
  server.
