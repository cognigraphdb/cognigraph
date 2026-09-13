# Completion model defaults

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:324-334` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Completion model defaults.** OpenAI now defaults to `gpt-5.6-luna` and
  Gemini to stable `gemini-3.8-flash`, including independent side-view lanes.
  Luna explicitly uses `reasoning_effort: none` to preserve the previous
  OpenAI default's non-reasoning behavior. Current configuration examples and
  provider references are updated; historical benchmark measurements retain
  their original model identities. Formatting, Clippy, all 888 tests, and the
  release HTTP/harness matrix passed with synthetic loopback providers;
  that initial validation did not measure external model quality or
  availability. The subsequent real comparison is recorded above. See the
  [provider decision](../decisions/decision_sideviews_provider_and_benchmark.md#current-model-defaults-2026-09-08).
