# Decision: split the construct proposal and directed modules by concern behind stable paths

Status: accepted and implemented 2026-09-23 for [CG-94](../issues/CG-94.md) and
[CG-95](../issues/CG-95.md).

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
- CG-95 applied the same approach to `draft.rs` and `grounding.rs`; see
  below.

## CG-95: drafting and grounding

4. **Same cut, same proof.** `draft.rs` keeps `DRAFT_REV`, `DraftReport` and
   the drafter entry points; `draft/prompts.rs` holds the two system prompts
   and schemas; `draft/finalize.rs` holds the finalization pass.
   `grounding.rs` keeps the public grounding contract (`effective_config`,
   `VetoRule`, `effective_vetoes`, `mentions`, `GroundedFact`,
   `ground_chunk`, `affirms_phrase`, `sentence_bounds`); `grounding/negation.rs`,
   `grounding/sentences.rs`, `grounding/semantics.rs` (`SEMANTICS_REV`) and
   `grounding/triggers.rs` hold the four supporting concerns. `merge_drafted`,
   `finalize_draft`, `SEMANTICS_REV` and `relation_semantics_signals` stay at
   their original paths through re-exports; moved struct fields became
   `pub(super)`.
5. **Grounding output is pinned, not only asserted.** Because `grounding.rs`
   is the soundness core, a new test records everything the four public
   Semantic Neurons kits construct (chunks, entities, mentions, facts, fact
   semantics, evaluation outcome) in `tests/snapshots/grounding-kits.json`,
   recorded on the unsplit code and unchanged after the split. Removing a
   single negation cue makes it fail on the negative-probe kit. Intentional
   grounding changes regenerate it with `CG_UPDATE_SNAPSHOT=1`, which makes
   such a change visible in review.

Sizes after CG-95: `draft.rs` 321, `draft/finalize.rs` 318, `draft/prompts.rs`
69; `grounding.rs` 343, `grounding/negation.rs` 129, `grounding/semantics.rs`
127, `grounding/sentences.rs` 91, `grounding/triggers.rs` 139 lines.
