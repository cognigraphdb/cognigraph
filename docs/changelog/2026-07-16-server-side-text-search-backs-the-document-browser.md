# Server-side text search backs the document browser

- Date: 2026-07-16
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1283-1295` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Server-side text search backs the document browser.** New
  `POST /api/search/text`: plain BM25 over string fields via
  `GraphBackend::text_search` (the tantivy capability hybrid already used
  for its text leg), requiring no embedding provider. Hits are addressed
  by the document's own `_id` — never an application-level `document_id`
  field. The console's document browser now searches the whole collection
  through it: a non-empty query switches the table from the paged listing
  to server hits over title/content/summary/text merged with an exact-key
  lookup (a pasted key pins that document first); clearing restores the
  paged listing. Verified live on the 10k-label DailyMed corpus: cold
  tantivy index build once per collection+fields (persisted beside the
  redb store), ~20 ms warm.
