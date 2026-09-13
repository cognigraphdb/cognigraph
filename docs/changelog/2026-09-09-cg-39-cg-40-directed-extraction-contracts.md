# CG-39/CG-40: directed extraction contracts

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Dated CG-39/CG-40 decision and candidate capture (committed 2026-09-10)
- Source: `CHANGELOG.md:5-16` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CG-39/CG-40: directed extraction contracts.** Policy v2 requires Unicode
  token boundaries around endpoint mentions and supplies exact request chunk-ID
  and relation enums to completion providers. Quote byte offsets, local rejection
  of unknown IDs, malformed-output preservation and explicit-empty reconciliation
  remain intact. Formatting, strict Clippy, 967 reported Rust tests, the release
  build and 34 OpenAI/Gemini HTTP cases passed. Replaying the original development
  proposals removed one invalid endpoint occurrence and retained all 16 reference
  matches. Forty fresh Luna-low calls produced zero invalid chunk citations and
  35 stored occurrences with 20 reference matches. The original trial is committed
  as `a0e4b24`; historical packages and the unrun holdout remain unchanged.
  See the [candidate results and limits](../research/experiments/luna-directed-v2-2026-09-09/results.md).
