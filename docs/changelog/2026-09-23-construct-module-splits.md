# v2.7.35 — Split the construct proposal and directed modules

- Date: 2026-09-23
- Status: v2.7.35
- Kind: Refactor (no behavior change)

## Changes

`cognigraph-construct` splits `propose.rs` and `directed.rs` by concern
([CG-94](../issues/CG-94.md),
[decision record](../decisions/decision_construct_module_splits.md)):

- `propose.rs` (report types, neuron entry points), `propose/neurons.rs`
  (per-fact proposer), `propose/blockers.rs` (blocker path).
- `directed.rs` (contract types, `directed_ingest`), `directed/gates.rs`
  (evidence gate and quote matching), `directed/prompt.rs` (taxonomy check,
  prompt, schema).

Every item moved whole; shared helpers became `pub(super)`. Public paths and
crate-root re-exports are unchanged.

The workspace version moves to 2.7.35.

## Validation

A lossless comparison of top-level items shows all 30 moved with unchanged
bodies. The construct crate's test list is identical before and after (198
tests, all passing) and no test file changed. Build, Clippy and the full local
gate pass.
