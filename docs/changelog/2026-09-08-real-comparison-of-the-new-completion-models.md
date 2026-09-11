# Real comparison of the new completion models

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:315-323` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Real comparison of the new completion models.** The existing side-view
  harness made 32 successful official API calls (two preflights plus 30
  measured calls). Luna averaged 3.679 s and an estimated $0.4844 per 1,000
  similar passages; Gemini 3.8 Flash averaged 5.141 s and $4.9993 with default
  thinking. Exact 12-pair counts were 13/15 versus 15/15. The source review
  found unsupported additions from both models; no judge qualification or
  model default changed. See the
  [measurement, limitations, and raw artifacts](../../fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/README.md).
