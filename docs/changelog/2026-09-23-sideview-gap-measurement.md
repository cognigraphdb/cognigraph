# v2.7.38 — Side-view gap source measured under trigger ablation

- Date: 2026-09-23
- Status: v2.7.38
- Kind: Research and verification (no product behavior change)

## Changes

- **Measurement.** It covers the side-view gap source from v2.7.37
  ([CG-97](../issues/CG-97.md),
  [research note](../research/semantic-neurons/sideview-gap-measurement-2026-09-23.md)).
  The run used the public forensics alpha and bravo kits with the triggers
  of their expected-fact rules removed, `gpt-5.6-luna`, three repetitions,
  and evaluation-backed review.
  - Every missing expected fact was a candidate in every run.
  - One negated forbidden fact was nominated in every bravo run. The
    proposer declined it every time.
  - All stored proposals survived review, and none built a forbidden fact.
  - Recall after repair was 5/6 on alpha and 7 to 8/9 on bravo.
- **Harness.** A new live harness,
  `docs/verification/harnesses/sideview-gap-measurement.py`, has a
  provider-free `--preflight` mode.
  - It pins the generation providers and models and blanks or relocates
    every storage root.
  - It lets the server load provider keys from the chosen directory's
    `.env` itself, and aborts unless the server log confirms the authorized
    model.
  - Its pure logic is covered by `scripts/tests/test_sideview_gap_measurement.py`.
- **Decision.** The
  [decision record](../decisions/decision_sideview_gap_detector.md)
  now records the measurement and keeps `min_support` at 1.

The workspace version moves to 2.7.38.

## Validation

- **Harness logic:** 12 unit tests cover ablation, candidate classification,
  the survival rule, log redaction, the model check and the run summary.
- **Preflight:** it passed on the same build with no model calls. Ablation
  lowered baseline recall to 0/6 and 0/9, and the provider-free refusals
  were the expected ones.
- **Raw records:** they are sealed in the private evidence repository with
  a SHA-256 manifest.
- **Gate:** the full local gate passes.
