# Research counting-unit consistency (CG-24)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:129-137` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Research counting-unit consistency (CG-24).** Rev2 and its HTML/PDF exports
  now report 59/59 cold construction, 54/59 repaired, and 0/32 violations as
  distinct triples within each kit. Historical partial deduplication and
  per-question answer scores have explicit units; 31/68 versus 32/68 answer
  recall is described as similar, without claiming identical outputs. Twelve
  offline Rust release replays reproduced the counts without model calls.
  Desktop/mobile HTTP rendering, PDF text/visual checks, and documentation
  validation passed. See the [report](../issues/paper-counting-units-2026-09-09.md).
