# Decision: cross-chunk grounding — rejected by measurement

**Status:** Decided 2026-07-07. Cross-chunk grounding semantics
(windowed grounding, gate lookback) are NOT adopted; the measured cost
of chunk locality was a tokenization artifact, fixed instead. Revisit
trigger below.

## Context

Grounding is chunk-local: a trigger must be affirmed within one chunk,
and D1 gates judge the sentence around the occurrence. Evidence
spanning chunk boundaries is invisible by design. This session asked
whether that costs enough recall to justify the biggest semantics
extension on the board — and, per the benchmarks-first culture, built
the measurement before choosing semantics
(`examples/chunk_sensitivity.rs`, deterministic, production
`ground_chunk` throughout).

## The measurement (owner: measurement; user approved the session)

Five reference corpora (96 distinct expected facts, 45 forbidden),
space types as authored (gates included), five regimes:

| regime | result (all five kits) |
|---|---|
| original chunking | 96/96, v0/45 |
| **single-sentence chunks** (adversarial floor: nothing may span) | 94/96 before the fix → **96/96 after**, v0/45 throughout |
| windows of 2 and 3 sentence-chunks (stride 1) | recovered NOTHING before the fix; 96/96 after |
| gate lookback 0 (sanity: must equal original) | equal ✓ |
| gate lookback 1 (gates satisfied from the previous sentence) | **no recall gained, no restraint lost** — 96/96, v0/45 |

The two facts lost at the adversarial floor were `OpenProtein.AI` and
`Chorus.ai` — **dotted names split mid-token by the sentence
boundary**, not cross-chunk evidence. The entire measured cost of
chunk locality on these corpora was a tokenization bug.

## Decisions

**D1 — Measure first: done, and it decided the session.** Chunk
locality costs zero recall on all five corpora even under
single-sentence chunking, once the boundary artifact is fixed.

**D2 — Windowed grounding and gate lookback: REJECTED BY
MEASUREMENT.** Windows recovered nothing (and would blur provenance —
the licensing evidence must sit in ONE chunk for `evidence_chunk_id`
to mean anything); gate lookback recovered nothing and, notably,
re-leaked nothing on the hostile corpus (a useful restraint datum kept
on record). Neither buys anything on evidence available today.

**D3 — The artifact fix lands in production `sentence_bounds`:** a
`.` ends a sentence only when followed by whitespace or end-of-text.
This subsumes the earlier digit-digit decimal rule ("$3.09B") and
keeps dotted names ("Chorus.ai", "OpenProtein.AI", "U.S." mid-token)
whole — mid-name splits also truncated the sentence a D1 gate judges.
Regression test added; all kit baselines, the endpoint-gate sim, and
the advisor's 6/6 rediscovery re-verified identical.

**D4 — The deployment-level remedy stays upstream:** chunk with
overlap (the chunker's job, documented in dataops guide 01). Overlap
preserves every grounding invariant — verbatim evidence, chunk-local
provenance, negation lookback, veto locality — which no grounding-side
window can.

**D5 — Revisit trigger.** This measurement is bounded by its corpora:
five author-written kits whose prose keeps facts within sentences.
The decision reverses if the 10k-document operational pilot (real
chunker, real prose) shows misses attributable to evidence that
genuinely spans chunks — `chunk_sensitivity` is the standing
instrument to quantify them, and the lookback-1 restraint datum here
says a narrow gate-lookback would be the first candidate, not
windowed grounding.

## Boundaries kept

- Triggers must be affirmed within ONE chunk — what licenses a fact is
  a single verbatim span with a single `evidence_chunk_id`.
- Gates judge the licensing sentence only.
- Vetoes stay chunk-local plain presence.

## Outcome

Landed 2026-07-07: `sentence_bounds` whitespace rule + dotted-name
regression test; `examples/chunk_sensitivity.rs` as a permanent
instrument; guide 01 overlap note. 369 tests; every prior baseline
byte-identical.
