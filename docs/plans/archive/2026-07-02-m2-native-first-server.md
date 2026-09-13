# M2 — Native-First Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Make the native backend a first-class citizen end to end: capability-based health checks, explicit (not silent) hybrid-search behavior on non-AQL backends, a shared GraphBackend conformance suite, native contract fixes, and O(E) traversal.

**Architecture:** Capability detection through explicit trait methods (`ping()`, `query_language()`) per the backend-contract skill — never `backend_name()` string checks. The conformance suite lives in `cognigraph-core::contract` as plain async assertion functions over `&dyn GraphBackend`; native tests run it unconditionally, ArangoDB integration tests run it env-gated (`ARANGO_PASSWORD`).

**Tech Stack:** Rust 2024, existing crates only (no new dependencies).

---

### Task 1: `DocumentConflict` error + `ping()` capability in core

**Files:**
- Modify: `crates/cognigraph-core/src/error.rs` (new variant)
- Modify: `crates/cognigraph-core/src/traits.rs` (`ping()` default method)
- Modify: `crates/cognigraph-server/src/error.rs` (409 mapping + test)
- Modify: `crates/cognigraph-arango/src/backend.rs` (map 409, override `ping()`)

- [x] **Step 1: Failing test** — in `crates/cognigraph-server/src/error.rs` tests:

```rust
#[tokio::test]
async fn document_conflict_maps_to_409() {
    let err = AppError(CogniGraphError::DocumentConflict("docs/a".into()));
    let (status, _) = response_parts(err).await;
    assert_eq!(status, StatusCode::CONFLICT);
}
```

- [x] **Step 2: Implement** — core error variant:

```rust
#[error("Document conflict: {0}")]
DocumentConflict(String),
```

Trait method (after `backend_name`):

```rust
/// Cheap connectivity/liveness probe. Backends with a remote server
/// should override this; embedded backends can keep the default.
async fn ping(&self) -> Result<()> {
    Ok(())
}
```

Server mapping: `DocumentConflict(_) => (StatusCode::CONFLICT, ...)`. Arango `map_err`: `ArangoError::Server { code: 409, message } => CogniGraphError::DocumentConflict(message)`, and `ping()` override delegating to `self.query("RETURN 1", HashMap::new())` (AQL stays inside the Arango crate, where it belongs).

- [x] **Step 3: Run** — `cargo test -p cognigraph-core -p cognigraph-server -p cognigraph-arango` → PASS.

### Task 2: Native backend contract fixes

**Files:**
- Modify: `crates/cognigraph-native/src/memory.rs`
- Test: `crates/cognigraph-native/tests/native_backend.rs`

- [x] **Step 1: Failing tests**

```rust
#[tokio::test]
async fn create_document_with_duplicate_key_conflicts() {
    let backend = seeded_backend().await;
    let err = backend
        .create_document("documents", json!({ "_key": "a", "title": "Clone" }))
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::DocumentConflict(_)));
    // Original untouched:
    let doc = backend.get_document("documents", "a").await.unwrap().unwrap();
    assert_eq!(doc["title"], json!("Alpha"));
}

#[tokio::test]
async fn update_on_missing_collection_does_not_create_it() {
    let backend = NativeBackend::new();
    let err = backend
        .update_document("ghost", "a", json!({ "x": 1 }))
        .await
        .unwrap_err();
    assert!(matches!(err, CogniGraphError::CollectionNotFound(_)));
    // Collection must not have been created as a side effect:
    assert!(backend.list_documents("ghost", None, None).await.is_err());
}

#[tokio::test]
async fn collection_type_is_enforced_on_writes() {
    let backend = seeded_backend().await;
    // documents is a Document collection; relationships is an Edge collection.
    assert!(matches!(
        backend
            .create_edge("documents", json!({ "_from": "a/1", "_to": "b/2" }))
            .await
            .unwrap_err(),
        CogniGraphError::ValidationError(_)
    ));
    assert!(matches!(
        backend
            .create_document("relationships", json!({ "title": "not an edge" }))
            .await
            .unwrap_err(),
        CogniGraphError::ValidationError(_)
    ));
}
```

- [x] **Step 2: Implement** in `memory.rs`:
  - `create_document`/`create_edge`: if the key already exists in the collection, return `DocumentConflict("{collection}/{key}")`.
  - `update_document`/`replace_document`: look up the collection with `state.collections.get_mut(name)` and return `CollectionNotFound` when absent (no implicit creation).
  - Type check helper: when a collection already has a recorded `CollectionType` that contradicts the operation (`create_document` on Edge, `create_edge`/`upsert_edge` on Document), return `ValidationError`. Reads stay permissive.

- [x] **Step 3: Run** — `cargo test -p cognigraph-native` → PASS (existing CRUD/traversal tests unchanged).

### Task 3: Shared GraphBackend conformance suite

**Files:**
- Create: `crates/cognigraph-core/src/contract.rs`
- Modify: `crates/cognigraph-core/src/lib.rs` (`pub mod contract;`)
- Modify: `crates/cognigraph-native/tests/native_backend.rs` (run suite)
- Modify: `crates/cognigraph-arango/tests/integration.rs` (run suite, env-gated)

- [x] **Step 1: Implement `contract.rs`** — plain async assertion functions over `&dyn GraphBackend`, each taking a collection-name `prefix` so backends with real data are safe. Functions: `document_crud_contract`, `create_conflict_contract`, `missing_collection_update_contract`, `collection_type_contract`, `edges_and_traversal_contract` (directionality + inclusive depth range incl. depth 0), `vector_search_contract` (descending score order, limit, threshold). Each function creates its collections via `ensure_collection`, exercises behavior with `assert!`/`assert_eq!`, and drops its collections at the end.
- [x] **Step 2: Wire into native tests** — one `#[tokio::test]` per contract fn calling it against `NativeBackend::new()` with prefix `"contract"`.
- [x] **Step 3: Wire into Arango integration tests** — one `#[tokio::test]` running all contract fns when `backend()` returns `Some`, prefix `"contract_test"`.
- [x] **Step 4: Run** — `cargo test -p cognigraph-core -p cognigraph-native` → PASS (Arango part skips without env).

### Task 4: De-AQL the server routes

**Files:**
- Modify: `crates/cognigraph-server/src/routes/health.rs` (use `ping()`)
- Modify: `crates/cognigraph-server/src/routes/search.rs` (hybrid, graph-augmented, raw query)

- [x] **Step 1: Health** — `database_check` calls `state.backend.ping()` instead of `query("RETURN 1", ...)`; drop the now-unused `HashMap` import.
- [x] **Step 2: Hybrid search** — gate the BM25 leg on `state.backend.query_language() == QueryLanguage::Aql`:
  - AQL backend: run BM25 and propagate failures with `?` (no more `unwrap_or_default()` swallowing).
  - Non-AQL backend: skip BM25, `tracing::warn!` once, and include `"bm25": "skipped: backend does not support AQL full-text search"` in the response so degradation is explicit. RRF fusion proceeds with the vector leg only.
  - Remove the dead `documents_collection` request field and its default fn.
- [x] **Step 3: Graph-augmented search** — replace `if let Ok(paths) = ...traverse(...)` with a `match` that logs `tracing::warn!(error = %e, "graph augmentation traversal failed")` on `Err` and continues.
- [x] **Step 4: Raw query language check** — in `raw_query`, when `language` is provided, not `"cgql"`, and doesn't match `state.backend.query_language().as_str()`, return `ValidationError` (400) naming both languages.
- [x] **Step 5: Run** — `cargo test -p cognigraph-server` → PASS; `cargo check` clean.

### Task 5: O(E) traversal in the native backend

**Files:**
- Modify: `crates/cognigraph-native/src/memory.rs` (`traverse`)

- [x] **Step 1: Implement** — build a per-call adjacency map over the edge collection once (`HashMap<&str, Vec<&Value>>`, keyed by `_from` for Outbound, `_to` for Inbound, both for Any), then BFS consults the map instead of scanning every edge per dequeued vertex. Behavior is identical; existing traversal tests plus the contract suite are the safety net. (A persistent adjacency index is deliberately deferred to the M3 storage model, where the invariants live in one place.)
- [x] **Step 2: Run** — `cargo test -p cognigraph-native -p cognigraph-query` → PASS.

### Task 6: Documentation + final validation

- [x] Update `docs/implementation-plan.md`: M2 row → Done; note contract suite under Phase 6; hybrid-search behavior note under Phase 9.
- [x] Update `docs/architecture.md` if it documents `/health/database` or hybrid search behavior.
- [x] Final: `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all` → all green.
