# WebNLG experiment status and provenance (CG-27)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:118-128` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **WebNLG experiment status and provenance (CG-27).** The pilot guide,
  positioning dossier, roadmap, and plan now record the completed fuzzy,
  expanded-corpus, live-proposal, and pruned-candidate experiments alongside
  the frozen deterministic result. Artifact links point into the construction
  crate. The pruned candidates' recall/precision tradeoff and development-set
  selection are explicit; offline pruning is distinguished from runtime
  governed review. Five offline verification commands passed, covering 14
  scoring observations; 401 prepared files remained unchanged and frozen
  mining reproduced byte for byte, without model calls or test scoring. See the
  [verification report](../issues/webnlg-status-2026-09-09.md).
