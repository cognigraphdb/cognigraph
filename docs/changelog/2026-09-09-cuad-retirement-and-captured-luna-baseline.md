# CUAD retirement and captured Luna baseline

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:79-89` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CUAD retirement and captured Luna baseline.** The user explicitly closed
  CG-25's remaining historical recovery scope without reconstructing its
  missing execution evidence. A new 48-excerpt synthetic directed-construction
  baseline captured sixteen real provider/server calls: Luna TP/FP/FN 27/1/5
  and 26/1/6; Astra 32/0/0 in both runs. Estimated total token cost was $0.207572.
  All 119 stored occurrences passed provenance checks; scorer tests, loopback
  controls, integrity probes and exact offline replay passed. Luna remains
  the default; independent representative-document evaluation is still needed.
  No Rust behavior, model defaults, or judge qualification changed. See the
  [results](../research/experiments/luna-baseline-2026-09-09/results.md).
