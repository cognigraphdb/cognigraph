# CUAD evidence recovery and claim correction (CG-25, partial)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:99-109` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **CUAD evidence recovery and claim correction (CG-25, partial).** Recovered
  the original splits, taxonomy, accepted-evidence projections, and scoring
  scripts. A pinned source recipe reproduces prepared inputs byte for byte.
  Legacy recall/F1 mixed counting units; consistent gold-entry accounting of
  the same outputs yields P 0.744 / R 0.370 / F1 0.494. The paper, HTML/PDF
  exports, and pilot one-pager now qualify those diagnostics and the holdout
  restart. Seven regression tests, source/scoring replays, notebook execution,
  and HTTP/visual checks passed without model calls. Original raw nominations,
  full accepted facts, and resolved model/settings remain missing, so CG-25
  stays open. See the [report](../issues/cuad-recovery-2026-09-09.md).
