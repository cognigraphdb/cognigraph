# Components and backend contracts

This page describes the current implementation, including the 2.7.x Arango
adapter. [Native-only storage](../decisions/decision_native_only.md) is now the
approved direction; [CG-65/CG-67/CG-68](../plans/native-only-2026-09-12.md) cover
preserved Native contracts, removal and first-deployment readiness. The component map below is not a commitment
to continued Arango runtime support.

## Workspace Structure

```
cognigraph/
├── Cargo.toml                    # Workspace root
├── docs/                         # Documentation
├── crates/
│   ├── cognigraph-core/          # Core traits, types, error handling
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── contract.rs       # Shared GraphBackend conformance suite
│   │       ├── types/            # Document, graph, predicate, search, storage types
│   │       ├── traits.rs         # GraphBackend trait
│   │       └── error.rs          # CogniGraphError enum (thiserror)
│   ├── cognigraph-query/         # CGQL: pest grammar, AST, validation, planner, executors
│   │   └── src/
│   │       ├── grammar.pest      # CGQL grammar (keywords, expressions, mutations)
│   │       ├── ast.rs            # Query/Expr/CollectClause/MutationClause
│   │       ├── parser/           # pest → AST with positional errors
│   │       ├── validation.rs     # Scope, reserved words, functions, limits
│   │       ├── planner.rs        # LogicalPlan (serializable)
│   │       ├── functions/        # 30+ built-in functions
│   │       └── executor/         # In-memory + GraphBackend executors, QueryMode
│   ├── cognigraph-native/        # Native backend: memory-primary, redb-durable
│   │   └── src/
│   │       ├── memory/           # GraphBackend impl, CRUD, traversal, search, batches
│   │       ├── storage.rs        # redb write-through store (see native-storage.md)
│   │       ├── sidecar.rs        # Rebuildable mmap vector sidecar
│   │       └── text_index.rs     # Rebuildable Tantivy BM25 index
│   ├── cognigraph-auth/          # Auth & RBAC over any GraphBackend
│   │   └── src/
│   │       └── lib.rs            # Users/tokens in _users/_tokens, argon2, scopes
│   ├── cognigraph-arango/        # ArangoDB backend (maintenance mode, contract reference)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client/           # HTTP client for ArangoDB REST API
│   │       ├── backend/          # GraphBackend trait implementation
│   │       └── database.rs       # Schema initialization
│   ├── cognigraph-cache/         # Semantic query cache with continuous influence
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs         # QueryCache trait
│   │       ├── types.rs          # CacheConfig, CacheKey, CacheHit, CacheStats
│   │       ├── memory.rs         # InMemoryCache (LRU + similarity-aware)
│   │       ├── persistent.rs     # redb-persistent embedding cache
│   │       ├── normalize.rs      # Query normalization
│   │       └── similarity.rs     # Cosine similarity
│   ├── cognigraph-embeddings/    # Embedding providers
│   │   └── src/
│   │       ├── lib.rs            # EmbeddingProvider trait
│   │       ├── openai.rs
│   │       ├── ollama.rs
│   │       ├── gemini.rs
│   │       └── completion.rs     # Structured OpenAI/Gemini completion providers
│   ├── cognigraph-construct/     # Semantic Neurons construction plus deterministic UTF-8 preparation
│   ├── cognigraph-governance/    # Canonical Ed25519 statements/signing/verification primitives
│   ├── cognigraph-cli/           # HTTP administration CLI
│   ├── cognigraph-lua/           # Lua scripting engine
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── runtime.rs        # LuaEngine: sandboxed VM, instruction limits
│   │       ├── control.rs        # Shared deadline, instruction count, cancellation
│   │       └── bindings.rs       # Graph primitives exposed to Lua
│   ├── cognigraph-artifacts/     # Shared M21-M24 CAS verification and offline custody bundles
│   │   └── src/
│   │       ├── cas.rs            # Read-only tenant-scoped content-address verification
│   │       └── recovery.rs       # Canonical plans, closed bundles, receipts, and absent-scope restore
│   └── cognigraph-server/        # HTTP API (Axum)
│       └── src/
│           ├── main.rs           # Entry point: backend/auth/cache init, layers
│           ├── config.rs         # Environment variable configuration
│           ├── state.rs          # AppState (backend + providers + cache + auth)
│           ├── jobs/             # Durable tenant-scoped queue, recovery, checkpoints
│           ├── promotions/       # Evaluation evidence, decisions, and derived heads
│           ├── governance/       # Signed trust registry, policy, approval, and intent
│           ├── artifact_attestations.rs # M20 signed exact-byte manifests and context bindings
│           ├── artifact_cas.rs   # Compatibility re-exports for shared CAS verification
│           ├── artifact_consumption/ # M21 consumption, M22 derivation, and M23 preparation
│           ├── materialized_repairs/ # M26 generation, deployment, snapshot recovery
│           ├── artifact_custody.rs # M24 read-only evidence recovery-plan derivation
│           ├── tenancy.rs        # Per-tenant backend/cache routing facade
│           ├── error.rs          # CogniGraphError → HTTP status mapping
│           ├── auth_middleware.rs# Bearer token + ScopePolicy checks
│           ├── hardening.rs      # /metrics + per-IP rate limiting
│           └── routes/
│               ├── mod.rs
│               ├── documents.rs  # Document CRUD
│               ├── graph.rs      # Relationship upsert, edges, traversal
│               ├── search/       # Vector, semantic, hybrid, graph-augmented search
│               ├── construct/    # Ground/evaluate/propose/review/draft routes
│               ├── neurons.rs    # Neuron lifecycle and graduation routes
│               ├── query.rs      # Read-write CGQL (mutations)
│               ├── batch.rs      # Atomic backend batches
│               ├── tenants.rs    # Host-admin tenant lifecycle
│               ├── admin/      # Snapshot export/import
│               ├── users.rs      # User + API token management
│               ├── cache.rs      # Cache stats and clear endpoints
│               ├── health.rs     # Health check, database connectivity
│               ├── jobs/       # Submit/list/status/cancel/retry API
│               ├── governance/ # Signed key/policy/artifact authority API
│               ├── promotions/ # Evidence, decision, head, and rollback API
│               └── lua.rs        # Lua script execution
```

---

The six CG-26 server module families expose their existing paths through `mod.rs`,
with contracts, validation, lifecycle operations, and themed tests in child
modules. See the [modularity report](../issues/server-modularity-2026-09-09.md) for
the source map, mechanical-movement checks, and documented size exceptions.

## Core Traits

### GraphBackend

The central storage abstraction. Every database backend implements this trait, making the rest of the system database-agnostic.

```rust
#[async_trait]
pub trait GraphBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    fn query_language(&self) -> QueryLanguage;  // Cgql for native, Aql for Arango
    fn supports_atomic_batches(&self) -> bool;  // explicit capability, false by default
    async fn ping(&self) -> Result<()>;         // backend-agnostic health probe

    // Document operations
    async fn create_document(&self, collection: &str, doc: serde_json::Value) -> Result<DocumentId>;
    async fn get_document(&self, collection: &str, key: &str) -> Result<Option<serde_json::Value>>;
    async fn update_document(&self, collection: &str, key: &str, update: serde_json::Value) -> Result<serde_json::Value>;
    async fn replace_document(&self, collection: &str, key: &str, doc: serde_json::Value) -> Result<serde_json::Value>;
    async fn delete_document(&self, collection: &str, key: &str) -> Result<bool>;
    async fn list_documents(&self, collection: &str, limit: Option<usize>, offset: Option<usize>) -> Result<Vec<serde_json::Value>>;
    async fn list_documents_projected(/* ... */) -> Result<Vec<serde_json::Value>>;
    async fn list_documents_after_key(/* collection, exclusive key, projection, limit */) -> Result<Vec<serde_json::Value>>;
    async fn list_documents_filtered(/* ... */) -> Result<Vec<serde_json::Value>>;
    async fn export_snapshot(&self) -> Result<serde_json::Value>;
    async fn import_snapshot(&self, data: &serde_json::Value) -> Result<()>;

    // Edge operations
    async fn create_edge(&self, collection: &str, edge: serde_json::Value) -> Result<DocumentId>;
    async fn upsert_edge(&self, collection: &str, from: &str, to: &str, relation_type: &str, data: serde_json::Value) -> Result<serde_json::Value>;
    async fn get_edges(&self, collection: &str, vertex_id: &str, direction: Direction) -> Result<Vec<serde_json::Value>>;
    async fn execute_batch(&self, ops: Vec<BatchOp>) -> Result<Vec<serde_json::Value>>;

    // Graph traversal
    async fn traverse(&self, start_vertex: &str, opts: &TraversalOpts) -> Result<Vec<TraversalPath>>;

    // Vector search
    async fn vector_search(&self, collection: &str, query_vector: &[f64], opts: &VectorSearchOpts) -> Result<Vec<SearchHit>>;

    // Full-text search (native BM25; capability method, default = unsupported)
    async fn text_search(&self, collection: &str, query: &str, fields: &[String], limit: usize) -> Result<Vec<SearchHit>>;

    // Raw query in the backend's language (CGQL for native, AQL for Arango)
    async fn query(&self, query: &str, bind_vars: HashMap<String, serde_json::Value>) -> Result<Vec<serde_json::Value>>;

    // Schema management
    async fn ensure_collection(&self, name: &str, collection_type: CollectionType) -> Result<()>;
    async fn ensure_index(&self, collection: &str, index: &IndexDef) -> Result<()>;
    async fn drop_collection(&self, name: &str) -> Result<()>;
}
```

### EmbeddingProvider

Abstracts over different embedding services and models.

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>>;
    fn dimension(&self) -> Option<usize> { None }
    fn model_name(&self) -> &str { "default" }
}
```

### QueryCache

Semantic query cache with pluggable backends. Returns the best matching cached results with similarity score; the caller decides behavior based on similarity. `CacheKey` carries a params fingerprint of every result-shaping request parameter (threshold, limit, fusion weights, traversal knobs), and the similarity index buckets by (collection, mode, params) — lookups are fuzzy on the query text, never on parameters.

```rust
#[async_trait]
pub trait QueryCache: Send + Sync {
    async fn get_embedding(&self, query: &str, model: &str) -> Option<Vec<f64>>;
    async fn put_embedding(&self, query: &str, model: &str, embedding: Vec<f64>);
    async fn get_results(&self, key: &CacheKey, query_embedding: Option<&[f64]>) -> Option<CacheHit>;
    async fn put_results(&self, key: &CacheKey, query_embedding: Option<Vec<f64>>, results: Vec<SearchHit>, meta: Option<serde_json::Value>);
    async fn invalidate(&self, key: &CacheKey);
    async fn invalidate_collection(&self, collection: &str);
    async fn clear(&self);
    async fn entry_count(&self) -> usize;
    fn stats(&self) -> &CacheStats;
    fn stats_snapshot(&self) -> CacheStatsSnapshot;
    fn config(&self) -> &CacheConfig;
}
```

---

## Data Flow

```
Client Request → Rate Limit → Auth/RBAC → Axum Router → Handler → GraphBackend trait
                                                                       → NativeBackend (memory + redb) | ArangoDB
                                    │
                                    ├── EmbeddingProvider trait → OpenAI / Ollama
                                    │
                                    ├── QueryCache trait → InMemoryCache (LRU + similarity)
                                    │
                                    └── LuaEngine (optional) → Sandboxed Lua VM
```

Durable governed work branches after authorization: the handler freezes its
operation input and commits one protected `_cognigraph_jobs` document. A single
process-local dispatcher executes tenant queues round-robin through explicit
`TenantScoped` backends. Native ingestion yields after each atomic chunk-batch
checkpoint; startup requeues an interrupted record and may replay its last
idempotent batch. Evaluations remain non-preemptible operations. Job state and
transition history share one document so their replacement is atomic on native
and maintenance-mode ArangoDB. Persistent storage provides restart recovery,
not multi-process coordination or HA.

The hot job record and `_cognigraph_job_archive` are authoritative. The
`_cognigraph_job_catalog` contains only projected summaries ordered by a stable
tenant/incarnation plus inverse-created-time key; cursor listing scans it by
exclusive key. Cursor v2 base64url-encodes that opaque position so even a
malformed punctuation-bearing catalog alias remains resumable during repair;
safe v1 cursors remain readable. Catalog writes are derived and repairable: a
failure degrades `/health/jobs`, while bounded reconciliation rebuilds missing
or stale rows and deletes orphans and noncanonical scoped aliases from the two
authoritative collections. Archival copies a
terminal job to the immutable archive before updating the catalog and deleting
the hot record. Retention never means destructive purge in M17.

### Request Lifecycle

1. **Client Request** arrives at the Axum HTTP server.
2. **Router** dispatches to the appropriate handler based on path and method.
3. **Handler** accesses shared `AppState` containing backend, embedder, and cache.
4. For search routes: **Cache** is checked first. Strong similarity hits are returned directly. Above-floor weaker matches are merged with fresh search results via RRF fusion with continuous weighting; at or below the floor the fresh result is used without claiming cache assistance.
5. **GraphBackend** dispatches typed operations and server-authored queries to the selected native or maintenance-mode ArangoDB backend. Public query text is parsed CGQL; opaque AQL is not exposed over HTTP or Lua.
6. Server mutation-capable routes perform **dependency-safe result-cache invalidation** after the attempt (or after commit for typed single operations). This includes read-write CGQL and write-capable Lua. Writes made outside the server require an explicit result-cache clear.
7. **Response** is serialized as JSON and returned to the client.

Job submission returns after the durable record is committed. Later job work
does not inherit request-local tenant state: it carries the captured tenant and
incarnation explicitly, and tenant suspension/deletion fences the worker before
the store can be retired.

For Lua script execution, the Lua runtime is invoked with graph primitives bound to the active backend.

---

## Custom ArangoDB Client

Purpose-built rather than relying on the semi-maintained `arangors` crate. Minimal surface area, fully understood, fully controlled.

### Supported Operations

| Category | Operations |
|---|---|
| **Authentication** | Basic auth (base64), bearer token (JWT) |
| **Documents** | Create, read, update, replace, delete, list |
| **AQL** | Execute queries via `/_api/cursor`, bind variables |
| **Collections** | Create, drop, ensure (idempotent) |
| **Indexes** | Create (persistent, hash, fulltext, geo, inverted, vector) |
| **Vector Search** | `APPROX_NEAR_COSINE` (native) or `COSINE_SIMILARITY()` (fallback) |

---
