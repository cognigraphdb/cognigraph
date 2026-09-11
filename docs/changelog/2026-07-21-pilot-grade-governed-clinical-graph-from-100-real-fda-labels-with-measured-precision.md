# Pilot-grade governed clinical graph from 100 real FDA labels, with measured precision

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:743-761` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Pilot-grade governed clinical graph from 100 real FDA labels, with measured
  precision.** 100 DailyMed labels → 659 section chunks → per-document drafting as
  a durable background job (1024 s, checkpointed every 25 documents) → 1384
  entities / 1327 rules → ingest **659 chunks to 1746 facts in 10.7 s**, with all
  100 labels contributing. Precision measured on a seeded 200-fact sample by an
  **independent** judge (Gemini, different provider from the `gpt-5.4-mini`
  drafter, strict response schema, calibrated 6/6 on a two-sided set before use):
  **71.5% evidence-supported overall, 81.1% on clinical relations** (89% of the
  graph), 87.6% label-correct. The entity starvation is gone — broad drafting gave
  0 condition entities and 0 drug→condition rules on 50 labels; per-document
  drafting gives **435 and 511**. Two results drive the next work: administrative
  relations (`REPORT_ADVERSE_REACTIONS_TO`, `CONTAINS_INFO_ON`, …) are 11% of the
  graph at **19.4%** precision and a third of all failures, and the deterministic
  gate advisor **no longer separates good facts from bad** (70.4% clean vs 72.9%
  flagged) because the failure class it detects — cross-document leakage — has
  largely been eliminated, leaving relation-semantics mismatches it is structurally
  blind to. Full numbers, two rejected hypotheses, and the priority order in
  `docs/decisions/decision_pilot_clinical_graph.md`.
