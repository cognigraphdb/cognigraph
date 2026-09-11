# Decision: DailyMed pilot gate 3 — frozen baseline policy (D1–D6)

**Status:** Decided 2026-07-13 (user approved all six). Freezes the
policy for the full 10,000-document DailyMed baseline. Builds on
`decision_dailymed_ontology.md` and the gate 1 / 1.5 / 2 measurements.

## Context

Two measured facts from gate 2 drive gate 3:

1. **Deterministic narrative precision falls with scale.** 100 docs
   ≈ 90%; 1,000 docs = 2,050 true / 943 false = 69%. The dominant
   false-positive source is the `of {target}` trigger firing on
   co-mentioned drugs in interaction/warnings sections. At 10k that is
   ~9,400 false positives — far too many to bulk-judge (~19k LLM calls).
   The FP rate must fall at the source, not be cleaned up downstream.

2. **Judge confidence does not separate its mistakes.** On the gate-2
   sample, accepted-true confidence spanned 0.82–0.99 (mean 0.96) and
   accepted-false spanned 0.82–0.99 (mean 0.95); correctly-rejected-false
   had a *higher* mean (0.98). A confidence threshold cannot gate these
   relations — the judge is confidently wrong on its false-accepts.

Together these point to one conclusion: for mechanically-checkable
relations, the structured-data oracle — not the judge's confidence — is
the right precision gate.

## Decisions (owner: user, 2026-07-13)

**D1 — Freeze tightened triggers; kill the FP source.** Replace the
broad `of {target}` template with composition-specific patterns, measure
precision/recall against the oracle on the gate-2 1,000-doc cohort
(deterministic, free), and freeze the set that maximizes precision at
acceptable recall. The chosen set is pinned in the policy manifest.

*Calibration outcome (2026-07-13, 1,000 docs, `--calibrate`): the
tradeoff is not uniform, so the freeze is per-relation.*
`CONTAINS_INGREDIENT` drops `of {target}` — precision 50%→86% for 25%
active-recall (production precision is guaranteed by the structured gate,
so narrative precision governs only review-queue size).
`ADMINISTERED_VIA` **keeps** `{target} administration` — dropping it
(the "tightest" set) cost 45 recall points (62%→17%) to gain 12 precision
points, an unacceptable trade, and route mentions are far less
leakage-prone than ingredient mentions. Frozen sets:
`CONTAINS_INGREDIENT = ["contains {target}", "{target}, usp"]`,
`ADMINISTERED_VIA = ["for {target} use", "{target} use only", "{target} administration"]`.

**D2 — Two-tier oracle + salt/base normalization as frozen scoring
truth.** TRUE = active ∪ inactive ingredients (retires the oracle-scope
FP category — excipients are real). Report active-only and
full-ingredient recall separately for comparability with gate 1.5. Parse
SPL `<activeMoiety>` and add it as an ingredient alias so salt/base pairs
("lidocaine hydrochloride" / "lidocaine") stop counting as misses
(retires the name-granularity category).

**D3 — For mechanically-checkable relations, structured data is the
production gate; the judge/narrative layer is the instrument and the
path for relations WITHOUT ground truth.** After deterministic grounding,
every narrative fact is cross-checked against the structured oracle for
the same product, yielding three buckets: **agreed** (narrative +
structured — auto-accepted, ~100% precision by construction),
**narrative-only** (no structured backing — the review queue, dominated
by the FP population), and **structured-only** (imported facts prose did
not state). The agreed graph is the high-precision deliverable. The
judge remains load-bearing exactly where it will be needed for real —
pass-2 clinical relations (`TREATS`, interactions) that have no
structured oracle. This sharpens the thesis: it shows precisely where
governed narrative construction earns its keep.

**D4 — No confidence-threshold auto-accept for these relations
(measured, not assumed).** Because confidence does not separate true from
false accepts, mechanically-checkable relations are gated on structured
agreement (D3), never on judge confidence. The 0.90 policy threshold is
retained only where structured truth is absent (pass 2).

**D5 — Frozen baseline relation set + a free structured graph.**
Baseline = four relations: `CONTAINS_INGREDIENT` and `ADMINISTERED_VIA`
(narrative + structured), `HAS_DOSAGE_FORM` and `MARKETED_BY`
(structured). Structured facts populate the full 10k graph
deterministically at zero LLM cost. `HAS_PHARMACOLOGIC_CLASS` /
`HAS_RXNORM_CONCEPT` need a collector extension (two more DailyMed API
families) and are a recorded post-baseline item, out of gate-3 scope.

**D6 — The 10k baseline deliverable, acceptance criteria, and a pinned
policy manifest.** The full run produces: the complete graph (narrative +
structured), oracle-scored precision/recall per relation at 10k scale, a
certified judge-quality sample (Experiment-A style — judge quality
measured ON the baseline, not assumed from gate 2), the
structured/narrative agreement rate, grounding cost/wall-clock, and a
versioned `gate3-policy-v1` manifest (triggers, gates, oracle definition,
relation set, DailyMed snapshot). Acceptance: measured precision targets
met per relation, zero cross-product leakage, reproducible from the
pinned snapshot.

## Boundaries kept

- The drift experiment (fixed multi-version Set-ID cohort) stays
  deferred to after the baseline, as the staging doc plans.
- No clinical-effect relation is constructed in the baseline.
- Structured and narrative facts never share an edge (provenance is
  explicit — `decision_dailymed_ontology.md` D2).
- The oracle gates production only where it exists; pass-2 relations
  without structured truth fall back to the governed judge loop.

## Outcome

The frozen 10,000-label run completed. The admitted production graph contains
129,091 facts: 116,855 imported structured facts plus 12,236 narrative facts
corroborated by the same structured oracle, with zero cross-product leakage.
The remaining 4,384 narrative candidates were quarantined rather than asserted.
Narrative extraction alone measured 73.6% precision and failed its 80% proxy
target; “100% precision” applies only to the structured-oracle-gated production
graph, by construction. The lossless mention prefilter reduced a 2+ CPU-hour
grounding wall to roughly 25 minutes.
