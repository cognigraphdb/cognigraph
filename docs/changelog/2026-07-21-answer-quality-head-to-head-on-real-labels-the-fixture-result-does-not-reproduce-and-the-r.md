# Answer-quality head-to-head on REAL labels — the fixture result does not reproduce, and the reason is located

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:648-668` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Answer-quality head-to-head on REAL labels — the fixture result does not
  reproduce, and the reason is located.** The 2026-07-07 control arm measured
  graph 86% vs vector 36% answer recall on five kits with hand-authored
  ontologies. Rerunning the same runner and methodology on the real 100-label
  corpus with the LLM-drafted `dailymed_pilot_v4` ontology — ground truth written
  from the LABEL TEXT by an independent model, never from the graph — inverts it:
  **vector 54/149 (36%), graph 33/149 (22%)**. The diagnosis is precise and none
  of it is architectural: only 34 of 140 expected clinical facts exist in the
  graph at all, and the graph arm asserted **33 of those 34 — 97% of its
  ceiling**, so retrieval and answering are already maxed. Of the 106 misses,
  **0 had a rule that failed to ground** and **0 involved an entity the ontology
  lacked** — every one is a relation that was never drafted. Splitting those
  further, **43% of expected facts ARE in the graph under a different relation
  label** (`AVOID_IN` vs `use_with_caution_in`, `HAS_WARNING` vs
  `adverse_effect`): true coverage is **67%**, not 24%. Consequently relation-
  vocabulary normalization is **promoted to the top construction priority**,
  reversing the "usability item, not a precision blocker" call made earlier the
  same day. Full write-up, including the 0/9 administrative kit that quantifies
  what the administrative-relation filter costs in answers, in
  `docs/decisions/decision_answer_head_to_head_real_labels.md`.
