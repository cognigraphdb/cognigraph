# DeepSeek/GLM model comparison

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:68-78` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **DeepSeek/GLM model comparison.** Compared DeepSeek V4 Pro/Flash and
  GLM-5.3/Flash with a fresh Luna reference in a shared JSON-mode configuration.
  Three candidates scored 32/0/0 TP/FP/FN twice; Luna scored 31/0/1 and 31/1/1.
  DeepSeek Flash exhausted its output allowance and returned truncated JSON;
  the failed arm is retained without a full quality score or retry. The 38
  calls cost an estimated $0.075562071 including preflights and failure.
  GLM Flash is the strongest candidate for a representative-data trial; Luna
  remains the default. Six tests, 45 synthetic controls, exact score replay,
  255 occurrence checks and preservation/integrity checks passed. See the
  [comparison and limits](../../fixtures/semantic-neurons/cross-provider-baseline-2026-09-09/results.md).
