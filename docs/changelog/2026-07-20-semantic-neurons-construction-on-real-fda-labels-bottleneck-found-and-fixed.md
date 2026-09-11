# Semantic Neurons construction on real FDA labels — bottleneck found and fixed

- Date: 2026-07-20
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:805-822` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Semantic Neurons construction on real FDA labels — bottleneck found and fixed.**
  The first governed construction run on real DailyMed labels confirmed the pipeline
  works end to end (closure-driven restraint solid, the deterministic gate advisor
  flags every bad fact), but localized the lost value to **stage-1 entity
  extraction**: one broad draft samples ~evenly across a mixed corpus, starving the
  condition entities that the pilot-relevant `INDICATED_FOR` / `CONTRAINDICATED_IN`
  relations need, so endpoint-closure drops them. A/B: broad interleaved drafting
  extracted **0/15** condition entities and 0 drug→condition rules; **per-document
  drafting recovered 15/15**. Added `draft_space_type_per_document` and a
  `per_document` flag on `POST /api/construct/draft` (chunks grouped into documents
  by `title`, drafted densely per document, catalogues/rules merged with type
  conflicts surfaced) — live-validated to recover the drug→condition rules broad
  drafting misses (e.g. `Betamethasone TREATS chronic plaque psoriasis`). Known
  follow-up: per-document drafting is synchronous, so large corpora should move to
  the durable job framework. Findings, the A/B, and the priority order (D1
  per-document drafting, D2 label-scoped grounding, D3 global-entity identity) in
  `docs/decisions/decision_neurons_real_label_construction.md`.
