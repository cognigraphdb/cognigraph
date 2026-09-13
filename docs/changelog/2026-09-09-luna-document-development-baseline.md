# Luna document development baseline

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:17-29` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Luna document development baseline.** Prepared 40 development and 80 unrun
  holdout documents from the human-reviewed Re-DocRED release, with twelve fixed
  relations and no supplied entity pairs. Forty real Luna-low calls completed;
  55 nominations produced 32 stored facts, of which 16 matched the 683 published
  references. Annotation/inference-policy gaps prevent a truth-precision claim.
  Ten tests, forty release-server gold controls, source/score replay and five
  integrity probes passed. Preserved a versioned scorer correction for invalid
  chunk citations. Three separate release HTTP probes reproduced two new Open
  findings: CG-39 endpoint substring acceptance and CG-40 unconstrained completion
  identifiers, subsequently resolved by policy v2 above. No Rust behavior changed
  during this original baseline capture. See the
  [results and limits](../research/experiments/luna-documents-2026-09-09/results.md).
