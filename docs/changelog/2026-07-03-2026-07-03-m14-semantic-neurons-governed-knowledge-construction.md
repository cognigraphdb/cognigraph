# 2026-07-03 — M14: Semantic Neurons (governed knowledge construction)

- Date: 2026-07-03
- Status: Historical
- Kind: History
- Date source: Original section heading
- Source: `CHANGELOG.md:2274-2288` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- New `cognigraph-construct` crate porting the Semantic Neurons research:
  space-type ontologies, alias/relation_hint neurons with the full
  lifecycle, negation-aware evidence grounding (Case Bravo regression),
  ingestion with intrinsic provenance, recall + restraint evaluation, and
  per-neuron ablation/redundancy reports.
- Research parity on first run: SOTU full recall with accepted neurons,
  case:alpha + study:px-101 on base ontology, Case Bravo 0/4 forbidden,
  graduation finding reproduced.
- `CompletionProvider` trait (OpenAI + Gemini) powers gap-directed
  proposals; the closed control loop (measure → propose → validate →
  accept → re-measure) passes deterministically and LIVE on both
  providers. Fixtures converted from the research repo.
