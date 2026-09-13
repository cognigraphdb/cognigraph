# M4 — CGQL v2 + Pushdown + Native-Default Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Function registry with validation-time name/arity checks, LIMIT pushdown, native BM25 full-text search wired into hybrid search, `Option<f64>` vector threshold (no sentinel), native as the default backend — plus a file-driven CGQL test corpus (`*.cgql`) covering parse/validate/execute edge cases on BOTH engines.

**Scope decisions (deliberate deviations from the roadmap wording):**
1. **BM25 is a custom exact implementation, not tantivy.** Consistent with the project preference for custom implementations, with M3's brute-force vector precedent, and with the backend's current honest scale: BM25 (k1=1.2, b=0.75) computed over the collection at query time, deterministic and dependency-free. Tantivy becomes the *indexed* implementation later (Phase 10 / storage-model migration path), behind the same trait method.
2. **Filter pushdown is deferred; LIMIT pushdown ships.** The native backend runs CGQL in-process (nothing to push down to), and Arango is maintenance-mode — filter pushdown currently has no consumer that benefits. LIMIT/offset pushdown for unfiltered/unsorted collection scans is real and ships. Documented in `docs/implementation-plan.md`.

---

### Task 1: Function registry (`cognigraph-query/src/functions.rs`)

- [x] Registry of `FunctionDef { name, min_args, max_args, eval }`; lookup is case-insensitive.
- [x] Functions (lenient semantics — wrong argument *types* yield `null`; wrong *arity* is a validation error):
  - String: `LENGTH`, `UPPER`, `LOWER`, `TRIM`, `CONTAINS`, `STARTS_WITH`, `SUBSTRING(s, start[, len])` (char-based), `CONCAT(...)` (nulls skipped), `SPLIT`
  - Numeric: `ABS`, `FLOOR`, `CEIL`, `ROUND`
  - Array: `MIN`, `MAX`, `SUM` (empty → 0), `AVG` (empty → null), `FIRST`, `LAST`, `UNIQUE`
  - Object/type: `HAS`, `TYPENAME`
  - `LENGTH`: string → char count, array/object → element count, else null.
- [x] Validation: unknown function name and wrong arity are `ValidationError`s (`UnknownFunction`, `FunctionArity`); `VECTOR_SEARCH` stays reserved.
- [x] Executor `eval_function` dispatches through the registry.

### Task 2: `VectorSearchOpts.threshold: Option<f64>`

- [x] Core type change (serde default `Some(0.7)`); kills the `-1.0` sentinel in the CGQL executor (now `None`).
- [x] Native: `threshold.is_none_or(|t| score >= t)`. Arango: numeric bind `threshold.unwrap_or(-1.0)` — sentinel lives inside the Arango crate only. Server routes pass `Some(req.threshold)`; Lua default `Some(0.7)`; contract suite updated.

### Task 3: LIMIT pushdown for collection scans

- [x] Backend executor: when a plan has no filters and no sort, `list_documents(collection, Some(offset + count), None)`; the plan's own limit still applies afterwards (correct and bounded).
- [x] Test via the mock backend in `tests/backend_executor.rs` asserting the passed limit.

### Task 4: `GraphBackend::text_search` + native BM25

- [x] Trait: `async fn text_search(&self, collection, query, fields, limit) -> Result<Vec<SearchHit>>`, default `Err(BackendError("…does not support full-text search"))`.
- [x] Native impl: exact BM25 over string fields (tokenize: lowercase, split on non-alphanumeric), deterministic tie-break by `_key`, `source: "text"`. Tests: ranking, field selection, unicode tokens, empty query.

### Task 5: Hybrid search native path

- [x] `documents_collection` request field returns (default `"documents"`) with a real consumer: non-AQL backends call `text_search(documents_collection, query, search_fields, fetch_limit)`. AQL backends keep the ArangoSearch view path. `text_search` errors → explicit skip (as in M2).

### Task 6: Native becomes the default backend

- [x] `COGNIGRAPH_BACKEND` default `"arango"` → `"native"`; unknown values fall back to native with a warning. README + architecture updated.

### Task 7: CGQL test corpus (`*.cgql` files)

- [x] Layout under `crates/cognigraph-query/tests/corpus/`:
  - `parse_ok/*.cgql` — must parse + validate + plan
  - `parse_err/*.cgql` — must fail parsing
  - `validate_err/*.cgql` — must parse but fail validation
  - `exec/*.cgql` + `exec/*.json` — execute against the shared fixture, exact expected results (queries must be deterministic — use SORT)
  - `dataset.json` — shared fixture in `import_json` format (unicode, missing fields, nulls, nested objects, zero/negative/float numbers, empty strings, edges, embeddings)
  - Per-file bind variables via a `// binds: {...}` header comment (strict bind-var matching forbids superset injection).
- [x] Runner `tests/corpus.rs` in `cognigraph-query` (in-memory executor).
- [x] **Dual-engine equivalence**: runner `tests/cgql_corpus.rs` in `cognigraph-native` loads the same corpus, seeds via `import_json` (no stamping → byte-identical fixture), executes through `backend.query()`, asserts the same expected results. Exec queries project explicit fields (never a bare traversal `p` — path shapes legitimately differ across engines).

### Task 8: Docs + validation

- [x] `docs/cgql-v1.md`: function catalog with null semantics, LIMIT pushdown note. `docs/implementation-plan.md`: M4 row, Phase 5/9/10 updates, tantivy/filter-pushdown deferrals. README backend default.
- [x] `cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --all` green; commit.
