# Independent-label GLM Flash–Luna trial

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:56-67` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Independent-label GLM Flash–Luna trial.** Retired DeepSeek V4 Flash from
  future candidate runs. Evaluated 600 distinct human-labelled SemEval sentences,
  twice per model, through the release server and a shared supplied-pair adapter.
  GLM scored 65.75% raw accuracy versus Luna's 42.67%; paired difference
  +23.08 points, 95% interval [+17.67, +28.75]. Both had substantial Other-case
  restraint errors; Luna remains the default and GLM leads the next domain trial.
  Retained all six measured failures, the earlier stopped attempt and its
  recorder correction. Combined known token cost was $0.119233, with two
  timeout calls of unknown usage. Six tests, amended gold/failure HTTP controls,
  official scoring, exact replay, 1,705 evidence checks and integrity checks
  passed. See the [results and limits](../research/experiments/glm-luna-semeval-2026-09-09/results.md).
