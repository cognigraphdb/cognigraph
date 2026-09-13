# CogniGraph Architecture

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
│   │       ├── storage.rs        # redb write-through store (see native-storage-model.md)
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
│               ├── admin.rs      # Snapshot export/import
│               ├── users.rs      # User + API token management
│               ├── cache.rs      # Cache stats and clear endpoints
│               ├── health.rs     # Health check, database connectivity
│               ├── jobs.rs       # Submit/list/status/cancel/retry API
│               ├── governance.rs # Signed key/policy/artifact authority API
│               ├── promotions.rs # Evidence, decision, head, and rollback API
│               └── lua.rs        # Lua script execution
```

---

The six CG-26 server module families expose their existing paths through `mod.rs`,
with contracts, validation, lifecycle operations, and themed tests in child
modules. See the [modularity report](issues/server-modularity-2026-09-09.md) for
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

## Semantic Query Cache

The cache is not a simple key-value store. It is a **continuous retrieval signal** that accumulates knowledge from previous queries.

### How It Works

1. **Embedding cache**: Maps (query text, model) → embedding vector, avoiding redundant API calls.
2. **Result cache**: Maps (collection, search_mode, parameters, normalized query)
   → search results. Similarity lookup requires exact parameter identity.
   Hybrid, semantic, and graph-augmented search use versioned JSON parameters,
   preserving collection/view names, ordered field arrays, and optional values
   (CG-31/CG-32). An omitted/null semantic model filter accepts every model;
   an empty string matches only that literal model name and has a distinct key.
3. **Continuous influence**: Cache weight follows a cubic curve based on cosine similarity between query embeddings:
   - `weight = ((similarity - floor) / (1 - floor))³`
   - High similarity (0.97+) → return cached results directly (fast path)
   - Above-floor lower similarity → merge cached signal with fresh results via RRF, weighted by the curve
   - At or below the configured floor → weight 0, fresh retrieval only
   - Per-document rank decay ensures top cached results influence more than tail results
4. **Continuous when enabled**: The cache participates between the configured floor and strong-hit threshold instead of acting only as a binary hit/miss layer. The cache itself is disabled by default.
5. **Invalidation**: Managed document, relationship, construction,
   neuron-transition, batch, read-write CGQL, and write-capable Lua routes
   invalidate result caches conservatively. An
   invalidation generation prevents an older in-flight search from repopulating
   stale results. Direct storage writes outside the server require explicit
   clearing.

### Cache-Assisted Retrieval (Semantic Search)

```
Query → Embed → Check Cache → Best Match Found?
                                  │
                    ┌──────────── YES ──────────────┐
                    │                               │
              sim >= 0.97?                    weight = cubic(sim)
                    │                               │
                   YES                    Run fresh vector search
                    │                               │
            Return cached               RRF merge: cached (weight)
            results directly            + fresh (1.0 - weight)
                                                    │
                                             Return merged results
```

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

## Lua Query Engine

### Runtime Configuration

- **Engine:** mlua with LuaJIT. Construction verifies JIT is disabled and
  propagates sandbox/hook setup failures.
- **Sandboxing:** `os`, `io`, `debug`, `jit`, `load`, `loadstring`, `loadfile`,
  `dofile`, `require`, and `package` are removed; host scripts are text-only.
- **Resource limits:** A shared hook checks every 1,000 Lua instructions
  (default cap 1M), including coroutines. Limits reset per execution. Resource
  termination escapes `pcall`/`xpcall` through a typed host unwind. Both query
  permission modes apply the server's CGQL row budget, and all callbacks share
  one script deadline. Dropping the HTTP request signals cancellation; a
  supervisor joins the worker and invalidates the caller's result cache.
  Cancellation is cooperative at hooks/future yields: synchronous backend
  operations must return, and already committed writes are not rolled back.
- **Authorization:** read primitives require `lua:execute`; typed CRUD, edge,
  and batch mutations additionally require `documents:write`. `graph.query()`
  is available only when the active backend uses parsed CGQL; opaque AQL is
  disabled for every role. With auth disabled, Lua graph writes remain disabled.

### Exposed Primitives

| Function | Description |
|---|---|
| `graph.query(query, bind_vars)` | Parsed CGQL query; disabled on AQL backends |
| `graph.get_document(collection, key)` | Fetch single document |
| `graph.find_documents(collection, opts)` | List with limit/offset |
| `graph.create_document(collection, doc)` | Create document |
| `graph.update_document(collection, key, merge)` | Partially update document |
| `graph.replace_document(collection, key, doc)` | Replace document |
| `graph.delete_document(collection, key)` | Delete document |
| `graph.traverse(start_vertex, opts)` | Multi-hop traversal with scoring |
| `graph.neighbors(vertex_id, direction?, collection?)` | Get edges |
| `graph.upsert_edge(from, to, type, data?, collection?)` | Create/update edge |
| `graph.similarity(collection, vector, opts)` | Vector search |
| `graph.text_search(collection, query, fields?, limit?)` | BM25 text search when supported by the backend |
| `graph.batch(ops)` | Execute an atomic backend batch |

---

## Configuration

All configuration is via environment variables, with `.env` file support via `dotenvy`.

### Server & Backend

| Variable | Description | Default |
|---|---|---|
| `COGNIGRAPH_HOST` | HTTP server bind address | `0.0.0.0` |
| `COGNIGRAPH_PORT` | HTTP server port | `3000` |
| `COGNIGRAPH_BACKEND` | Database backend: `native` or `arango` (maintenance) | `native` |
| `ARANGO_URL` | ArangoDB connection URL | `http://localhost:8529` |
| `ARANGO_DB` | ArangoDB database name | `cognigraph` |
| `ARANGO_USER` | ArangoDB username | `root` |
| `ARANGO_PASSWORD` | ArangoDB password | (empty) |
| `COGNIGRAPH_VECTOR_SEARCH_MODE` | ArangoDB vector search: `native` (model filtering requires ≥3.12.6) or `fallback`; filter before candidate selection and expand candidates for parent deduplication | `native` |
| `COGNIGRAPH_NATIVE_PATH` | redb storage path for the native backend; in-memory when unset | (unset) |
| `COGNIGRAPH_DATA_DIR` | Multi-tenant mode: one redb store per tenant plus a separate control store; mutually exclusive with `COGNIGRAPH_NATIVE_PATH` | (unset) |
| `COGNIGRAPH_JOB_INGEST_BATCH_SIZE` | Chunks per durable construction checkpoint | `25` |
| `COGNIGRAPH_JOB_MAX_ACTIVE_TOTAL` | Maximum nonterminal jobs across the process | `1000` |
| `COGNIGRAPH_JOB_MAX_ACTIVE_PER_TENANT` | Host cap for nonterminal jobs per tenant; tenant quota may lower it | `100` |
| `COGNIGRAPH_JOB_RETENTION_SECS` | Terminal-job age before default archive eligibility | `2592000` |
| `COGNIGRAPH_JOB_ARCHIVE_BATCH_SIZE` | Default bounded archive operator batch | `100` |
| `COGNIGRAPH_VECTOR_MODE` | `embedded` or mmap `sidecar` vectors | `embedded` |
| `COGNIGRAPH_STORAGE_MODE` | `resident` or redb-primary `paged` documents | `resident` |
| `COGNIGRAPH_CACHE_BYTES` | Paged document-cache budget | `268435456` |

The `native` backend speaks CGQL and is memory-primary. With
`COGNIGRAPH_NATIVE_PATH` set it is durable: every write is committed to a
redb database before memory mutates, and the database is loaded on startup.
See `docs/native-storage-model.md` for the storage model, schema, and
migration path.

### Embedding and completion providers

| Variable | Description | Default |
|---|---|---|
| `COGNIGRAPH_EMBEDDING_PROVIDER` | Provider: `openai`, `ollama`, `gemini`, or `none` | `none` |
| `OPENAI_API_KEY` | OpenAI API key | (required if provider=openai) |
| `OPENAI_BASE_URL` | OpenAI-compatible base URL for completion, side-views, and both dedicated review judges; validated absolute HTTP(S), without query/fragment | `https://api.openai.com/v1` |
| `COGNIGRAPH_EMBEDDING_MODEL` | Model name override | Provider default |
| `COGNIGRAPH_COMPLETION_PROVIDER` | Construction/fallback-judge provider: `openai` or `gemini`; explicit settings require that provider's key | Nonempty OpenAI key, then Gemini key; neither = disabled |
| `COGNIGRAPH_COMPLETION_MODEL` | Completion model override shared by server and harnesses; Luna uses `reasoning_effort: none` | `gpt-5.6-luna` for OpenAI; `gemini-3.8-flash` for Gemini |
| `COGNIGRAPH_SIDEVIEWS_PROVIDER` | Optional independent side-view provider; explicit selection uses its own default model unless overridden | Inherit completion provider |
| `COGNIGRAPH_SIDEVIEWS_MODEL` | Side-view model override | Inherit completion model, or separate provider default |
| `GEMINI_BASE_URL` | Gemini completion base URL override shared by server and harnesses | `https://generativelanguage.googleapis.com/v1beta` |
| `COGNIGRAPH_JUDGE_MODEL` | Dedicated OpenAI review-judge model; shares `OPENAI_API_KEY`/`OPENAI_BASE_URL`. Unset/blank uses completion fallback; explicit model requires a nonempty OpenAI key | unset |
| `COGNIGRAPH_JUDGE_PARTNER_MODEL` | Dedicated OpenAI Lane A+ partner using the same key/base URL; explicit model requires a nonempty key. Unset/blank = A+ degrades to queue | unset |
| `GEMINI_API_KEY` | Gemini key for embeddings or construct/eval completion | (required when selected) |
| `OLLAMA_BASE_URL` | Ollama server URL | `http://localhost:11434` |

### Query Cache

| Variable | Description | Default |
|---|---|---|
| `COGNIGRAPH_QUERY_CACHE_ENABLED` | Enable semantic query cache | `false` |
| `COGNIGRAPH_QUERY_CACHE_BACKEND` | Cache backend type | `memory` |
| `COGNIGRAPH_QUERY_CACHE_TTL_SECS` | Time-to-live for cache entries | `900` |
| `COGNIGRAPH_QUERY_CACHE_MAX_ENTRIES` | Maximum LRU cache size | `1000` |
| `COGNIGRAPH_QUERY_CACHE_SIMILARITY_FLOOR` | Minimum similarity for cache influence | `0.7` |
| `COGNIGRAPH_QUERY_CACHE_STRONG_THRESHOLD` | Similarity for direct return (fast path) | `0.97` |

---

## API Endpoints

All application endpoints are served under the `/api` prefix (the UI owns
`/`); only `/health`, `/metrics`, and `/openapi.yaml` stay at the root.

### Documents
| Method | Path | Description |
|---|---|---|
| GET | `/api/collections` | Collection catalog: names, types ("document"/"edge"), counts; system (`_`-prefixed) collections hidden |
| POST | `/api/collections` | Idempotently create an empty document or edge collection |
| DELETE | `/api/collections/{name}` | Drop a non-system collection and all of its contents |
| POST | `/api/documents` | Create document |
| GET | `/api/documents` | List documents |
| POST | `/api/documents/embed` | Provider-batched embedding plus one atomic store transaction |
| GET | `/api/documents/{collection}/{key}` | Get document |
| PATCH | `/api/documents/{collection}/{key}` | Update document (partial) |
| PUT | `/api/documents/{collection}/{key}` | Replace document |
| DELETE | `/api/documents/{collection}/{key}` | Delete document |

The public backend facade treats `space_types`, `neurons`,
`review_policies`, `eval_specs`, `entities`, `chunks`, `mentions`, and `facts`
as M25-managed collections. Generic reads and catalog visibility remain
compatible, but collection creation/drop/indexing, document CRUD, embedding,
batch, graph, Lua, and CGQL mutation paths reject writes to those names before
backend mutation. Underscore-prefixed authority collections remain hidden from
both reads and writes.

### Search (all POST)
| Path | Description |
|---|---|
| `/api/search/vector` | Pre-computed vector search |
| `/api/search/text` | Plain BM25 full-text search over string fields (`GraphBackend::text_search`); no embedder required — backs the document browser's search box |
| `/api/search/query` | Parsed, read-only CGQL; `language` may be omitted or set to `cgql`. Opaque backend-native text is disabled. |
| `/api/search/semantic` | Text → embed → vector search → fetch docs |
| `/api/search/hybrid` | BM25 + vector with RRF fusion; native uses `GraphBackend::text_search`, Arango uses AQL/ArangoSearch, and unsupported backends skip the BM25 leg explicitly |
| `/api/search/graph-augmented` | Semantic seeds + multi-hop graph traversal |

### Graph
| Method | Path | Description |
|---|---|---|
| POST | `/api/graph/relationships` | Create/upsert relationship |
| GET | `/api/graph/relationships` | Get edges by vertex and direction |
| POST | `/api/graph/traverse` | Graph traversal |

Generic edges cannot use a managed collection or point into a managed vertex
collection. This prevents an ordinary relationship write from manufacturing a
`facts` edge or attaching an ungoverned edge to `entities`, while typed
construction retains narrow access through the server's internal managed
backend.

### Construction & Neurons (graph scopes)
| Method | Path | Description |
|---|---|---|
| POST | `/api/construct/governed-ingest` | Resolve the current M25 approved revision through the existing promotion head, then explicitly ground at most 1,000 supplied chunks from its embedded typed candidate; native atomic-batch capability required. The transition lock covers one batch, not a multi-request corpus generation or head-triggered switch. |
| POST | `/api/construct/directed` | Synchronous taxonomy-directed extraction over 1–32 chunks; one main-provider completion, deterministic evidence gates, and atomic replacement of all occurrences for supplied chunks. Empty valid output replaces with zero; malformed output preserves occurrences. Missing space is auto-created rule-less; active deployments fence this legacy writer. No directed job kind. [Limits and examples](examples/construction/README.md). |
| POST | `/api/construct/ingest` | Ground chunks and atomically reconcile text, mentions, and independently keyed fact occurrences (accepted neurons applied; native atomic-batch capability required; write scope). Sanitized-key collisions fail closed; legacy chunks without raw `chunk_id` require a derived-collection rebuild. |
| POST | `/api/construct/evaluate` | Recall + restraint vs an eval spec (read scope despite POST) |
| POST | `/api/construct/answer-eval` | Answer-level recall/restraint through the graph-augmented trace (read scope; needs the completion provider; optional `two_pass`, `evidence_sentences`) |
| POST | `/api/construct/advise` | Gate advisor: per rule, where `require_in_sentence` is safe (suggestion) vs the author's call (REVIEW flag) — deterministic, read scope |
| POST | `/api/construct/propose` | Gap-directed neuron proposals, stored `proposed` (write scope) |
| POST | `/api/construct/review` | Judge pending proposals + apply the per-space review policy (write scope; `limit` slices for cron-able drains, Lane A+ via the attested judge pair) |
| POST | `/api/construct/draft` | Ontology drafter: NEW space type from chunks into `space_type_drafts` — structurally inert (write scope). `per_document` drafts each title-grouped document separately; `async: true` (with `Idempotency-Key`) enqueues a durable `construct.draft` job instead and returns the submission envelope — required for real corpora, since drafting spends two completions per document and runs past the request timeout |
| POST | `/api/construct/draft/{id}/accept` | Attributed human acceptance: draft becomes vocabulary in `space_types` (write scope) |
| POST | `/api/neurons` | Author a neuron (validated; always stored `proposed`) |
| GET | `/api/neurons` | List (space/status filters) |
| GET | `/api/neurons/graduation` | Leave-one-out redundancy candidates |
| POST | `/api/neurons/{key}/accept\|reject\|retire` | Attributed lifecycle transitions |

### Side-views (graph scopes)

| Method | Path | Description |
|---|---|---|
| POST | `/api/sideviews/generate` | Convenience `sideviews.generate` job submission; requires Idempotency-Key, side-view completion, embedding, and atomic batches. Freezes up to 10,000 exact ordinary-source keys, reads text on execution, clamps requested count to 1–50. Regeneration replaces per-parent rows; deletes share cascade/publication fencing. |

Provider/model inheritance and current restrictions are in the
[operator reference](operations.md#side-view-generation-and-configuration).
Side-views are retrieval aids, never governed facts. Hybrid retrieval opts in
with `include_side_views: true`.

### Durable governed jobs (graph scopes)
| Method | Path | Description |
|---|---|---|
| POST | `/api/jobs` | Idempotently submit `construct.ingest`, `construct.evaluate`, `construct.draft`, or `sideviews.generate` work; M21 context v4 selects verified local-CAS evaluation/job v2, M22 context v5 selects prepared-corpus derivation/job v3, and M23 context v6 selects raw-document preparation plus derivation/job v4 |
| GET | `/api/jobs` | List projected job summaries with a tenant/filter-bound cursor and `archived=exclude\|include\|only`; explicit `offset` is deprecated and hot-only |
| GET | `/api/jobs/{id}` | Read immutable input, result/error, and transition history; successful M21-M23 results include a durable `artifact_consumption` receipt, with a derivation receipt for M22/M23 and a nested preparation receipt for M23 |
| POST | `/api/jobs/{id}/cancel` | Owner-or-Admin cooperative cancellation |
| POST | `/api/jobs/{id}/retry` | Owner-or-Admin idempotent resume/restart retry |
| GET | `/api/admin/jobs/status` | Admin queue limits, active/ready/running state, and catalog reconciliation health |
| POST | `/api/admin/jobs/reconcile` | Admin bounded dry-run/apply hot/archive/catalog repair |
| POST | `/api/admin/jobs/archive` | Admin bounded dry-run/apply terminal archival; no purge |
| POST | `/api/tenants/{name}/quotas` | Host-admin quota merge; `max_active_jobs` may lower but not exceed the host cap |

`GET /health/jobs` exposes collection, data, and catalog failures observed by
startup recovery or queue/catalog operations; it does not rescan all job
history per health request. It is an operational root endpoint alongside
`/health` and `/metrics`. Capacity exhaustion rejects new submission
or retry admissions with HTTP 429 and `Retry-After: 1`; replaying an existing
idempotent request does not need another slot. Quota updates and final slot
reservation share the job transition barrier. Suspended jobs remain counted
globally while their tenant queue is fenced, and worker panic cleanup clears
in-process running/scheduler occupancy after attempting a durable failed
transition. Capacity is reusable only when that terminal write succeeds; a
write failure leaves the job and its slot conservatively occupied and degrades
health.

### Signed evaluation and Semantic Repair governance (specialized scopes)
| Method | Path | Description |
|---|---|---|
| GET | `/api/governance/status` | Tenant-local configured-root and actor status; never private material |
| GET | `/api/governance/keys` | Cursor-list root-certified public registrations |
| POST | `/api/governance/keys` | Admin submits an externally root-signed public registration |
| GET | `/api/governance/keys/{id}` | Inspect one public registration |
| POST | `/api/governance/keys/{id}/revoke` | Admin submits a prospective root-signed revocation |
| GET | `/api/governance/revocations` | Cursor-list immutable revocation history |
| GET | `/api/governance/revocations/{id}` | Inspect one immutable revocation |
| GET | `/api/governance/policies` | Cursor-list signed policy revisions |
| POST | `/api/governance/policies` | `policy-author` submits a signed immutable revision |
| GET | `/api/governance/policies/{id}` | Inspect one signed policy revision |
| POST | `/api/governance/policies/{id}/approve` | Distinct `policy-approver` signs the exact revision |
| GET | `/api/governance/approvals/{id}` | Inspect one signed approval |
| GET | `/api/governance/bindings/{approval_id}` | Resolve the exact context-v2 governance binding |
| GET | `/api/governance/artifact-attestations` | Cursor-list at most 50 compact immutable attestation summaries; full signed records remain on the detail route |
| POST | `/api/governance/artifact-attestations` | `artifact-attestor` submits a pre-signed exact-byte manifest claim |
| GET | `/api/governance/artifact-attestations/{id}` | Inspect one immutable public artifact attestation |
| POST | `/api/governance/artifact-bindings/resolve` | Resolve five active attestations to the exact context-v3/v4/v5/v6 binding set |
| POST | `/api/promotions/evidence` | `promoter` registers an immutable four-job v2 through v6 evidence bundle |
| GET | `/api/promotions/evidence` | Cursor-list promotion evidence |
| GET | `/api/promotions/evidence/{id}` | Inspect one immutable evidence bundle |
| POST | `/api/promotions/evidence/{id}/promote` | `promoter` submits a signed promote intent; gates and head CAS still apply |
| POST | `/api/promotions/evidence/{id}/reject` | `promoter` submits a signed reject intent |
| GET | `/api/promotions/decisions` | Cursor-list immutable decisions |
| GET | `/api/promotions/decisions/{id}` | Inspect one immutable decision and stored signed intent |
| GET | `/api/promotions/current/{space_type}/{channel}` | Read the repairable selection projection |
| POST | `/api/promotions/current/{space_type}/{channel}/rollback` | `promoter` submits an exact signed rollback intent |
| GET | `/api/admin/promotions/status` | Admin authority/recovery status |
| POST | `/api/admin/promotions/reconcile` | Admin dry-run/apply derived-head reconciliation |
| POST | `/api/admin/promotions/recover` | Admin full signed-authority validation and recovery |
| GET | `/api/admin/artifact-custody/evidence/{id}` | Live Admin-only deterministic M24 recovery plan for one immutable M21-M23 evidence record; metadata only, no CAS mutation |
| GET | `/api/semantic-repairs/revisions` | Cursor-list compact immutable revision summaries (default/max 8); candidate and signature stay on detail |
| POST | `/api/semantic-repairs/revisions` | `policy-author` submits one pre-signed revision embedding the exact unchanged M22 typed candidate |
| GET | `/api/semantic-repairs/revisions/{id}` | Inspect one immutable revision and embedded candidate |
| POST | `/api/semantic-repairs/revisions/{id}/review` | Distinct `policy-approver` submits one pre-signed final approve or reject review |
| GET | `/api/semantic-repairs/reviews` | Cursor-list compact immutable review summaries (default 25, max 100); signature stays on detail |
| GET | `/api/semantic-repairs/reviews/{id}` | Inspect one immutable review |
| GET | `/api/semantic-repairs/current/{space_type}/{channel}` | Resolve only when the existing promotion head selects the exact independently approved revision digest |

The external root public key is process configuration, never tenant data. The
HTTP service and CLI accept already signed statements only; private-key fields
are rejected and no accepted authority or stored record contains signer private
material. Stable principal ids enforce author/approver/promoter/attestor duty
separation across key rotation. Revocation is prospective.

M20 artifact records bind one canonical manifest and exact usage subject for
each of corpus, graph, oracle, scorer, and verifier. Context v3 requires all
five bindings, evidence copies the candidate and baseline sets, and the signed
promotion intent binds their aggregate authority digest. Signed locations are
audit observations: the server never fetches them, stores the external bytes,
or proves that `construct.evaluate` consumed them. Evaluation still reads the
live tenant graph. Native snapshots include authority records and manifests,
not external bytes; ArangoDB has no application snapshot surface.

M21 context v4 adds a closed loader plan and uses an optional operator-staged,
tenant-incarnation-scoped local CAS. The worker streams and rehashes every blob
named by the five stored manifests, parses verified singleton graph/oracle
JSON, and evaluates that immutable fact set and EvalSpec instead of the live
graph. Scorer/verifier blobs must match the executable-path digest pinned
through `current_exe()` at server startup but are not executed and provide no
mapped-code or independent verifier-verdict proof. The successful job's receipt
is finalized after scoring and binds the exact job execution, five consumed
slots, and canonical result. Its unkeyed hash does not authenticate authorship
by itself; evidence v4 and the signed consumption authority digest carry it
into governed authority. This adds no network fetch,
artifact upload, graph-derivation proof, external receipt signature, deployment,
quorum, or HA surface. Its repository gates and release-binary
persistent-Native and live-ArangoDB verification passed on 2026-07-18.

M22 context v5 pins loader plan v2 and a nested prepared-corpus derivation plan.
The corpus manifest must contain one canonical `corpus.json` bound to the
space, corpus revision, and preprocessing digest. The graph manifest must
contain exactly canonical `candidate.json` and `graph.json` entries. The worker
resolves the candidate's base space and accepted neurons into the effective
configuration, replays the existing Semantic Neurons grounding function over
the prepared chunks, and derives sorted unique evidence-bearing fact rows
without reading the live backend. It reconstructs the full canonical
evaluation-graph envelope and requires exact object, byte, facts-digest, and
content-address equality with the attested `graph.json` before scoring.

Job v3 stores receipt v2 with a nested corpus/candidate/configuration/plan/
fact/graph derivation binding. Evidence and decision v5, head v3, and the
domain-v3 signed promoter intent carry an explicit aggregate derivation-
authority digest. Recovery, reconciliation, and Native stored-plus-incoming
snapshot preflight validate those links; CAS bytes remain external. The
receipt's unkeyed hash is a server record bound by later signed authority, not
an independent attestation.

The derivation input begins at prepared chunks and its output is only the
canonical evaluation-fact projection. It does not replay raw-document
preprocessing or reconstruct persistent chunks, entities, mentions, indexes,
trigger spans, or storage keys. It does not publish or deploy a graph, execute
staged code, switch a consumer, remotely attest a host, or add quorum or HA.
Final release-binary Native and live-ArangoDB verification is recorded in the
M22 decision.

M23 preserves that M22 meaning and adds a fresh context-v6 authority
generation upstream of it. Loader plan v3 requires the corpus attestation to
use `cognigraph.reproducible-prepared-chunk-corpus.v1` with exactly two sorted,
non-executable `application/json` entries: canonical `corpus.json` followed by
canonical `documents.json`. `documents.json` is a closed schema-v1 set bound to
the target space, corpus revision, and preparation-plan digest. Its rows are
sorted by unique non-blank NFC/control-free document id and contain an
NFC/control-free title; ids and titles are each capped at 1,024 UTF-8 bytes.
Every row also carries the exact media type `text/plain; charset=utf-8`, exact
byte length, SHA-256 digest, and canonical unpadded base64url payload. It is a
package of exact UTF-8 text bytes, not an arbitrary PDF, HTML, office-document,
archive, or OCR input surface.

The pinned preparation plan freezes Unicode 17.0.0, strips one leading UTF-8
BOM, rejects invalid UTF-8 and controls other than CR/LF/TAB, maps CRLF and
bare CR to LF, normalizes to NFC, and enforces the per-document normalized-byte
cap at that post-newline/NFC stage before whitespace collapse. It trims
line-edge Unicode whitespace, collapses interior whitespace runs, treats blank
lines as paragraph boundaries, rejects documents with no non-blank paragraph,
joins other lines and packed paragraphs with one ASCII space, and enforces the
aggregate normalized-byte cap after this full collapse. It greedily emits
byte-bounded chunks without overlap, preferring a `.`, `!`, or `?` boundary
only when followed by whitespace or paragraph end, then a whitespace boundary,
then the largest fitting UTF-8 boundary. A chunk id is
`d-<full lowercase sha256 of the NFC document-id UTF-8>-c<eight-digit ordinal>`;
the final rows are sorted. The reconstructed canonical `corpus.json` must equal
the signed object, bytes, length, and SHA-256 address before the unchanged M22
corpus-to-graph derivation can run.

M23 uses job v4, consumption plan and receipt v3, derivation receipt v2,
preparation plan and receipt v1, context/evidence/decision v6, head v4, and
signed `cognigraph.promotion-intent.v4`. The intent explicitly binds the nested
preparation-authority digest. Earlier M22 records remain prepared-corpus
authority and are never reinterpreted as raw-document preparation; a normal
M23 adoption uses a fresh target channel.

Preparation receipts are address-durable rather than byte-self-contained.
They bind the signed corpus manifest, exact `documents.json` and `corpus.json`
content addresses, and the pinned plan. The enclosing derivation receipt and
later four-run evidence establish downstream derivation/preparation authority;
the preparation receipt does not claim that authority by itself. Neither layer
copies those byte streams into jobs or Native snapshots. Re-execution
therefore requires the external tenant-incarnation CAS, which operators must
back up and replicate separately; ArangoDB still has no CogniGraph application
snapshot. M23 does not extract text, run OCR, parse containers, materialize the
prepared corpus into tenant document/chunk/entity/mention collections, rebuild
the complete operational graph, publish or deploy artifacts, execute staged
code, switch a consumer, or add distributed scheduling, replication, quorum,
consensus, or HA. Authenticated release-binary verification passed on
persistent Native in 14.46 seconds and live ArangoDB Enterprise 3.12.9-1 in
109.22 seconds, including restart/recovery, fail-closed prepared-output and CAS
tamper cases, revocation/history fencing, and isolated cleanup.

M24 closes only that external byte-recovery boundary. The Admin plan endpoint
projects one immutable M21-M23 evidence record into a timestamp-free canonical
union of its exact candidate/baseline artifact sets, historically valid M20
attestation identities, and sorted unique blob addresses. The server performs
no CAS read or write for this projection. `cognigraph-artifacts` supplies the
same tenant-scope and digest verifier to online evaluation and the offline CLI.
The CLI creates and rereads a closed content-addressed bundle and restores a
complete absent scope through verified sibling staging and no-replace
publication. A final normal-CAS reread precedes the external restore receipt.

No M18-M23 generation changes: custody observations neither authorize
promotion nor change evaluation quality. The unkeyed receipts prove one
successful read, not ongoing custody, backup provenance, freshness,
availability, independent replication, encryption, RPO/RTO, or HA. Native and
ArangoDB database recovery remain separate and must be composed with the CAS
bundle and externally retained configuration/trust/secrets.

M25 leaves every M18-M24 promotion wire contract unchanged. It stores the
exact M22 schema-v1 construction candidate in one tenant/incarnation/target-
bound immutable statement signed by an active root-certified PolicyAuthor. It
also binds the observed base promotion-head decision id, with explicit JSON
`null` required for a fresh target rather than field omission.
One independent PolicyApprover signs the revision's only final approve or
reject review; author and approver separation uses stable principal identity,
not key id. Approval does not select or materialize anything. Governed
resolution requires the existing promotion head for the same target to select
that exact recomputed candidate digest. Missing, rejected, mismatched,
unselected, revoked-at-admission, or same-principal authority fails closed.

The signed revision and review live in the hidden
`_cognigraph_semantic_repair_revisions` and
`_cognigraph_semantic_repair_reviews` collections. Their recovery and Native
stored-plus-incoming snapshot validation join the existing signed-authority
union. The public server and CLI receive pre-signed JSON only and never receive
a private key. Legacy semantic collections remain readable, but their existing
rows are not retroactively certified. Normal adoption uses a fresh target
channel and new signed authority.

`POST /api/construct/governed-ingest` is the narrow consumption point: it
resolves the existing head and approved revision, uses the embedded candidate
directly, and invokes the normal atomic construction reconciler for the
supplied chunks. Selecting a head does not call this route automatically.
Each call is capped at 1,000 chunks and holds the promotion transition lock
through its one atomic batch; multiple calls do not form a generation-atomic
deployment and can be separated by another promotion.
M25 adds no durable repair job, drift scheduler, signed judge qualification,
new artifact kind, CAS write, retained graph generation, consumer switch,
distributed writer, or HA behavior.

The selected head is a singleton control-plane pointer, not deployment,
consensus, quorum, or HA.

Governance model: the legacy Semantic Neuron authoring workspace remains human
by default. Its measured, injection-gated LLM judge
(`cognigraph_construct::judge`, two-stage since `judge-policy-v2`: taint
screener then quality judge, versioned `POLICY_REV`) can auto-accept only an
eligible `relation_hint` under an existing `review_policies/{space}` document;
aliases, blockers, rank hints, malformed screener output, and malformed quality
verdicts all queue for a human. That policy document and its model list are not
M25 signed judge qualification. Judge output is upstream authoring evidence,
not Semantic Repair authority: only the separately signed exact candidate,
independent review, and matching existing promotion head satisfy M25.
Vocabulary drafts are structurally inert in their own collection until an
attributed typed acceptance (decision_ontology_drafter.md).

Multi-tenancy (decision_multi_tenancy.md) follows the same structural
principle: one backend store per tenant, routed at a single choke
point — the auth middleware wraps each handler future in a task-local
tenant scope. Admission captures the immutable incarnation, concrete backend,
and cache under the tenant lifecycle lock. The `RoutedBackend`/`RoutedCache`
facades (`server/src/tenancy.rs`) retain those handles across awaited provider
work and tenant recreation; Lua transfers the context into its worker and
cleanup supervisor. User administration holds the lifecycle lock through its
shared-control-store operation. Raw CGQL needs no rewriter, and no
cross-tenant read path exists for any role (host-admin manages tenant
records under the dedicated `TenantAdmin` scope, never tenant data).
Auth lives in a separate control store.

### Query (read-write CGQL)
| Method | Path | Description |
|---|---|---|
| POST | `/api/query` | CGQL including mutations (INSERT/UPDATE/REPLACE/REMOVE/UPSERT); requires `COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true` and, with auth, the `documents:write` scope |

Parsed CGQL reads may inspect the eight M25-managed ordinary collections, but
all five mutation forms targeting one are rejected. Opaque backend-native query
text cannot safely prove read-only behavior, so the guarded raw-query boundary
rejects any reference to a managed collection, including collection bind
values. Direct operator AQL/database access is outside the HTTP/Lua product
trust boundary.

### Sessions (unauthenticated; requires `COGNIGRAPH_AUTH_ENABLED` + `COGNIGRAPH_JWT_SECRET`)
| Method | Path | Description |
|---|---|---|
| POST | `/api/auth/login` | Exchange username/password for a short-lived HS256 JWT (stateless — no revocation; use API tokens for long-lived access) |

### Users & Auth (admin scope; active when `COGNIGRAPH_AUTH_ENABLED=true`)
| Method | Path | Description |
|---|---|---|
| POST | `/api/users` | Create a same-tenant user (`admin`, `editor`, `viewer`, `script-runner`, `policy-author`, `policy-approver`, `promoter`, or `artifact-attestor`); tenant Admin cannot create `host-admin` |
| GET | `/api/users` | List same-tenant users; host-level identities remain hidden |
| DELETE | `/api/users/{key}` | Delete a same-tenant user (revokes their tokens) |
| POST | `/api/users/{key}/tokens` | Create API token (plaintext returned once) |
| GET | `/api/users/{key}/tokens` | List tokens (names only, never hashes) |
| DELETE | `/api/users/{key}/tokens/{token_key}` | Revoke token |
| POST | `/api/users/{key}/tokens/{token_key}/rotate` | Rotate token in place and invalidate the old secret |

### Tenants (host-admin `TenantAdmin` scope; operational in multi-tenant mode)
| Method | Path | Description |
|---|---|---|
| GET | `/api/tenants` | List tenant records and store-open state |
| POST | `/api/tenants` | Create a tenant record |
| POST | `/api/tenants/{name}` | Suspend or activate a tenant |
| DELETE | `/api/tenants/{name}` | Delete credentials and tenant record, evict the store, and quarantine its redb/vector files; the default tenant cannot be deleted |

### Batch (documents:write scope)
| Method | Path | Description |
|---|---|---|
| POST | `/api/batch` | Capability-dependent atomic multi-op writes; native is all-or-nothing, while Arango currently returns unsupported |

### Cache
| Method | Path | Description |
|---|---|---|
| GET | `/api/cache/stats` | Cache statistics (hits, misses, breakdown) |
| POST | `/api/cache/clear` | Clear all cache entries |

### Other
| Method | Path | Description |
|---|---|---|
| GET | `/health` | Service health check |
| GET | `/health/database` | Database readiness check via backend-agnostic `ping()`; HTTP 200 when connected, HTTP 503 when disconnected |
| GET | `/metrics` | Prometheus text exposition (requests, status classes, latency, uptime) |
| GET | `/openapi.yaml` | Current OpenAPI specification |
| POST | `/api/lua/execute` | Execute Lua script |
| GET | `/api/admin/export` | Export a hot JSON snapshot |
| GET | `/api/admin/logs` | Recent non-2xx responses (ring buffer): method, path, status, latency, message, tenant; scoped to the caller's tenant |
| POST | `/api/admin/import` | Import an additive JSON snapshot |

---

## Test Coverage

Tests run across all crates. Most do not require external dependencies; ArangoDB integration tests require a running ArangoDB instance.

| Crate | Scope |
|---|---|
| cognigraph-core | Types, serde, error mapping, shared backend conformance suite (`contract::run_all`) |
| cognigraph-query | Parser/validation/planner/executor units plus the file-driven corpus (`tests/corpus/*.cgql`) |
| cognigraph-native | Backend contract, persistence/durability, BM25, CGQL corpus equivalence, mutations |
| cognigraph-auth | Role/scope matrix, hashing, full user/token lifecycle |
| cognigraph-arango | Client units; integration + conformance suite env-gated on `ARANGO_PASSWORD` |
| cognigraph-cache | Weight curve, rank decay, similarity, invalidation |
| cognigraph-lua | Sandbox, instruction limits, RBAC-gated write mode |
| cognigraph-server | Error → HTTP mapping, rate limiter, metrics rendering |

Run `cargo test --all` for the authoritative result; do not freeze a
repository-wide count in this document. Environment-gated ArangoDB and
live-provider tests may skip when credentials are absent.
