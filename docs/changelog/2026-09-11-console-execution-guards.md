# Guard console executions across buttons and shortcuts

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-55 with a synchronous in-flight slot shared by Lua/CGQL Run buttons,
editor shortcuts and form submission. Only the owning execution publishes its
result, history, timing, notification and busy state. Departed screens/connections
discard late completions; requests release their slot after success or failure.

## Verification

UI lint/types, 137 tests and the production build pass. A real Enterprise/Native
release binary and controlled response latency verified repeated shortcuts,
CGQL form submission, one Lua write per intentional run, persisted reloads,
server-error recovery, invalid bindings and completion ownership.
[Evidence and boundaries](../evidence/ui-2026-09-11-execution-guards.md#artifact-ba0883e40b9f39017230).
No Rust source changed; no push or publication was performed.
