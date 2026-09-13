# Per-fact semantics verdicts, persisted as a quarantined sidecar

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:685-704` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Per-fact semantics verdicts, persisted as a quarantined sidecar.**
  `ingest_chunks` writes a verdict per grounded fact into a new `fact_semantics`
  collection, keyed by the fact's own key, in the same atomic batch as the facts.
  Deliberately a **sidecar rather than fields on the fact edge**: M26 attests the
  fact projection byte-for-byte, and this detector is a heuristic expected to keep
  improving — folding it in would make every detector revision invalidate signed
  artifacts. It joins `side_views` in `GENERATED_COLLECTIONS`: publicly readable
  and joinable, never publicly writable, so a forged "clean" verdict cannot
  launder a suspect fact. Only suspect facts get a row, so the collection *is* the
  review queue, and because rows are rebuilt with their facts, "no row" means
  "analyzed and clean" and a stale verdict can never outlive its evidence.
  Measured on the rebuilt graph: 756 of 1575 facts flagged, **clean lane 91.8%**,
  +15.2% separation, 70% of errors caught. **Correction to the previous entry:**
  the claimed "per-rule aggregation costs ~2 points" was wrong — prototype and
  shipped agree on 192 of 200 judged facts, and the apparent 93.9% was an artifact
  of the prototype's crude sentence extraction, not of granularity. The remaining
  disagreements are all one diagnosed cause: `sentence_bounds` splits on
  abbreviation periods, truncating a sentence mid-entity-name (`St. John's Wort`,
  `D C Yellow No. 10`), which produces ~6 false flags per 200 facts.
