---
name: backend-contract
description: Check or evolve the shared GraphBackend contract across Arango and native backends. Use when changing traits, backend semantics, query routing, or cross-backend behavior.
argument-hint: "[optional-focus]"
user-invocable: true
allowed-tools: Bash Read Grep Glob Edit MultiEdit Write
---

# Backend Contract

Use this workflow when a change affects more than one backend or the shared `GraphBackend` API.

## Scope

- The shared contract lives in `cognigraph-core`.
- Backend-specific implementations should expose the same observable semantics unless explicitly documented.
- Native is the strategic CGQL backend; ArangoDB uses AQL internally as the
  maintenance/conformance backend. Public opaque AQL remains disabled, including
  Lua `graph.query()` on an AQL backend. HTTP public query text remains parsed CGQL.
- Prefer capability detection through explicit types or methods over scattered backend-name checks.

## Workflow

1. Read the shared contract first:
   - `crates/cognigraph-core/src/traits.rs`
   - `crates/cognigraph-core/src/types/`
2. Inspect each affected backend:
   - `crates/cognigraph-arango/src/`
   - `crates/cognigraph-native/src/`
3. Check integration points:
   - `crates/cognigraph-server/src/`
   - `crates/cognigraph-lua/src/`
   - `crates/cognigraph-query/src/`
4. For any changed method behavior, add tests that can be repeated across backends where practical.
5. Keep `QueryLanguage` behavior explicit:
   - Arango reports AQL.
   - Native reports CGQL.
   - these declarations do not grant public opaque-query authority; preserve
     parsed public query execution and each surface's capability checks.
6. Update docs when the public behavior changes.

## Validation

Run focused backend and integration checks based on the touched code:

```bash
cargo test -p cognigraph-core
cargo test -p cognigraph-arango
cargo test -p cognigraph-native
cargo test -p cognigraph-server
```

When query or Lua routing changes, include the affected `cognigraph-query` and
`cognigraph-lua` tests as well.

For completed Rust changes, finish with the repository standard:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Follow [AGENTS.md](../../../AGENTS.md) for live verification and coverage reporting.
Credential-gated tests that return early do not establish live ArangoDB coverage.
For documentation-only changes, use the root documentation checks and exercise
any changed runnable examples.
