# Keep console results attached to their executed inputs

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-56 by clearing Query, Lua, search and graph results/timings on input
changes and reruns. Only the current execution may publish a completion. Graph
shows persistent loading/errors in both views and removes stale inspectors and
paths; expansion and relationship readback retain the correct result scope.

## Verification

UI lint/types, 144 tests and production build pass. Real Enterprise/Native browser
checks verify input edits, delayed successes/errors, reversed vector/graph
completion, graph connection failures/retry, expansion, relationship creation
and persisted reloads. Error layouts pass at effective 125%/150% desktop sizes.
[Evidence and limits](../evidence/ui-2026-09-11-result-ownership.md#artifact-8aa8b46fb43543771a2c).
No Rust source changed; no push or publication was performed.
