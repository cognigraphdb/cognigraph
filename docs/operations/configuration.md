# Configuration

The current release still accepts Arango settings. The approved
[Native-only cleanup](../plans/native-only-2026-09-12.md) will remove them before
first live deployment; preserving legacy settings is not required. Current
configuration below describes existing code, not the completed cleanup.

The default build is Community. Multi-tenant storage, governance, construction,
job and artifact settings belong to the Enterprise build; Community rejects
configured multi-tenant storage, governance roots and artifact CAS. Enable the
Cargo feature or select the corresponding Docker image as described in
[build editions](running.md#build-editions).


## Configuration reference

| Variable | Default | Meaning |
|---|---|---|
| `COGNIGRAPH_HOST` / `COGNIGRAPH_PORT` | `0.0.0.0` / `3000` | Listen address |
| `COGNIGRAPH_BACKEND` | `native` | `native` or `arango` (maintenance) |
| `COGNIGRAPH_NATIVE_PATH` | unset | redb file; unset = in-memory (no durability) |
| `COGNIGRAPH_VECTOR_MODE` | `embedded` | `sidecar` = int8 mmap vectors, ~7x less RAM |
| `COGNIGRAPH_STORAGE_MODE` | `resident` | `paged` = keys+LRU in RAM (requires sidecar) |
| `COGNIGRAPH_CACHE_BYTES` | 256 MiB | Paged-mode document cache budget |
| `COGNIGRAPH_AUTH_ENABLED` | `false` | Bearer tokens + RBAC |
| `COGNIGRAPH_ADMIN_PASSWORD` | unset | Bootstraps the `admin` user at startup; requires auth and `COGNIGRAPH_JWT_SECRET` |
| `COGNIGRAPH_HOST_ADMIN_PASSWORD` | unset | Separately bootstraps the control-plane-only `host-admin` user at startup; requires auth and `COGNIGRAPH_JWT_SECRET` |
| `COGNIGRAPH_JWT_SECRET` / `COGNIGRAPH_JWT_TTL_SECS` | unset / 3600 | Enables `POST /api/auth/login` sessions |
| `COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY` | unset | M19-M26 trust anchor: canonical unpadded base64url encoding of exactly 32 Ed25519 root public-key bytes. Signed-governance mutations fail closed when absent or invalid; never configure a private key. |
| `COGNIGRAPH_ARTIFACT_SOURCE` | `disabled` | M21-M23 evaluation and M26 generation source: `disabled` or `local-cas`. Context-v4/v5/v6 evaluation and M26 build are rejected while disabled. No mode dereferences a signed location or fetches over the network. |
| `COGNIGRAPH_ARTIFACT_CAS_ROOT` | unset | Required with `COGNIGRAPH_ARTIFACT_SOURCE=local-cas`: existing absolute, normal non-symlink operator-staged root containing a normal `tenants` directory. Mount read-only; CogniGraph never creates, uploads, replaces, or deletes blobs. |
| `COGNIGRAPH_ARTIFACT_MAX_EVALUATION_BYTES` | 1073741824 (1 GiB) | Positive maximum cumulative declared lengths charged for actual M21-M23 verification and M26 generation-build reads after safe digest+length reuse. This variable is not a peak-memory, CPU, or elapsed-time limit. M21-M23 durable verification separately enforces 10,000 entries, 4,096 unique digest+length pairs, and a fixed 300-second operation deadline. M26 narrows its synchronous build to one at-most-64-MiB canonical prepared corpus plus its generation bounds and is canceled by the configured HTTP request timeout; there is no separate M26 300-second deadline. M22 pins corpus/candidate/graph derivation caps, and M23 additionally pins raw-document and preparation caps. |
| `COGNIGRAPH_ARTIFACT_MAX_CUSTODY_BYTES` | 2147483648 (2 GiB) | Positive maximum distinct bytes projected into one M24 evidence recovery plan. A plan may union at most 8,192 unique candidate/baseline blobs. The offline CLI separately enforces its selected copy/verification bound. |
| `COGNIGRAPH_TOKEN_TTL_SECS` | 0 (never) | Default expiry for new API tokens |
| `COGNIGRAPH_JOB_INGEST_BATCH_SIZE` | 25 | Chunks per durable construction checkpoint; lower values reduce replay after interruption at the cost of more transactions |
| `COGNIGRAPH_JOB_MAX_ACTIVE_TOTAL` | 1000 | Process-wide admission ceiling for nonterminal durable jobs (`queued`, `running`, and `cancel_requested`) |
| `COGNIGRAPH_JOB_MAX_ACTIVE_PER_TENANT` | 100 | Host ceiling for one tenant incarnation; a tenant `max_active_jobs` quota may lower but not raise it |
| `COGNIGRAPH_JOB_RETENTION_SECS` | 2592000 (30 days) | Default terminal-job age used when `POST /api/admin/jobs/archive` omits `before_ms`; it does not start an automatic purge |
| `COGNIGRAPH_JOB_ARCHIVE_BATCH_SIZE` | 100 | Default terminal records processed by one archive request; operator requests are capped at 1000 |
| `COGNIGRAPH_CGQL_MUTATIONS_ENABLED` | `false` | Mounts read-write `POST /api/query` |
| `COGNIGRAPH_CGQL_MAX_SOURCE_ROWS` / `COGNIGRAPH_CGQL_TIME_BUDGET_MS` | 0 (off) | HTTP CGQL budgets; Lua applies the row cap per `graph.query()` and shares the time allowance across the whole script |
| `COGNIGRAPH_LUA_INSTRUCTION_LIMIT` | 0 (use engine default) | Sandbox instruction cap; 0 keeps the 1,000,000-instruction engine default |
| `COGNIGRAPH_QUERY_CACHE_ENABLED` | `false` | Semantic query cache (`COGNIGRAPH_QUERY_CACHE_*` tunables) |
| `COGNIGRAPH_QUERY_CACHE_BACKEND` / `COGNIGRAPH_QUERY_CACHE_PATH` | `memory` / unset | `persistent` + a redb path makes cached embeddings survive restarts (results stay in-memory) |
| `COGNIGRAPH_EMBEDDING_PROVIDER` | `none` | `openai` / `ollama` / `gemini` + provider keys |
| `COGNIGRAPH_COMPLETION_PROVIDER` | inferred | `openai` or `gemini`; unset selects a nonblank OpenAI key first, then Gemini. No key and no explicit provider disables the optional lane. Invalid explicit settings fail startup. |
| `COGNIGRAPH_COMPLETION_MODEL` | `gpt-5.6-luna` / `gemini-3.8-flash` | Main provider's model for directed construction, drafting, proposal, answer evaluation, and fallback review. Luna uses the selected `reasoning_effort: low` baseline wherever it is resolved; other OpenAI model overrides keep their provider defaults. |
| `COGNIGRAPH_SIDEVIEWS_PROVIDER` | inherits main provider | Optional separate `openai` / `gemini` lane; an explicit provider uses its own default model unless `COGNIGRAPH_SIDEVIEWS_MODEL` is set. |
| `COGNIGRAPH_SIDEVIEWS_MODEL` | inherits main model | Overrides the inherited model, or the explicitly selected side-view provider's default. Invalid configuration fails startup. |
| `OPENAI_API_KEY` / `GEMINI_API_KEY` | unset | Each selected completion lane needs its provider's nonblank key. |
| `OPENAI_BASE_URL` / `GEMINI_BASE_URL` | `https://api.openai.com/v1` / `https://generativelanguage.googleapis.com/v1beta` | Completion endpoint base, including separate side-view lanes; absolute HTTP(S), without query/fragment. Both dedicated OpenAI judges share `OPENAI_BASE_URL`; whitespace/trailing slashes are normalized ([CG-36](../issues/CG-36.md)). |
| `COGNIGRAPH_JUDGE_MODEL` | unset | Dedicated OpenAI review-judge model for `/api/construct/review`, independent of the main provider/model; uses `OPENAI_API_KEY` and `OPENAI_BASE_URL`. Unset/blank falls back to completion. A nonempty model requires a nonempty OpenAI key; invalid selected judge configuration fails startup before storage opens |
| `COGNIGRAPH_JUDGE_PARTNER_MODEL` | unset | Dedicated OpenAI agreement-lane (Lane A+) partner with the same key/base validation. Unset/blank = A+ degrades to queue; explicit model without a nonempty key fails startup. Review-policy qualification and exact pair matching remain required |
| `COGNIGRAPH_DATA_DIR` | unset | Multi-tenant mode: per-tenant stores at `<dir>/<tenant>.redb`, control store (auth + tenant records) at `<dir>/_control.redb`. Mutually exclusive with `COGNIGRAPH_NATIVE_PATH` |
| `COGNIGRAPH_RATE_LIMIT_PER_MINUTE` | 0 (off) | Per-client request cap |
| `COGNIGRAPH_REQUEST_TIMEOUT_SECS` | 30 | Request deadline (408 past it) |
| `COGNIGRAPH_LOG_FORMAT` | `text` | `json` for log shippers |
| `COGNIGRAPH_UI_DIST` | unset | Directory with the built console (`ui/dist`). When set, the server serves it with an SPA fallback: real files win, any other non-`/api` path answers `index.html` so the console's path URLs deep-link. Unknown `/api` paths stay JSON 404. Unset = API-only. |

Production baseline: `COGNIGRAPH_AUTH_ENABLED=true`, separate
`COGNIGRAPH_ADMIN_PASSWORD` and `COGNIGRAPH_HOST_ADMIN_PASSWORD` secrets set,
`COGNIGRAPH_JWT_SECRET` set, budgets on
(`COGNIGRAPH_CGQL_MAX_SOURCE_ROWS`, `COGNIGRAPH_CGQL_TIME_BUDGET_MS`, `COGNIGRAPH_LUA_INSTRUCTION_LIMIT`),
`COGNIGRAPH_RATE_LIMIT_PER_MINUTE` on, `COGNIGRAPH_LOG_FORMAT=json`. TLS terminates at your
reverse proxy. An M19-M26 governed deployment also requires one externally
managed `COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY`; graph-only and historical
M18 deployments may leave it unset, but no signed-governance mutation will
then be accepted. M21-M23 and M26 additionally require an operator-staged read-only
local CAS; leave `COGNIGRAPH_ARTIFACT_SOURCE=disabled` when verified
consumption and derivation are not in use.

## Provider and cache details

| Variable | Default | Meaning |
|---|---|---|
| `ARANGO_URL` | `http://localhost:8529` | ArangoDB connection URL |
| `ARANGO_DB` | `cognigraph` | ArangoDB database name |
| `ARANGO_USER` | `root` | ArangoDB username |
| `ARANGO_PASSWORD` | (empty) | ArangoDB password |
| `COGNIGRAPH_VECTOR_SEARCH_MODE` | `native` | ArangoDB vector search: `native` (model filtering requires ≥3.12.6) or `fallback`; filter before candidate selection and expand candidates for parent deduplication |
| `COGNIGRAPH_EMBEDDING_MODEL` | Provider default | Model name override |
| `OLLAMA_BASE_URL` | `http://localhost:11434` | Ollama server URL |
| `COGNIGRAPH_QUERY_CACHE_TTL_SECS` | `900` | Time-to-live for cache entries |
| `COGNIGRAPH_QUERY_CACHE_MAX_ENTRIES` | `1000` | Maximum LRU cache size |
| `COGNIGRAPH_QUERY_CACHE_SIMILARITY_FLOOR` | `0.7` | Minimum similarity for cache influence |
| `COGNIGRAPH_QUERY_CACHE_STRONG_THRESHOLD` | `0.97` | Similarity for direct return (fast path) |
