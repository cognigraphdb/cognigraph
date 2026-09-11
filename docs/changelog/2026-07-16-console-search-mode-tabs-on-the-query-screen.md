# Console: search-mode tabs on the Query screen

- Date: 2026-07-16
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1309-1316` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Console: search-mode tabs on the Query screen.** The Query console
  hosts five tabs — CGQL plus semantic, hybrid, vector, and
  graph-augmented forms backed by `/api/search/*` — with catalog-fed
  collection selects, results tables whose rows deep-link into the
  document browser, raw-response views, and honest disabled states (no
  embedder, no edge collections). Verified live against Ollama
  `nomic-embed-text` over a 150-label DailyMed fixture.
