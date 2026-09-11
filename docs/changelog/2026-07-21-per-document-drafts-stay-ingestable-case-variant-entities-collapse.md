# Per-document drafts stay ingestable: case-variant entities collapse

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:762-775` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Per-document drafts stay ingestable: case-variant entities collapse.**
  Independent per-document drafts capitalize the same entity differently
  (`Epinephrine` / `epinephrine`), which are one entity to the store because it
  keys on the case-folded `entity_key`. `ingest_chunks` correctly fails closed on
  the collision, so drafting 100 real labels produced **63 colliding groups and an
  ontology that could not be ingested at all**. `finalize_draft` now canonicalizes
  identity before the trigger check: variants collapse to the first-seen
  definition with aliases unioned, every collapse recorded in `skips`, and type
  disagreements surfaced for review like any other D3 conflict. Crucially the
  **rules are rewritten too** — they name endpoints by literal name and grounding
  resolves them by exact string, so collapsing entities without rewriting would
  have silently orphaned every rule referencing a dropped variant; rules that then
  collide onto one triple are folded together with their triggers unioned.
