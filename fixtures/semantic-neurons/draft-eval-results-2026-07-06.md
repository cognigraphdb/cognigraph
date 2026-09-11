# Ontology drafter D5 measurement — 2026-07-06, `gpt-5.4-mini` @ draft-policy-v1

The decision's own gate (decision_ontology_drafter.md, D5): draft a
space type from each reference kit's chunks with the author ontology
WITHHELD, ground the corpus with the draft, score against the kit's
eval spec. Scoring is label-lenient and direction-respecting (drafted
names and labels won't string-match the author's); violations counted
the same way, which is conservative for restraint. Runner:
`examples/ontology_draft_eval.rs`. THREE full runs (the judge-replay
lesson: single runs overstate determinism).

## The numbers, as promised — whatever they are

| metric | run 1 | run 2 | run 3 |
|---|---|---|---|
| expected pairs covered (5 kits, 96 distinct) | 7/96 | 7/96 | 5/96 |
| forbidden pairs connected (45 distinct) | **0/45** | **0/45** | **0/45** |
| author-entity coverage (187 entities) | — | — | 95/187 (51%) |

Per-kit entity coverage (run 3): 27/35 (77%), 14/27, 11/27, 22/38, and
21/60 on the hostile kit — where only 40 of 1,120 chunks fit the
sample, so vocabulary the model never saw cannot be drafted. Per-kit
rule coverage varies widely run to run (the vendor kit scored 0, 6,
and 2 across observations): one-shot generative drafting has a wide
output distribution.

## Reading

1. **As a rule author, the drafter fails — ~6% of an expert's
   expected pairs.** The drafts are not nonsense: rules are real,
   evidence-backed, and verbatim-checked, but they connect DIFFERENT
   pairs than an expert's eval demands, and direction reversals are
   endemic (vendor-as-source habit: "Snowflake --SELECTED_PLATFORM-->
   [the company]"). This is the third independent sighting of the
   direction-faithfulness weakness (judge ctl-s2, drafted rules,
   hostile traps) — models are weak at direction everywhere we have
   measured them.
2. **As a vocabulary bootstrap, the drafter works — about half the
   expert catalogue, all surface-checked, at zero measured restraint
   risk** (0/135 forbidden-pair opportunities across three runs). And
   vocabulary is exactly what the rest of the system cannot create for
   itself: the repair loop's proposals validate their endpoints
   against declared entities, so entities-first is the enabling step.
3. **The honest workflow is therefore composite:** draft → human
   review/edit → accept (vocabulary exists, attributed) → author or
   accept a few seed rules → evaluate → the REPAIR LOOP authors the
   rules under governance — the loop is the measured rule author (93%
   recovery from zero on independent data), not the drafter. The
   drafter's job is to make the loop startable on a fresh corpus in
   minutes instead of days.
4. Positioning shipped accordingly: guide 04 and the route docs call
   the drafter a vocabulary bootstrap; nothing anywhere claims it
   writes good rules.

## Caveats

- Label-lenient pair matching under-credits semantically-equivalent
  rules with different endpoint granularity (the drafter often chose
  finer-grained endpoints than the author); the entity-coverage number
  is the fairer measure of the labor actually saved.
- gpt-5.4-mini only; 40-chunk sample cap; three runs.

---

## Cross-tier addendum: `gpt-5.4` (2026-07-07, three runs)

Full capacity roughly TRIPLES rule coverage (21, 14, 27 /96 vs mini's
7, 7, 5) and lifts entity coverage to ~67% (121–130/187 vs 95/187) —
and produced the first drafter restraint miss in six runs across both
models (run 3 connected one forbidden boundary-trap pair, 1/45).
Positioning unchanged: ~22% of an expert's spec with high variance is
a starting point to edit, and human review of drafts now visibly earns
its keep. Details: `model-matrix-gpt54-results-2026-07-07.md`.
