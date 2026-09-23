# Decision: split the construct proposal and directed modules by concern behind stable paths

Status: accepted and implemented 2026-09-23 for [CG-94](../issues/CG-94.md).

## Context

`cognigraph-construct/src/propose.rs` (728 lines) and `directed.rs` (509) each
mixed a public contract with two separable concerns. The workspace
recommends 300–400 lines per module as a guide, not a cap (owner guidance,
2026-09-22): cohesion decides the cut.

## Decision

1. **Cut along concerns, move items whole.** `propose.rs` keeps the report
   types and the neuron entry points; `propose/neurons.rs` holds the
   per-fact proposer (`neuron_schema`, `candidate_chunks`, `propose_one`,
   `ATTEMPTS`, `kebab`); `propose/blockers.rs` holds the blocker path.
   `directed.rs` keeps the contract types, `DIRECTED_POLICY`, the response
   type and `directed_ingest`; `directed/gates.rs` holds the pure evidence
   gate with its quote and endpoint matching; `directed/prompt.rs` holds the
   taxonomy check, system prompt and response schema.
2. **No behavior change, verified mechanically.** Every top-level item moved
   with its documentation; the only edits are `pub(super)` on helpers now
   shared across sibling modules and the formatting that follows from it.
   A lossless item parser compared the old and new item sets.
3. **Stable public paths.** `cognigraph_construct::propose::*`,
   `cognigraph_construct::directed::{gate_directed_proposals, DIRECTED_POLICY,
   DirectedProposal, …}` and the crate-root re-exports are unchanged; tests
   and server routes compile without edits.

## Consequences

- Resulting sizes: 216, 176 and 348 lines for the proposal modules; 204,
  239 and 80 for the directed modules.
- [CG-95](../issues/CG-95.md) (`draft.rs`, `grounding.rs`) follows the same
  approach; `grounding.rs` additionally needs its evaluation results
  compared, being the soundness core.
