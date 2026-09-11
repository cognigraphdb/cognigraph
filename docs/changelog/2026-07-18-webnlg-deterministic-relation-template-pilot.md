# WebNLG deterministic relation-template pilot

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1235-1247` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **WebNLG deterministic relation-template pilot.** Added a resumable
  `webnlg-pilot` generator for the pinned `GEM/web_nlg` English snapshot. It
  prepares 38,872 text documents while keeping 115,279 DBpedia oracle triples
  in a physically separate evaluation path, excludes overlapping challenge
  splits, records artifact digests, and supports strict offline verification.
  The 70 MB baseline was rebuilt from its cache with every non-timestamped file
  byte-identical. Frozen supervised template mining scored 12.4% recall / 76.6%
  precision on validation and 7.0% / 68.8% on the one-shot test. Entity surfaces
  were supplied from the oracle in every lane, and the mined artifact did not
  pass through the runtime Neuron review lifecycle; this is relation-template
  evidence, not full text-to-graph construction. It remains an automatically
  scorable, non-clinical complement to DailyMed.
