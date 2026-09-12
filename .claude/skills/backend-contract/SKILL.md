---
name: backend-contract
description: Check or evolve GraphBackend semantics across Native storage modes, guarded and tenant-scoped wrappers, and capability/failure test doubles.
argument-hint: "[optional-focus]"
user-invocable: true
allowed-tools: Bash Read Grep Glob Edit MultiEdit Write
---

# Backend Contract

Use this workflow when a change affects the shared `GraphBackend` API, its
Native implementation or the wrappers that enforce access and tenant scope.

## Scope

- The shared contract lives in `cognigraph-core`; Native is the only runtime
  storage backend in both editions.
- Preserve shared observable semantics across memory, resident/embedded,
  resident/sidecar and paged/sidecar storage, including persistent reopen.
- Prefer explicit capabilities over backend-name conditionals. Failure doubles
  must expose unsupported guarantees honestly; never silently emulate atomicity.
- Public HTTP and Lua queries are parsed CGQL. Language declarations and roles
  never authorize opaque query passthrough.

## Workflow

1. Read `crates/cognigraph-core/src/traits.rs`, `src/types/` and `src/contract.rs`.
2. Inspect `crates/cognigraph-native/src/` and the affected guarded, routed,
   tenant-scoped or cache wrappers in `crates/cognigraph-server/src/`.
3. Check callers in `cognigraph-query`, `cognigraph-lua`, `cognigraph-construct`
   and the server as required by the changed behavior.
4. Reuse shared contract assertions and the fixed CGQL corpus. Storage modes
   share CGQL code; their agreement is not an independent language oracle.
5. Add focused assertions for persistence, atomic rollback, errors, budgets,
   cancellation and authorization where relevant. Expected read rows alone do
   not verify those behaviors. Use the counted no-access double for rejection
   that must precede storage access.
6. Update the owning contract guide and implementation plan when behavior changes.

## Validation

Run focused tests for the affected crates and storage modes. For completed Rust
changes, finish with the repository standard:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Follow [AGENTS.md](../../../AGENTS.md) for Enterprise checks, real-binary
verification, modularity and documentation checks. Report executed coverage
separately from intentionally skipped provider qualification. Preserve historical
captures and fixture bytes; new measurements belong in new evidence records.
