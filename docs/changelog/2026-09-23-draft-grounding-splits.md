# v2.7.36 — Split draft.rs and grounding.rs; pin grounding output

- Date: 2026-09-23
- Status: v2.7.36
- Kind: Refactor (no behavior change) and tests

## Changes

`cognigraph-construct` splits its two largest modules by concern
([CG-95](../issues/CG-95.md),
[decision record](../decisions/decision_construct_module_splits.md)):

- `draft.rs` (entry points, `DraftReport`), `draft/prompts.rs` (system
  prompts and schemas), `draft/finalize.rs` (finalization pass).
- `grounding.rs` (the public grounding contract), `grounding/negation.rs`,
  `grounding/sentences.rs`, `grounding/semantics.rs` and
  `grounding/triggers.rs`.

Every item moved whole; shared helpers and one moved struct's fields became
`pub(super)`; public paths are re-exported unchanged.

A new test, `tests/grounding_snapshot.rs`, pins everything the four public
Semantic Neurons kits construct and their evaluation outcomes in
`tests/snapshots/grounding-kits.json`. Regenerate it only for intentional
grounding changes with `CG_UPDATE_SNAPSHOT=1`.

The workspace version moves to 2.7.36.

## Validation

The snapshot was recorded on the unsplit code and is unchanged after the
split; removing one negation cue makes it fail on the negative-probe kit.
A lossless item comparison shows all 53 items moved with unchanged bodies.
The construct test list is identical (199 tests, all passing) and no
existing test changed. Build, Clippy and the full local gate pass.
