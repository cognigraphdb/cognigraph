# CogniGraph Architecture

## Workspace Structure

```
cognigraph/
├── Cargo.toml                    # Workspace root
├── docs/                         # Documentation
├── ui/                           # React management console (Bun)
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
│   │       ├── planner.rs        # LogicalPlan (serializable); deferral past LIMIT
│   │       ├── functions/        # 30+ built-in functions
│   │       └── executor/         # In-memory + GraphBackend executors, QueryMode, fetch-and-retry
│   ├── cognigraph-native/        # Native backend: memory-primary, redb-durable
│   │   └── src/
│   │       ├── memory/           # GraphBackend impl, CRUD, traversal, search, batches
│   │       ├── storage.rs        # redb write-through store (see native-storage-model.md)
│   │       ├── sidecar.rs        # Rebuildable mmap vector sidecar
│   │       └── text_index.rs     # Rebuildable Tantivy BM25 index
│   ├── cognigraph-auth/          # Auth & RBAC over any GraphBackend
│   ├── cognigraph-arango/        # ArangoDB backend (maintenance mode, contract reference)
│   │   └── src/
│   │       ├── client/           # HTTP client for ArangoDB REST API
│   │       ├── backend/          # GraphBackend trait implementation
│   │       └── database.rs       # Schema initialization
│   ├── cognigraph-cache/         # Semantic query cache with continuous influence
│   │   └── src/
│   │       ├── traits.rs         # QueryCache trait
│   │       ├── memory.rs         # InMemoryCache (LRU + similarity-aware)
│   │       ├── persistent.rs     # redb-persistent embedding cache
│   │       ├── normalize.rs      # Query normalization
│   │       └── similarity.rs     # Cosine similarity
│   ├── cognigraph-embeddings/    # Embedding + completion providers
│   │   └── src/
│   │       ├── lib.rs            # EmbeddingProvider trait
│   │       ├── openai.rs
│   │       ├── ollama.rs
│   │       ├── gemini.rs
│   │       └── completion.rs     # Structured OpenAI/Gemini completion providers
│   ├── cognigraph-construct/     # Semantic Neurons, directed construction, deterministic UTF-8 preparation, WebNLG/DailyMed eval tooling
│   ├── cognigraph-governance/    # Canonical Ed25519 statements/signing/verification primitives
│   ├── cognigraph-artifacts/     # Shared CAS verification and offline custody bundles
│   │   └── src/
│   │       ├── cas.rs            # Read-only tenant-scoped content-address verification
│   │       └── recovery.rs       # Canonical plans, closed bundles, receipts, absent-scope restore
│   ├── cognigraph-cli/           # HTTP administration CLI
│   ├── cognigraph-lua/           # Lua scripting engine
│   │   └── src/
│   │       ├── runtime.rs        # LuaEngine: sandboxed VM, instruction limits
│   │       └── bindings.rs       # Graph primitives exposed to Lua
│   └── cognigraph-server/        # HTTP API (Axum)
│       └── src/
│           ├── main.rs           # Entry point: backend/auth/cache init, layers
│           ├── config.rs         # Environment variable configuration
│           ├── state.rs          # AppState (backend + providers + cache + auth)
│           ├── jobs.rs           # Durable tenant-scoped queue, recovery, checkpoints
│           ├── promotions.rs     # Evaluation evidence, decisions, and derived heads
│           ├── governance.rs     # Signed trust registry, policy, approval, and intent
│           ├── artifact_attestations.rs # Signed exact-byte manifests and context bindings
│           ├── artifact_consumption.rs  # CAS consumption, derivation, and preparation replay
│           ├── artifact_custody.rs      # Read-only evidence recovery-plan derivation
│           ├── tenancy.rs        # Per-tenant backend/cache routing facade
│           ├── error.rs          # CogniGraphError → HTTP status mapping
│           ├── auth_middleware.rs# Bearer token + ScopePolicy checks
│           ├── hardening.rs      # /metrics + per-IP rate limiting
│           └── routes/
│               ├── documents.rs  # Document CRUD, collections, embed pipeline
│               ├── graph.rs      # Relationship upsert, edges, traversal
│               ├── search/       # Vector, text, semantic, hybrid, graph-augmented, CGQL
│               ├── construct.rs  # Ingest/governed-ingest/directed/evaluate/propose/review/draft
│               ├── neurons.rs    # Neuron lifecycle and graduation
│               ├── query.rs      # Read-write CGQL (mutations)
│               ├── batch.rs      # Atomic backend batches
│               ├── jobs.rs       # Submit/list/status/cancel/retry
│               ├── governance.rs # Signed key/policy/artifact authority
│               ├── promotions.rs # Evidence, decision, head, rollback
│               ├── semantic_repairs.rs # Signed revisions, reviews, generations, deployments
│               ├── tenants.rs    # Host-admin tenant lifecycle
│               ├── admin.rs      # Snapshot export/import, logs, recovery
│               ├── users.rs      # User + API token management
│               ├── cache.rs      # Cache stats and clear
│               ├── health.rs     # Health check, database connectivity
│               └── lua.rs        # Lua script execution
```

---

## Core Traits

### GraphBackend

The central storage abstraction. Every database backend implements this trait, making the rest of the system database-agnostic.

```rust
#[async_trait]
pub trait GraphBackend: Send + Sync {
    fn backend_name(&self) -> &str;
    fn query_language(&self) -> QueryLanguage;  // Cgql for native, Aql for Arango
    fn supports_atomic_batches(&self) -> bool;  // explicit capability, false by default
    async fn ping(&self) -> Result<()>;

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

```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f64>>>;
    fn dimension(&self) -> Option<usize> { None }
    fn model_name(&self) -> &str { "default" }
}
```

### QueryCache

Semantic query cache with pluggable backends. Returns the best matching cached results with a similarity score; the caller decides behavior. `CacheKey` carries a params fingerprint of every result-shaping request parameter (threshold, limit, fusion weights, traversal knobs), and the similarity index buckets by (collection, mode, params) — lookups are fuzzy on the query text, never on parameters.

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
Client Request → Rate Limit → Auth/RBAC → Tenant scope → Axum Router → Handler
                                                                          │
                                           ┌──────────────────────────────┼────────────────────────┐
                                           │                              │                        │
                                 GraphBackend trait            EmbeddingProvider trait      QueryCache trait
                                           │                   OpenAI / Ollama / Gemini    memory | persistent
                           NativeBackend (memory + redb) | ArangoDB
                                           │
                              LuaEngine (optional, sandboxed)
```

### Request Lifecycle

1. **Client request** arrives at the Axum server.
2. **Rate limit, auth, and tenant scope** are applied; the auth middleware wraps each handler future in a task-local tenant scope.
3. **Router** dispatches to the handler, which reads shared `AppState`.
4. For search routes: **cache** is checked first. Strong similarity hits return directly; above-floor weaker matches merge with fresh results via RRF with continuous weighting; at or below the floor the fresh result is used.
5. **GraphBackend** dispatches typed operations and server-authored queries to the native or maintenance-mode ArangoDB backend through the `RoutedBackend` facade. Public query text is parsed CGQL; opaque AQL is not exposed over HTTP or Lua.
6. Mutation-capable routes perform **dependency-safe result-cache invalidation** after the attempt (or after commit for typed single operations). Writes made outside the server require an explicit cache clear.
7. **Response** is serialized as JSON.

### Durable governed jobs

Durable work branches after authorization: the handler freezes its operation
input and commits one protected `_cognigraph_jobs` document. A single
process-local dispatcher executes tenant queues round-robin through explicit
`TenantScoped` backends. Native ingestion yields after each atomic chunk-batch
checkpoint; startup requeues interrupted records and may replay the last
idempotent batch. Job state and transition history share one document so
replacement is atomic on both backends.

The hot record and `_cognigraph_job_archive` are authoritative. The
`_cognigraph_job_catalog` holds projected summaries for cursor listing and is
derived and repairable: a write failure degrades `/health/jobs`; bounded
reconciliation rebuilds it. Archival is copy-first into the immutable archive;
there is no purge. Persistent storage provides restart recovery, not
multi-process coordination or HA.

---

## Semantic Query Cache

The cache is a **continuous retrieval signal**, not a key-value store.

1. **Embedding cache**: (query text, model) → embedding vector.
2. **Result cache**: (collection, search_mode, params, query) → results, similarity-aware.
3. **Continuous influence**: `weight = ((similarity - floor) / (1 - floor))³`.
   Similarity ≥ 0.97 returns cached results directly; above-floor lower similarity merges cached and fresh via RRF weighted by the curve; at or below the floor the weight is 0. Per-document rank decay makes top cached results influence more than tail results.
4. **Invalidation**: managed document, relationship, construction, neuron-transition, batch, read-write CGQL, and write-capable Lua routes invalidate conservatively; an invalidation generation prevents an older in-flight search from repopulating stale results.

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
```

---

## Custom ArangoDB Client

Purpose-built rather than relying on the semi-maintained `arangors` crate. Minimal surface area, fully understood, fully controlled.

| Category | Operations |
|---|---|
| **Authentication** | Basic auth, bearer token (JWT) |
| **Documents** | Create, read, update, replace, delete, list |
| **AQL** | Execute via `/_api/cursor`, bind variables |
| **Collections** | Create, drop, ensure (idempotent) |
| **Indexes** | Persistent, hash, fulltext, geo, inverted, vector |
| **Vector search** | `APPROX_NEAR_COSINE` (native) or `COSINE_SIMILARITY()` (fallback) |

---

## Lua Query Engine

- **Engine:** mlua with LuaJIT (JIT disabled in sandbox for reliable instruction counting)
- **Sandboxing:** `os`, `io`, `debug`, `load`, `loadfile`, `dofile`, `require`, `package` removed
- **Resource limits:** per-instance instruction count hook (default 1M)
- **Authorization:** read primitives require `lua:execute`; typed CRUD, edge, and batch mutations additionally require `documents:write`. `graph.query()` is parsed CGQL only and disabled on AQL backends. With auth disabled, Lua graph writes remain disabled. Lua cannot mutate governed semantic collections under any role.

| Function | Description |
|---|---|
| `graph.query(query, bind_vars)` | Parsed CGQL query |
| `graph.get_document(collection, key)` | Fetch single document |
| `graph.find_documents(collection, opts)` | List with limit/offset |
| `graph.create_document(collection, doc)` | Create document |
| `graph.update_document(collection, key, merge)` | Partial update |
| `graph.replace_document(collection, key, doc)` | Replace |
| `graph.delete_document(collection, key)` | Delete |
| `graph.traverse(start_vertex, opts)` | Multi-hop traversal with scoring |
| `graph.neighbors(vertex_id, direction?, collection?)` | Get edges |
| `graph.upsert_edge(from, to, type, data?, collection?)` | Create/update edge |
| `graph.similarity(collection, vector, opts)` | Vector search |
| `graph.text_search(collection, query, fields?, limit?)` | BM25 text search when supported |
| `graph.batch(ops)` | Atomic backend batch |

---

## Governance Model

This section is the short version. The normative contracts are the decision
records `decision_m15_foundation.md` through
`decision_m26_verified_semantic_repair_materialization.md`; the operator's
manual is `docs/dataops/04` and `05`.

### Structural principles

- **No LLM writes a fact.** Grounding is deterministic: trigger-, negation-,
  sentence-, and template-gated. Proposals, judging, drafting, and answer
  evaluation call a completion provider; construction, evaluation, and
  restraint do not.
- **Managed collections are closed to generic writes.** `space_types`,
  `neurons`, `review_policies`, `eval_specs`, `entities`, `chunks`, `mentions`,
  and `facts` remain readable through generic document and parsed-CGQL reads,
  but every generic mutation path (documents, batch, graph, Lua, CGQL) rejects
  them before backend mutation. Typed construction routes retain narrow
  internal write access. Underscore-prefixed authority collections are hidden
  from both reads and writes. Generic edges cannot target a managed collection
  or point into a managed vertex collection.
- **Opaque backend-native query text is disabled for every role.** Text
  screening was shown not to be a security boundary (Unicode-escaped
  identifiers, M18); only parsed CGQL is accepted on public surfaces.
- **Human review is the default.** The legacy neuron workspace's measured,
  injection-gated two-stage judge (taint screener, then quality judge; versioned
  `POLICY_REV`) can auto-accept only an eligible `relation_hint` under an
  existing per-space `review_policies` document. Aliases, blockers, rank hints,
  and malformed output queue for a person. That policy is authoring aid, not
  signed model qualification.
- **Vocabulary drafts are structurally inert** in `space_type_drafts` until an
  attributed typed acceptance (`decision_ontology_drafter.md`).

### The signed chain, one generation per milestone

| Milestone | Boundary closed | Key property |
|---|---|---|
| M16–M17 | Durable, fair, bounded jobs | Idempotent, restart-safe, tenant-scoped; archive never purges |
| M18 | Evaluation → selection | Four-job evidence (candidate/baseline × original/replay), independent integer gates, immutable decisions, repairable `{space_type, channel}` head |
| M19 | Who may author, approve, promote | Externally pinned Ed25519 root; three distinct stable principals; server never holds a private key; prospective revocation |
| M20 | What bytes were claimed | Fourth `artifact-attestor` principal signs canonical exact-byte manifests for corpus, graph, oracle, scorer, verifier; server validates, never fetches |
| M21 | What bytes were consumed | Operator-staged tenant-incarnation local CAS; hash-read of every manifest blob; verified `graph.json`/`oracle.json` replace the live graph as evaluation input; receipt bound into signed intent |
| M22 | Prepared corpus → evaluation facts | Pinned grounder replay; attested `graph.json` must equal the reconstruction byte for byte |
| M23 | Raw UTF-8 text → prepared corpus | Pinned mechanical preparer (Unicode 17.0.0 NFC, fixed whitespace/paragraph/chunk rules, content-addressed chunk ids); reproduced `corpus.json` must match the signed one |
| M24 | CAS byte recovery | Admin-derived canonical recovery plan; offline closed bundle; absent-scope verified restore; unkeyed receipts, no custody claim |
| M25 | Repair candidate authority | PolicyAuthor-signed immutable revision of the exact M22 candidate; independent PolicyApprover review; resolves only when the existing promotion head selects that digest |
| M26 | Selected candidate → served graph | Synchronous Native-only generation build with exact impact receipt; separate Promoter-signed deployment intent; one atomic target-space switch; one generation served per space |

Authority generations never mix within a target: adopting a newer generation
uses a fresh target channel; older records keep their original meaning and
remain readable. Evidence and decisions are immutable; heads are rebuilt from
the decision chain after restart or snapshot import, and degraded authority
fences mutations until Admin recovery validates the tenant.

### What the chain does not claim

Signatures prove who authorized a frozen statement, not that it is true,
current, or complete. Receipts are server-produced unkeyed records bound by
later signed intent, not independent attestations or trusted timestamps.
Scorer/verifier blobs are matched against the executable-path digest pinned at
startup but are never executed. Staged code is never run; promotion never
deploys; building never activates; M26 adds no drift scheduler, automatic
healing, per-generation query routing, generation GC, distributed writer,
quorum, replication, or HA. Maintenance-mode ArangoDB has no application
snapshot surface and fails closed before governed construction because it lacks
atomic batches.

### Multi-tenancy

One backend store per tenant, routed at a single choke point
(`server/src/tenancy.rs`): the auth middleware wraps each handler future in a
task-local tenant scope and the `RoutedBackend`/`RoutedCache` facades resolve
every call through it. Handlers are untouched, CGQL needs no rewriter, and no
cross-tenant read path exists for any role. Host-admin manages tenant records
under the `TenantAdmin` scope, never tenant data. Auth lives in a separate
control store. See `decision_multi_tenancy.md`.

---

## Configuration

All configuration is via environment variables, with `.env` support via `dotenvy`.
The complete reference is `docs/operations.md`; the README carries the common
table. Naming follows `decision_env_naming.md`: CogniGraph-owned knobs are
`COGNIGRAPH_*`; ecosystem-standard third-party names (`OPENAI_API_KEY`,
`ARANGO_URL`, ...) stay bare; legacy names warn loudly at startup rather than
being read.

---

## API Endpoints

All application endpoints are served under `/api` (the UI owns `/`);
`/health`, `/health/database`, `/health/jobs`, `/metrics`, and `/openapi.yaml`
stay at the root. An OpenAPI drift test (`src/openapi_drift.rs`) checks the
source-derived route set against the spec in both directions.

### Documents
| Method | Path | Description |
|---|---|---|
| GET | `/api/collections` | Collection catalog; system (`_`-prefixed) collections hidden |
| POST | `/api/collections` | Idempotently create a document or edge collection |
| DELETE | `/api/collections/{name}` | Drop a non-system collection |
| POST | `/api/documents` | Create document |
| GET | `/api/documents` | List documents |
| POST | `/api/documents/embed` | Provider-batched embedding plus one atomic store transaction |
| GET | `/api/documents/{collection}/{key}` | Get document |
| PATCH | `/api/documents/{collection}/{key}` | Partial update |
| PUT | `/api/documents/{collection}/{key}` | Replace |
| DELETE | `/api/documents/{collection}/{key}` | Delete |

### Search (all POST)
| Path | Description |
|---|---|
| `/api/search/vector` | Pre-computed vector search |
| `/api/search/text` | BM25 full-text over string fields; no embedder required |
| `/api/search/query` | Parsed read-only CGQL |
| `/api/search/semantic` | Text → embed → vector search → fetch docs |
| `/api/search/hybrid` | BM25 + vector with RRF; backends without text search skip the BM25 leg explicitly |
| `/api/search/graph-augmented` | Semantic seeds + multi-hop traversal |

### Graph
| Method | Path | Description |
|---|---|---|
| POST | `/api/graph/relationships` | Create/upsert relationship |
| GET | `/api/graph/relationships` | Edges by vertex and direction |
| POST | `/api/graph/traverse` | Graph traversal |

### Construction & Neurons
| Method | Path | Description |
|---|---|---|
| POST | `/api/construct/ingest` | Ground chunks; atomically reconcile text, mentions, and fact occurrences (native atomic batch required) |
| POST | `/api/construct/governed-ingest` | Resolve the current approved M25 revision through the promotion head, then ground ≤1,000 supplied chunks from its embedded candidate |
| POST | `/api/construct/directed` | Directed construction: taxonomy-constrained per-document nomination with deterministic verbatim-evidence gates; one completion per request, 32-chunk cap |
| POST | `/api/construct/evaluate` | Recall + restraint vs an eval spec |
| POST | `/api/construct/answer-eval` | Answer-level recall/restraint through the graph-augmented trace |
| POST | `/api/construct/advise` | Deterministic gate advisor |
| POST | `/api/construct/propose` | Gap-directed neuron proposals, stored `proposed` |
| POST | `/api/construct/review` | Judge pending proposals under the per-space review policy |
| POST | `/api/construct/draft` | Ontology drafter (`per_document`; `async: true` enqueues a durable `construct.draft` job) |
| POST | `/api/construct/draft/{id}/accept` | Attributed acceptance of a draft into `space_types` |
| POST | `/api/neurons` | Author a neuron (always stored `proposed`) |
| GET | `/api/neurons` | List with space/status filters |
| GET | `/api/neurons/graduation` | Leave-one-out redundancy candidates |
| POST | `/api/neurons/{key}/accept\|reject\|retire` | Attributed lifecycle transitions |

### Durable jobs
| Method | Path | Description |
|---|---|---|
| POST | `/api/jobs` | Idempotently submit `construct.ingest`, `construct.evaluate`, or `construct.draft`; the promotion-context version selects the verified-CAS evaluation generation |
| GET | `/api/jobs` | Cursor-paged summaries; `archived=exclude\|include\|only` |
| GET | `/api/jobs/{id}` | Input, result/error, transition history, consumption/derivation/preparation receipts |
| POST | `/api/jobs/{id}/cancel` | Owner-or-Admin cooperative cancel |
| POST | `/api/jobs/{id}/retry` | Owner-or-Admin idempotent resume/restart |
| GET | `/api/admin/jobs/status` | Queue limits, active state, catalog health |
| POST | `/api/admin/jobs/reconcile` | Bounded dry-run/apply repair |
| POST | `/api/admin/jobs/archive` | Bounded dry-run/apply archival; no purge |
| POST | `/api/tenants/{name}/quotas` | Host-admin quota merge |

Capacity exhaustion returns HTTP 429 with `Retry-After: 1`.

### Signed governance
| Method | Path | Description |
|---|---|---|
| GET | `/api/governance/status` | Configured-root and actor status |
| GET/POST | `/api/governance/keys` | List / Admin submits a root-signed public registration |
| GET | `/api/governance/keys/{id}` | Inspect registration |
| POST | `/api/governance/keys/{id}/revoke` | Prospective root-signed revocation |
| GET | `/api/governance/revocations[/{id}]` | Immutable revocation history |
| GET/POST | `/api/governance/policies` | List / `policy-author` submits a signed revision |
| GET | `/api/governance/policies/{id}` | Inspect revision |
| POST | `/api/governance/policies/{id}/approve` | `policy-approver` signs the exact revision |
| GET | `/api/governance/approvals/{id}` | Inspect approval |
| GET | `/api/governance/bindings/{approval_id}` | Resolve governance binding |
| GET/POST | `/api/governance/artifact-attestations` | List / `artifact-attestor` submits a signed manifest |
| GET | `/api/governance/artifact-attestations/{id}` | Inspect attestation |
| POST | `/api/governance/artifact-bindings/resolve` | Resolve five active attestations to a binding set |

### Promotion
| Method | Path | Description |
|---|---|---|
| GET/POST | `/api/promotions/evidence` | List / `promoter` registers a four-job evidence bundle |
| GET | `/api/promotions/evidence/{id}` | Inspect bundle |
| POST | `/api/promotions/evidence/{id}/promote\|reject` | Signed promoter intent |
| GET | `/api/promotions/decisions[/{id}]` | Immutable decisions |
| GET | `/api/promotions/current/{space_type}/{channel}` | Current selection projection |
| POST | `/api/promotions/current/{space_type}/{channel}/rollback` | Signed rollback intent |
| GET | `/api/admin/promotions/status` | Authority/recovery status |
| POST | `/api/admin/promotions/reconcile` | Dry-run/apply head reconciliation |
| POST | `/api/admin/promotions/recover` | Full signed-authority validation and recovery |
| GET | `/api/admin/artifact-custody/evidence/{id}` | Deterministic M24 recovery plan; metadata only |

### Semantic Repair
| Method | Path | Description |
|---|---|---|
| GET/POST | `/api/semantic-repairs/revisions` | List / `policy-author` submits a signed revision embedding the exact M22 candidate |
| GET | `/api/semantic-repairs/revisions/{id}` | Inspect revision |
| POST | `/api/semantic-repairs/revisions/{id}/review` | `policy-approver` submits the one final approve/reject |
| GET | `/api/semantic-repairs/reviews[/{id}]` | Immutable reviews |
| GET | `/api/semantic-repairs/current/{space_type}/{channel}` | Resolves only when the promotion head selects the approved digest |
| GET/POST | `/api/semantic-repairs/generations` | List / `promoter` builds an immutable generation with impact receipt |
| GET | `/api/semantic-repairs/generations/{id}` | Inspect generation |
| POST | `/api/semantic-repairs/generations/{id}/deploy` | Signed deployment intent; atomic Native switch |
| GET | `/api/semantic-repairs/deployments/current/{space_type}` | Currently served generation |

### Query (read-write CGQL)
| Method | Path | Description |
|---|---|---|
| POST | `/api/query` | CGQL including mutations; requires `COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true` and `documents:write` |

### Sessions, users, tenants
| Method | Path | Description |
|---|---|---|
| POST | `/api/auth/login` | Username/password → short-lived HS256 JWT |
| POST/GET | `/api/users` | Create / list same-tenant users |
| DELETE | `/api/users/{key}` | Delete user (revokes tokens) |
| POST/GET | `/api/users/{key}/tokens` | Create (plaintext once) / list tokens |
| DELETE | `/api/users/{key}/tokens/{token_key}` | Revoke |
| POST | `/api/users/{key}/tokens/{token_key}/rotate` | Rotate in place |
| GET/POST | `/api/tenants` | List / create tenant (host-admin) |
| POST | `/api/tenants/{name}` | Suspend or activate |
| DELETE | `/api/tenants/{name}` | Delete and quarantine store |

### Batch, cache, admin, other
| Method | Path | Description |
|---|---|---|
| POST | `/api/batch` | Atomic multi-op writes (native); Arango returns unsupported |
| GET | `/api/cache/stats` | Cache statistics |
| POST | `/api/cache/clear` | Clear cache |
| POST | `/api/lua/execute` | Execute Lua script |
| GET | `/api/admin/export` | Hot JSON snapshot |
| POST | `/api/admin/import` | Additive JSON snapshot import |
| GET | `/api/admin/logs` | Recent non-2xx responses (ring buffer) |
| GET | `/health`, `/health/database`, `/health/jobs` | Liveness, backend `ping()`, job-subsystem health |
| GET | `/metrics` | Prometheus text exposition |
| GET | `/openapi.yaml` | Current OpenAPI specification |

---

## Test Coverage

| Crate | Scope |
|---|---|
| cognigraph-core | Types, serde, error mapping, shared backend conformance suite |
| cognigraph-query | Parser/validation/planner/executor units plus the file-driven corpus (`tests/corpus/*.cgql`) |
| cognigraph-native | Backend contract, persistence, BM25, CGQL corpus equivalence, mutations, scan-count planner contracts |
| cognigraph-auth | Role/scope matrix, hashing, user/token lifecycle |
| cognigraph-arango | Client units; integration + conformance suite env-gated on `ARANGO_PASSWORD` |
| cognigraph-cache | Weight curve, rank decay, similarity, invalidation |
| cognigraph-lua | Sandbox, instruction limits, RBAC-gated write mode |
| cognigraph-construct | Grounding gates, neurons, directed construction, WebNLG scorer, preparation replay |
| cognigraph-governance / artifacts | Canonicalization, signature verification, CAS tamper cases |
| cognigraph-server | Error mapping, rate limiter, metrics, OpenAPI drift, jobs, promotion/governance lifecycles |

Run `cargo test --all` for the authoritative result; no repository-wide count
is frozen here. Live-provider and ArangoDB tests skip when credentials are
absent. Feature verification additionally requires a release-binary live run
(see `AGENTS.md`).
