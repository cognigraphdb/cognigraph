# Track B — governed construction on public contracts (CUAD), status 2026-07-22

Goal: a credibility number for governed construction on real legal text,
without any customer data. Corpus: **CUAD** (Atticus Project, CC-BY 4.0) —
510 real commercial contracts with ~13k expert clause annotations.

## What ran today

- Sample: 10 design / 40 holdout contracts, stratified over 8 compliance
  clause categories (`prep.py`; seed 20260722). 702 + 1,705 chunks.
- **Per-document space draft** through the product job system
  (`construct.draft`, gpt-5.4-mini, 20 completions, ~2 min): 225 entities,
  21 relation rules, 51 KB of restraint skips (aliases rejected for not
  occurring verbatim — the gates doing their job on legal text).
- Accept + ingest of the design set: **33 evidence-bound fact occurrences**
  (22 distinct facts) grounded across the 10 contracts.

## Measured (design set — calibration numbers, not the quotable ones)

Every occurrence judged against its evidence sentence, occurrence-level,
same discipline as the FDA-label pilot (`work/judgments.json`):

| metric | value |
|---|---|
| occurrence precision | **32/33 = 97.0%** |
| distinct-fact precision | 21/22 = 95.5% |
| the one error | `Major Territories --PART_OF--> United States` — inverts a definition list |
| **self-flagged?** | **yes** — the semantics detector marked exactly that fact suspect ("sentence never uses the relation's vocabulary") |
| precision excluding suspect-flagged facts | 26/26 = 100% (yield 33 → 26) |

Two independent spot-verifications against the expert labels where the
tasks overlap: the extracted `GOVERNED_BY State of New York` fact's evidence
is **word-for-word CUAD's gold Governing Law span** for that contract, and
the `AMENDS` fact matches the labeled amendment target.

The detector also flagged 6 true-but-weakly-phrased occurrences (e.g.
`DESCRIBES` grounded via an appendix title) — the same ones hand-judging
had caveated. As an error-finder it over-flags (1 real error in 7 flags);
as a quarantine signal it caught 1 of 1 real errors.

## The architectural finding — the honest headline

The auto-drafted space extracts **recurring corpus facts** (TREATS,
AMENDS, GOVERNED_BY, COLLECTIVELY_KNOWN_AS…) — which is what Semantic
Neurons are designed for — and it does so at 97% precision on legal prose.
It does **not** chase CUAD's per-document clause categories (exclusivity,
non-compete, cap on liability): rules are instance-anchored to entities the
draft saw, so clause-category detection is a **directed** task the pipeline
does not yet have a mode for. Forcing the corpus-knowledge mode onto the
clause-classification benchmark would have produced a rigged comparison in
either direction; registered instead as an open product item (D12 in
`docs/decisions/decision_cgql_v2_workload_gaps.md`): *directed extraction —
given a clause taxonomy, extract per-document facts with evidence through
the same grounding gates.*

## D12 built, holdout run (same day)

D12 (directed construction) shipped as `POST /api/construct/directed`:
taxonomy in, evidence-bound facts out, deterministic gates deciding what
the model's nominations may land. Calibration: three taxonomy rounds on the
design set only (F1 0.627 / 0.605 / 0.579 — rounds 2–3 fit run-to-run
variance, so round 1 was frozen). The holdout then ran ONCE.

### The quotable numbers — 40 held-out contracts, frozen taxonomy & scorer

| category | P | R |
|---|---|---|
| Governing Law | 0.90 | 0.79 |
| Anti-Assignment | 0.88 | 0.45 |
| Cap On Liability | 0.86 | 0.66 |
| Non-Compete | 0.83 | 0.38 |
| Termination For Convenience | 0.83 | 0.33 |
| Audit Rights | 0.81 | 0.90 |
| Exclusivity | 0.55 | 0.74 |
| Ip Ownership Assignment | 0.22 | 0.25 |
| **overall** | **0.744** | **0.635** (F1 0.685) |

164 evidence-bound facts over 40 real contracts; every fact carries a
verbatim quote with byte offsets into the source text. Scoring is
agreement with CUAD's expert clause spans (12-word-run/containment match),
category-level recall counted when a labeled clause got no prediction.

### Error analysis (post-hoc; changes nothing above)

- **One failure mode dominates Exclusivity**: at least 8 of its 19 FPs
  assert exclusivity over expressly **non-exclusive** grants — "exclusiv"
  as restraint vocabulary cannot catch its own negation by prefix. The v2
  taxonomy description fixed exactly this and then lost the design-set
  coin flip; it is the first change for the next taxonomy revision, made
  here in writing rather than silently after seeing the holdout.
- Ip Ownership Assignment fails mostly on **retention vs assignment**
  ("each party retains ownership…") and on tooling/equipment title — a
  definition boundary, not an evidence failure.
- Several FPs are true, well-evidenced facts that CUAD simply does not
  file under the category (e.g. sole-authority allocations) — the score
  is expert-label agreement, strictly harsher than fact correctness.
- **Negative finding, reported as measured**: filtering semantics-flagged
  facts leaves precision flat (0.742) while halving recall (0.635 →
  0.389). semantics-v1's signals were designed for corpus-knowledge
  shapes; on directed legal facts they do not separate right from wrong.
  The 97%-precision auto-draft result's self-flagging does NOT transfer
  to this mode — do not quote it as if it did.
- The holdout corpus also found a real D12 bug on first contact:
  `[***]`-redacted endpoints passed the textual gates and then failed the
  atomic write. Fixed at the gate with a regression test (repo e0ff466).

### How to present this

"On 40 held-out real contracts with expert labels, directed construction
extracted 164 evidence-bound clause facts at 74% precision / 64% recall
against the expert annotations — every fact quoting its sentence with
offsets, every rejection logged. Six of eight clause categories sit at
81–90% precision; the two weak categories have a named, understood failure
mode each." Do not compare against CUAD's QA-model leaderboard — different
task framing (span QA vs typed facts).

Re-run: `prep.py` → `run_directed.py --set design` (calibrate) →
freeze → `run_directed.py --set holdout` → `score.py`.
