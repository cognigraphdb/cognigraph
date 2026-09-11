# M3 — Native Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Durable storage for the native backend per `docs/native-storage-model.md`: write-through redb persistence, startup load, JSON import/export, durability tests — with the `GraphBackend` contract and all existing tests unchanged.

**Architecture:** Memory-primary, redb-durable (see the storage-model doc — written first, per the native-backend-dev skill). One redb write transaction per backend write operation, committed under the memory write lock *before* memory mutates. `NativeBackend::new()` stays in-memory; `NativeBackend::open(path)` is persistent; the server selects via `COGNIGRAPH_NATIVE_PATH`.

**Tech Stack:** Rust 2024, redb (new dependency in `cognigraph-native` only).

---

### Task 1: `storage.rs` — redb store

**Files:**
- Create: `crates/cognigraph-native/src/storage.rs`
- Modify: `crates/cognigraph-native/Cargo.toml` (add redb), `src/lib.rs` (module)

- [x] `RedbStore::open(path)` — creates/opens the database, initializes `meta.schema_version = 1`, rejects files with a newer schema version.
- [x] `RedbStore::load()` — reads `collections` + `documents` tables into `NativeState` (missing tables → empty state).
- [x] `RedbStore::apply(&[StoreOp])` — one write transaction for `PutCollection` / `DropCollection` (removes its document range) / `PutDocument` / `DeleteDocument`.
- [x] Composite key helper `doc_key(collection, key)` with the `\u{0}` separator; `\u{0}` in names rejected at the write path.
- [x] All redb errors map to `CogniGraphError::BackendError`.

### Task 2: Write-through in `NativeBackend`

**Files:**
- Modify: `crates/cognigraph-native/src/memory.rs`

- [x] `NativeBackend { state, store: Option<RedbStore> }`; `new()` in-memory, `open(path)` loads state from disk.
- [x] Every write op persists before mutating memory: create/update/replace/delete document, create/upsert edge, ensure/drop collection. Implicit collection creation (create into a fresh collection) persists the collection record in the same transaction.
- [x] Reads unchanged.

### Task 3: JSON import/export

**Files:**
- Modify: `crates/cognigraph-native/src/memory.rs`

- [x] `export_json()` — snapshot per the storage-model doc.
- [x] `import_json(value)` — creates collections, inserts documents (overwrites), write-through when persistent.

### Task 4: Server wiring

**Files:**
- Modify: `crates/cognigraph-server/src/config.rs`, `src/main.rs`

- [x] `COGNIGRAPH_NATIVE_PATH` in config; the `"native"` backend branch opens persistently when set, logs the mode either way.

### Task 5: Tests

**Files:**
- Create: `crates/cognigraph-native/tests/persistence.rs`

- [x] Durability: open at temp path → write docs + edges → drop backend → reopen → all data present, types preserved, CGQL query works.
- [x] Delete/drop durability: deletions and dropped collections stay gone after reopen.
- [x] Import/export roundtrip: export from a seeded backend, import into a fresh one, exports are equal.
- [x] Conformance: `cognigraph_core::contract::run_all` against a persistent backend.
- [x] Temp dirs under `std::env::temp_dir()` with unique suffixes, cleaned up at test end (no tempfile dependency).

### Task 6: Docs + validation

- [x] `docs/implementation-plan.md`: Phase 6 persistence checkboxes, M3 milestone row → Done.
- [x] `docs/architecture.md`: native backend persistence + env var.
- [x] `README.md` env var table if present.
- [x] `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all` green; commit.
