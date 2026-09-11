---
name: native-backend-dev
description: Work on the native CogniGraph backend. Use when changing cognigraph-native, native storage behavior, traversal, vector search, persistence planning, or native query routing.
argument-hint: "[optional-focus]"
user-invocable: true
allowed-tools: Bash Read Grep Glob Edit MultiEdit Write
---

# Native Backend Development

Use this workflow for changes under `crates/cognigraph-native/` and native-backend integration points.

## Scope

- Treat `cognigraph-native` as the strategic backend; ArangoDB remains the
  maintenance/conformance reference.
- Preserve the delivered in-memory and redb-persistent modes, resident/paged
  storage, and optional vector sidecars. Follow the current
  [storage model](../../../docs/architecture/native-storage.md); documents are
  authoritative and indexes/sidecars have explicit rebuild and identity rules.
- Preserve the `GraphBackend` contract rather than creating native-only behavior leaks.
- `NativeBackend::query()` executes CGQL; public query surfaces must retain
  their parsed-query and authorization boundaries.
- Document compatibility, migration and recovery behavior before changing the
  persistence model. Preserve supported atomicity and single-writer boundaries.

## Workflow

1. Read the existing backend contract:
   - `crates/cognigraph-core/src/traits.rs`
   - `crates/cognigraph-core/src/types/`
   - `crates/cognigraph-native/src/`
   - `crates/cognigraph-native/tests/`
2. Compare behavior with `cognigraph-arango` when changing shared semantics.
3. Add or update tests for each affected operation:
   - document CRUD
   - schema collection handling
   - edge creation and lookup
   - traversal direction and depth behavior
   - vector search ordering and score behavior
   - CGQL query execution through `query()`
4. Update the owning architecture/storage guide when behavior changes, and
   `docs/implementation-plan.md` when status or scope changes. Keep the root
   README concise and link detailed contracts rather than duplicating them.

## Validation

For Rust backend changes, run focused native tests:

```bash
cargo test -p cognigraph-native
```

If query routing changed, also run:

```bash
cargo test -p cognigraph-query
```

For completed Rust changes, finish with the repository standard:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Follow [AGENTS.md](../../../AGENTS.md) for live verification, applicable modularity
checks and reporting of unexecuted integration coverage. Documentation-only
changes use its documentation checks and any affected runnable examples.
