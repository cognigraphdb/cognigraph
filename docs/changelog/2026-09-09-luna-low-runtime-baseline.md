# Luna low runtime baseline

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:30-42` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Luna low runtime baseline.** Adopted `reasoning_effort: low` wherever the
  shared OpenAI provider resolves to `gpt-5.6-luna`, including inherited
  side-views. Other model overrides keep their provider defaults; production
  JSON-schema strictness and judge qualification are unchanged. Formatting,
  strict Clippy, all 962 reported Rust tests, the release server/CLI build,
  13 server configurations, 13 rejected configurations and three CLI cases
  passed. Two real Luna calls used unchanged strict JSON-schema request bytes:
  construction grounded two expected directed facts with no negative-excerpt
  fact, and the inherited side-view job stored one grounded pair. Used disposable
  Native databases without resetting existing data. CG-26 and the four frozen
  benchmark packages are checkpointed locally as `8b8cceb`. See the
  [runtime verification and limits](../issues/luna-low-runtime-2026-09-09.md).
