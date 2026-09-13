# Relation-vocabulary normalization: attempted, measured, REJECTED (no code change)

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:628-647` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Relation-vocabulary normalization: attempted, measured, REJECTED (no code
  change).** The head-to-head appeared to show 43 points lost to label
  fragmentation, which promoted vocabulary work to top priority. Measuring it
  properly: only 10 of the 60 mismatches share a head token, so a conservative
  lexical fold moves coverage **24.3% → 25.7% (+1.4, not +43)**; a hand-written
  *semantic* equivalence map reaches **47.1%** with zero forbidden-fact
  collisions, so the honest ceiling for naming work is ~+23 points and only via
  semantics. That map is domain-specific pharma vocabulary and cannot live in the
  domain-general construct crate. The domain-general intervention — showing each
  per-document rule draft the labels the corpus already uses — was implemented and
  re-run over all 100 labels: it **converged the vocabulary 301 → 236 labels
  (−22%) and halved label-neutral answer coverage, 47.1% → 23.6%**, with expected
  target-entity coverage falling 124/140 → 100/140 while building slightly MORE
  facts. Most likely prompt dilution: the label list grows to ~236 entries and
  crowds out exhaustive reading of the excerpts. **Reverted** — convergence is not
  worth halving coverage. Any future attempt needs a fixed, short,
  operator-supplied vocabulary rather than an accumulated one, and must clear the
  same head-to-head. Full numbers in
  `docs/decisions/decision_answer_head_to_head_real_labels.md` D1.
