# Decision: 300–400 LOC file modularity convention

**Date:** 2026-07-03
**Owner:** skitsanos (convention), Claude (execution timing)

## Context

The workspace grew feature-first through M1–M14; several files accreted well
past comfortable size (worst offenders at the time of this decision:
`cognigraph-native/src/memory.rs` 1964 LOC, `cognigraph-query/src/executor.rs`
1282 LOC; 12 files total above 400 LOC out of 85 / ~20.3K LOC). Long files
raise cognitive load for human review and are measurably harder for AI agents
to edit safely — this project's recurring "silent no-op replace after rustfmt"
failure class hit almost exclusively in the longest files.

## Decision

- Source and test files target **300–400 LOC**; ~450 is a soft cap when a
  logical unit would otherwise be split mid-seam. Coherence beats the number:
  never split a tightly coupled unit just to satisfy the cap.
- Splits are **pure code motion**: public APIs stay identical (module dirs
  with `mod.rs` re-exports so external paths keep compiling), no logic
  rewrites, no renames of tests.
- Applied as a dedicated refactor pass between feature arcs (clean tree, all
  gates green), executed by a team of parallel agents — one per crate so file
  sets are disjoint — with full-workspace gates before the single commit.
- Ongoing: new code follows the convention; a file crossing ~450 LOC during
  feature work is split in that same arc, not deferred.

## Outcome

- 2026-07-03: initial pass executed by five parallel agents (one per crate,
  disjoint file sets). All 12 offending files split or consciously kept:
  - native: `memory.rs` 1964 → 5 modules; tests → 4 themed files
  - query: `executor.rs` 1282 → 5 modules; `parser.rs`/`functions.rs` split;
    corpus suite untouched and green on both engines
  - server: `routes/search.rs` 856 → 5 modules
  - arango: `backend.rs`/`client.rs` → module dirs with extracted tests
  - core: `types.rs` 571 → 4 submodules, glob re-exports, zero downstream edits
  - cache: `cache_behavior.rs` (402) deliberately left whole — coherent suite,
    splitting would only duplicate helpers
  - Full-workspace gates after merge: 286 tests green, clippy `-D warnings`
    clean, fmt clean. Purity verified per-crate by diffing moved bodies
    against `HEAD` (byte-identical logic; only `use`/`mod`/`pub(super)` added).
- Known residue (both single `impl` trait blocks, unsplittable across files
  without delegation shims — out of scope for pure code motion):
  `native/src/memory/backend.rs` (1009), `arango/src/backend/graph_backend.rs`
  (463). Candidate follow-up: extract inherent helper methods from the native
  `GraphBackend` impl as a separate, behavior-reviewed pass.
- 2026-07-03 (follow-up executed): the native residue is resolved. Every
  method body except `query` was synchronous, so the bodies moved verbatim
  into sync inherent `*_impl` methods — `memory/{documents,edges,batch,
  search}.rs` — and the trait impl became a 178-LOC delegation layer that
  reads as the backend's API surface. Two deliberate cleanups beyond pure
  motion: the duplicated projection closure became `helpers::project_fields`,
  and `text_search`'s stale pre-tantivy doc paragraph ("deterministic and
  index-free") was dropped. Gates: 286 tests green, clippy/fmt clean. The
  arango 463-LOC impl stays as-is (13 lines over soft cap, maintenance-mode
  crate — delegation indirection not worth it there).

- 2026-07-17 status check: the convention is no longer consistently enforced.
  Current files above the soft cap include `server/src/routes/construct.rs`
  (2,236 LOC), `construct/src/bin/dailymed-pilot.rs` (1,094),
  `construct/src/propose.rs` (863), `query/src/executor/materialize.rs` (862),
  and `server/src/tenancy.rs` (808). These are implementation debt, not evidence
  that the original modularity outcome still describes the whole workspace.


## CG-26 scoped refactor — 2026-09-09

The six server hotspots named in [CG-26](../issues/CG-26.md) now use module
directories: `promotions`, `jobs`, `governance`, `materialized_repairs`,
`artifact_consumption`, and `routes/construct`. Their `mod.rs` files preserve
existing caller paths while separating wire contracts, validation, transitions,
recovery, storage, and themed tests. Existing test names, protocol fields,
signature domains, state transitions, and lock boundaries are preserved.

Of 240 source/test files in these families, 233 fit the 450-line soft cap.
Seven existing indivisible functions/scenarios are explicitly retained: two
production execution methods, four full lifecycle scenarios, and one fixture
factory. Their [size budgets and reasons](../../scripts/policies/server-modularity-exceptions.json)
prevent unreviewed growth; coherence takes precedence over mechanical line
splitting. The 2,019-line M26 scenario remains one test so its shared state and
failure sequencing are not redesigned in a mechanical refactor.

Run `python3 scripts/check-server-modularity.py` alongside the Rust gates.
The manual CI workflow runs this check too. It covers the six named families;
it does not assert that the entire workspace now satisfies the convention.
Sixteen other server source/test files remain above the cap, including
`semantic_repairs.rs`, `artifact_attestations.rs`, `system_collections.rs`, and
`governance_tests.rs`; those are outside these six refactor units.

The [verification report](../issues/server-modularity-2026-09-09.md) records
code-motion equivalence, exact workspace gates, authenticated release HTTP
replay/recovery, corrected verification attempts, and the remaining scope.
