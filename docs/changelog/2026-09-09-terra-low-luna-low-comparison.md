# Terra low / Luna low comparison

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:43-55` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Terra low / Luna low comparison.** Completed 202 new real provider/server
  calls on the frozen 600-sentence SemEval corpus, two repetitions. Terra scored
  72.42% raw accuracy and Luna low 70.08%; the paired margin is +2.33 points
  (95% interval +0.42 to +4.25), at about 10.4 times the token cost. Luna low
  gains 27.42 observed points over historical Luna none, reducing direction
  errors from 136 to 1. Both new arms retained four content-filter failures
  and substantial Other-case restraint errors. Corrected cache-write accounting,
  exact replay, 404 prompt comparisons, eight official-scorer comparisons,
  1,772 evidence checks and tamper probes passed. Luna low and GLM Flash are
  the candidates recommended by that experiment; the subsequent user decision
  selects Luna low, adopted above. See
  [results, costs and limits](../research/experiments/terra-luna-low-semeval-2026-09-09/results.md).
