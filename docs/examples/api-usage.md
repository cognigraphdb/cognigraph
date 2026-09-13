# API Usage Examples

All examples use `curl`. The server runs on `http://localhost:3000` by default.
Application routes are namespaced under `/api`; health and metrics remain at
the server root.

---

## Health

```bash
# Service health
curl http://localhost:3000/health

# Database connectivity
curl http://localhost:3000/health/database

# Durable-job repository readiness
curl http://localhost:3000/health/jobs
```

---

## Durable governed jobs

Four kinds are supported: `construct.ingest`, `construct.evaluate`,
`construct.draft`, and `sideviews.generate`. Each has its own input shape;
async draft wrapper fields are not generic job fields. The idempotency key is
required and exclusive within the current tenant incarnation across all kinds.
See the [runnable construction, draft, and side-view examples](construction/README.md)
for exact request files, provider requirements, limits, and replacement behavior.
For authentication-enabled deployments, add a bearer token to every application
request below. The ingest example assumes an existing accepted `pharma` space.

```bash
curl -i -X POST http://localhost:3000/api/jobs \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: pharma-corpus-r17' \
  -d '{
    "kind": "construct.ingest",
    "input": {
      "space_type": "pharma",
      "chunks": [{"id":"label-1","text":"Meridian supplies Compound X."}]
    }
  }'

curl 'http://localhost:3000/api/jobs?status=running&limit=50'
curl http://localhost:3000/api/jobs/JOB_ID

curl -X POST http://localhost:3000/api/jobs/JOB_ID/cancel \
  -H 'Content-Type: application/json' \
  -d '{"reason":"source revision superseded"}'

curl -X POST http://localhost:3000/api/jobs/JOB_ID/retry \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: retry-JOB_ID-2' \
  -d '{"mode":"resume","reason":"dependency restored"}'
```

New submission returns `202` and `Location`; replay of the same key and
canonical `{kind,input}` returns the existing job with `200`. Reusing the key
for another request returns `409`.

### M21 verified artifact evaluation

M21 uses the same `POST /api/jobs` route and does not add an artifact upload or
fetch API. Before submission, an operator must stage every blob from the five
active M20 manifests in the configured tenant-incarnation local CAS. Signed
location URIs are never dereferenced.

The `construct.evaluate` input must carry a complete context schema version 4,
the exact M19 governance and M20 five-slot artifact bindings, the server's
pinned [consumption-plan schema version 1](m21-consumption-plan.json), and
`reproducibility.backend = "artifact-snapshot"`. The server rejects M21 input
while `COGNIGRAPH_ARTIFACT_SOURCE` is disabled. See the
[operations runbook](../operations/governance.md#signed-evaluation-semantic-repair-authority-and-deployment-m18-m26)
and [M21 decision](../decisions/decision_m21_verified_artifact_consumption.md)
for the exact formats, CAS layout, limits, and trust boundary.

```bash
curl -i -X POST http://localhost:3000/api/jobs \
  -H 'Authorization: Bearer ...' \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: candidate-original-m21-1' \
  --data-binary @m21-evaluation-job.json

curl -H 'Authorization: Bearer ...' \
  http://localhost:3000/api/jobs/JOB_ID | jq '.result.artifact_consumption'
```

The second command yields a receipt only for a succeeded M21 job. CogniGraph
finalizes it after hash-reading every blob entry named by the five manifests,
scoring the verified `graph.json` against the verified `oracle.json`, and
rechecking active artifact
authority; clients must not supply it. The receipt binds the exact durable job
execution, EvalSpec, five consumed slots, and canonical result. Its unkeyed hash
does not independently authenticate server authorship; later signed promotion
authority binds it into the governed chain. It is not an external signature or
remote attestation.

### M22 reproducible prepared-corpus derivation

M22 keeps the same job route, local-CAS layout, and five M20 artifact kinds.
Use a fresh context schema version 5 with the exact
[consumption/derivation plan schema version 2](m22-derivation-plan.json).
The corpus attestation must contain only canonical `corpus.json` in format
`cognigraph.prepared-chunk-corpus.v1`; the graph attestation must contain
exactly canonical `candidate.json` and `graph.json` in format
`cognigraph.reproducible-evaluation-graph.v1`.

```bash
curl -i -X POST http://localhost:3000/api/jobs \
  -H 'Authorization: Bearer ...' \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: candidate-original-m22-1' \
  --data-binary @m22-evaluation-job.json

curl -H 'Authorization: Bearer ...' \
  http://localhost:3000/api/jobs/JOB_ID | \
  jq '.result.artifact_consumption.derivation'
```

The nested receipt appears only after the server has verified all exact bytes,
resolved the candidate, derived the evidence-bearing fact rows from the
prepared chunks, required exact canonical equality with `graph.json`, scored
the reproduced facts against the verified oracle, and rechecked authority.
Clients never submit that receipt. Its unkeyed digest is not independent
attestation; evidence v5 and the promoter's signed
`cognigraph.promotion-intent.v3` bind the resulting derivation authority.

M22 does not replay raw-document parsing or chunking, reconstruct the complete
persistent graph, execute staged code, or deploy a selected head. See the
[M22 operations section](../operations/governance/prepared-corpus.md#m22-prepared-corpus-derivation),
[M22 authoring guide](../../fixtures/m22), and
[M22 decision](../decisions/decision_m22_reproducible_corpus_graph_derivation.md).

### M23 reproducible raw-document preparation

M23 is a new authority generation; it does not reinterpret an M22 job. Use a
fresh target with context schema version 6 and the exact
[consumption/preparation plan schema version 3](m23-preparation-plan.json).
Job schema version 4 is selected automatically. The existing graph, oracle,
scorer, and verifier artifact contracts remain unchanged from M22.

The corpus attestation changes to
`cognigraph.reproducible-prepared-chunk-corpus.v1` and must contain exactly two
sorted, non-executable `application/json` entries, `corpus.json` and
`documents.json`. Both are exact canonical JSON byte streams.
`documents.json` schema version 1 contains `space_type`,
`corpus_revision_id`, `preparation_plan_digest`, and rows sorted by unique
non-blank NFC/control-free document `id`. Every row has exactly `id`, `title`,
`media_type: "text/plain; charset=utf-8"`, `byte_length`, `blob_digest`, and
canonical unpadded `content_base64url`; ids and NFC/control-free titles are at
most 1,024 UTF-8 bytes. The decoded bytes must be non-empty, match the declared
length and SHA-256 digest, and be strict UTF-8. The context's
`effective_configuration.preprocessing_digest` must equal the nested
preparation-plan digest.

`corpus.json` keeps the M22 prepared-corpus schema, but its preprocessing digest
is the M23 preparation-plan digest and its chunks must be the exact output of
the pinned preparer. It freezes Unicode 17.0.0 for NFC normalization and
whitespace classification; rejects unsupported controls; strips exactly one
leading BOM when present; normalizes CRLF/bare CR and NFC; and checks the
per-document normalized-byte cap before whitespace collapse. It trims Unicode
whitespace from line edges, collapses interior runs to one ASCII space, joins
adjacent non-blank lines, uses blank lines as paragraph boundaries, rejects a
document with no non-blank paragraph, and checks the aggregate normalized-byte
cap after full collapse. It packs byte-bounded chunks without overlap,
splitting an overlong paragraph after the latest fitting `.`, `!`, or `?` only
when followed by whitespace or paragraph end, then at whitespace, then at the
largest fitting UTF-8 boundary. It assigns deterministic document-hash plus
ordinal chunk ids and fails closed unless the reconstructed object, canonical
bytes, length, and SHA-256 address equal signed `corpus.json`. The 8 MiB
per-document cap applies after newline/NFC normalization and before whitespace
collapse; the 64 MiB aggregate normalized-byte cap applies after full collapse.

```bash
curl -i -X POST http://localhost:3000/api/jobs \
  -H 'Authorization: Bearer ...' \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: candidate-original-m23-1' \
  --data-binary @m23-evaluation-job.json

curl -H 'Authorization: Bearer ...' \
  http://localhost:3000/api/jobs/JOB_ID | \
  jq '.result.artifact_consumption.derivation.preparation'
```

The server creates consumption receipt v3, derivation receipt v2, and nested
preparation receipt v1 only after exact raw-to-prepared reproduction and the
unchanged exact prepared-to-graph reproduction both pass. Evidence and decision
v6, head v4, and the promoter's signed
`cognigraph.promotion-intent.v4` carry explicit preparation authority. Clients
must not submit any receipt or derived authority.

The receipt retains content addresses and signed manifest projections, not a
second copy of raw or prepared bytes. Native snapshots therefore validate the
address chain but cannot replay preparation without the separately preserved
tenant-incarnation CAS. M23
accepts only already extracted UTF-8 plain text. It does not parse PDF, HTML,
office, compressed, or image inputs; run extraction/OCR outside CogniGraph and
attest the resulting exact text bytes. It also does not populate operational
document/chunk/entity collections, rebuild a full graph, deploy a selected
head, or add replication or HA. See the
[M23 operations section](../operations/governance/raw-documents.md#m23-raw-document-preparation),
[M23 decision](../decisions/decision_m23_reproducible_raw_document_prepared_corpus_processing.md),
and [`fixtures/m23/` authoring guide](../../fixtures/m23).

### M24 artifact recovery plans and offline custody

M24 does not add another evaluation or signed-promotion generation. Once a
real M21-M23 evidence record exists, a live tenant Admin can derive its exact
candidate/baseline artifact-byte recovery plan:

```bash
curl -s \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  "http://localhost:3000/api/admin/artifact-custody/evidence/$EVIDENCE_ID" \
  | tee custody-plan.json
```

The response is deterministic and metadata-only. Pin its `plan_digest`,
`tenant`, and `tenant_incarnation` outside the prospective bundle. The endpoint
never fetches a signed location and never writes the CAS.

On the host that can read the staged CAS, the CLI can fetch the same plan,
stream and rehash the exact bytes into an absent closed bundle, and verify it:

```bash
cognigraph artifact custody create "$EVIDENCE_ID" \
  --cas-root /var/lib/cognigraph/artifacts \
  --out /mnt/backup/cognigraph-$EVIDENCE_ID

cognigraph artifact custody verify /mnt/backup/cognigraph-$EVIDENCE_ID \
  --expected-plan-digest "$PLAN_DIGEST" \
  --tenant "$TENANT" --incarnation "$TENANT_INCARNATION"
```

With the server stopped and the target scope absent, restore into an existing
CAS root/`tenants` boundary and retain the receipt outside the CAS:

```bash
cognigraph artifact custody restore /mnt/backup/cognigraph-$EVIDENCE_ID \
  --cas-root /var/lib/cognigraph/restored-artifacts \
  --expected-plan-digest "$PLAN_DIGEST" \
  --tenant "$TENANT" --incarnation "$TENANT_INCARNATION" \
  --receipt restore-$EVIDENCE_ID.json
```

The receipt records one successful reread; it does not prove ongoing custody,
freshness, independent replication, encryption, or availability. Pair the CAS
bundle with a Native snapshot/cold copy, external configuration, the trust
anchor, and secrets. See the
[M24 operations section](../operations/governance/artifact-custody.md#m24-artifact-custody-and-verified-restoration)
and [M24 decision](../decisions/decision_m24_durable_cas_custody_verified_restoration.md).

---

## Documents

### Create

```bash
curl -X POST http://localhost:3000/api/documents \
  -H 'Content-Type: application/json' \
  -d '{
    "collection": "documents",
    "title": "Graph Theory Basics",
    "content": "A graph consists of vertices and edges...",
    "category": "mathematics"
  }'
```

### Get

```bash
curl http://localhost:3000/api/documents/documents/12345
```

### List

```bash
curl "http://localhost:3000/api/documents?collection=documents&limit=10&offset=0"
```

### Update (partial)

```bash
curl -X PATCH http://localhost:3000/api/documents/documents/12345 \
  -H 'Content-Type: application/json' \
  -d '{"category": "computer-science"}'
```

### Replace (full)

```bash
curl -X PUT http://localhost:3000/api/documents/documents/12345 \
  -H 'Content-Type: application/json' \
  -d '{
    "title": "Updated Title",
    "content": "Completely replaced content."
  }'
```

### Delete

```bash
curl -X DELETE http://localhost:3000/api/documents/documents/12345
```

---

## Graph

### Create a relationship

```bash
curl -X POST http://localhost:3000/api/graph/relationships \
  -H 'Content-Type: application/json' \
  -d '{
    "from": "documents/100",
    "to": "documents/200",
    "relation_type": "cites",
    "confidence": 0.9,
    "metadata": {"source": "auto-extracted"}
  }'
```

### Get relationships

```bash
# Outbound edges
curl "http://localhost:3000/api/graph/relationships?vertex_id=documents/100&direction=outbound"

# Inbound edges
curl "http://localhost:3000/api/graph/relationships?vertex_id=documents/100&direction=inbound"

# Both directions
curl "http://localhost:3000/api/graph/relationships?vertex_id=documents/100&direction=any"
```

### Traverse

```bash
curl -X POST http://localhost:3000/api/graph/traverse \
  -H 'Content-Type: application/json' \
  -d '{
    "start_vertex": "documents/100",
    "max_depth": 3,
    "direction": "outbound",
    "min_confidence": 0.5,
    "path_decay": 0.8
  }'
```

---

## Search

### Vector search (pre-computed embedding)

```bash
curl -X POST http://localhost:3000/api/search/vector \
  -H 'Content-Type: application/json' \
  -d '{
    "collection": "embeddings",
    "vector": [0.1, 0.2, ...],
    "threshold": 0.7,
    "limit": 10
  }'
```

### Semantic search (text query, auto-embedded)

Requires `COGNIGRAPH_EMBEDDING_PROVIDER` to be configured.

```bash
curl -X POST http://localhost:3000/api/search/semantic \
  -H 'Content-Type: application/json' \
  -d '{
    "query": "What programming language is fast and safe?",
    "threshold": 0.3,
    "limit": 5
  }'
```

### Hybrid search (BM25 + vector with RRF fusion)

The BM25 leg searches `documents_collection` using the Native text index.
`search_fields` chooses the text fields; the vector leg searches `embeddings_collection`.

```bash
curl -X POST http://localhost:3000/api/search/hybrid \
  -H 'Content-Type: application/json' \
  -d '{
    "query": "graph database traversal",
    "documents_collection": "documents",
    "search_fields": ["content", "title"],
    "threshold": 0.3,
    "limit": 10,
    "rrf_k": 60,
    "bm25_weight": 0.4,
    "vector_weight": 0.6
  }'
```

### Graph-augmented search (semantic + graph expansion)

```bash
curl -X POST http://localhost:3000/api/search/graph-augmented \
  -H 'Content-Type: application/json' \
  -d '{
    "query": "systems programming",
    "seed_limit": 3,
    "limit": 10,
    "max_depth": 2,
    "direction": "any",
    "min_confidence": 0.5,
    "path_decay": 0.8
  }'
```

### CGQL query (read-only)

`"language": "cgql"` may be explicit or omitted; both select the parsed
read-only CGQL path. Other language values are forbidden for every role.

```bash
curl -X POST http://localhost:3000/api/search/query \
  -H 'Content-Type: application/json' \
  -d '{
    "language": "cgql",
    "query": "FOR d IN documents FILTER d.category == @cat SORT d.title ASC RETURN { title: d.title }",
    "bind_vars": {"cat": "mathematics"}
  }'
```

### CGQL feature tour

Grouping, aggregates, LET bindings, and 30+ built-ins — see
`docs/cgql-v1.md` for the full language:

```bash
curl -X POST http://localhost:3000/api/search/query \
  -H 'Content-Type: application/json' \
  -d '{
    "language": "cgql",
    "query": "FOR d IN documents LET words = LENGTH(SPLIT(d.content, \" \")) COLLECT cat = d.category AGGREGATE avg_words = AVG(words) WITH COUNT INTO n SORT n DESC RETURN { cat: cat, avg_words: avg_words, n: n }"
  }'
```

### CGQL mutations (read-write)

Requires `COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true`; with auth enabled, a token whose
role grants `documents:write`.

```bash
curl -X POST http://localhost:3000/api/query \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer cg_...' \
  -d '{
    "query": "UPDATE @key WITH { reviewed: true } IN documents RETURN { before: OLD.reviewed, after: NEW.reviewed }",
    "bind_vars": {"key": "doc1"}
  }'
```

### Authentication

With `COGNIGRAPH_AUTH_ENABLED=true` and `COGNIGRAPH_ADMIN_PASSWORD` set,
manage users and tokens (admin scope), then send `Authorization: Bearer
cg_...` on protected data and administration requests:

```bash
# Create a viewer and an API token (plaintext token is returned exactly once)
curl -X POST http://localhost:3000/api/users \
  -H 'Authorization: Bearer cg_ADMIN_TOKEN' \
  -H 'Content-Type: application/json' \
  -d '{"username": "vera", "password": "s3cret", "role": "viewer"}'

curl -X POST http://localhost:3000/api/users/USER_KEY/tokens \
  -H 'Authorization: Bearer cg_ADMIN_TOKEN' \
  -H 'Content-Type: application/json' \
  -d '{"name": "ci"}'
```

---

### UPSERT

```bash
curl -X POST http://localhost:3000/api/query \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer cg_...' \
  -d '{
    "query": "UPSERT { slug: @slug } INSERT { slug: @slug, hits: 1 } UPDATE { seen: true } IN pages RETURN { old: OLD, new: NEW }",
    "bind_vars": {"slug": "home"}
  }'
```

### Atomic batch writes

All operations apply or none do (one storage transaction on the native
backend):

```bash
curl -X POST http://localhost:3000/api/batch \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer cg_...' \
  -d '{
    "ops": [
      {"op": "insert", "collection": "documents", "doc": {"_key": "n1", "title": "New"}},
      {"op": "update", "collection": "documents", "key": "n1", "merge": {"reviewed": true}},
      {"op": "delete", "collection": "documents", "key": "old1"}
    ],
    "invalidate": ["documents"]
  }'
```

### Session login (JWT)

With `COGNIGRAPH_AUTH_ENABLED=true` and `COGNIGRAPH_JWT_SECRET` set, exchange credentials for a
short-lived session token; JWTs and `cg_` API tokens are interchangeable
as bearers:

```bash
curl -X POST http://localhost:3000/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username": "admin", "password": "..."}'
# -> {"token": "eyJ...", "token_type": "Bearer", "expires_in": 3600, "role": "admin"}
```

## Lua Script Execution

```bash
curl -X POST http://localhost:3000/api/lua/execute \
  -H 'Content-Type: application/json' \
  -d '{
    "script": "return graph.find_documents(\"documents\", {limit = 5})"
  }'
```

See [lua-scripting.md](lua-scripting.md) for comprehensive Lua examples.

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `COGNIGRAPH_HOST` | `0.0.0.0` | Server bind host |
| `COGNIGRAPH_PORT` | `3000` | Server bind port |
| `COGNIGRAPH_EMBEDDING_PROVIDER` | `none` | `openai`, `ollama`, `gemini`, or `none` |
| `OPENAI_API_KEY` | — | Required when provider is `openai` |
| `OPENAI_BASE_URL` | — | Override the OpenAI endpoint for completion, side-views, and dedicated primary/partner judges |
| `COGNIGRAPH_EMBEDDING_MODEL` | — | Override embedding model name |
| `COGNIGRAPH_COMPLETION_PROVIDER` | inferred from keys | Explicit `openai` / `gemini`; otherwise OpenAI key takes precedence, then Gemini |
| `COGNIGRAPH_COMPLETION_MODEL` | `gpt-5.6-luna` / `gemini-3.8-flash` | Main completion provider model for construction/drafting and fallback review |
| `GEMINI_API_KEY` / `GEMINI_BASE_URL` | unset / provider default | Gemini completion credentials and endpoint |
| `COGNIGRAPH_SIDEVIEWS_PROVIDER` | inherits main provider | Separate side-view lane; explicit provider uses its own model default |
| `COGNIGRAPH_SIDEVIEWS_MODEL` | inherits main model or separate provider default | Override side-view completion model |
| `OLLAMA_BASE_URL` | — | Override Ollama endpoint |
| `COGNIGRAPH_NATIVE_PATH` | — | redb storage path for the native backend; in-memory when unset |
| `COGNIGRAPH_VECTOR_MODE` | `embedded` | Native vector storage: `embedded` or `sidecar` |
| `COGNIGRAPH_STORAGE_MODE` | `resident` | Native storage mode: `resident` or `paged` |

This table covers variables used by these examples. See
[the operations runbook](../operations/configuration.md#configuration-reference) for the
complete server configuration, including auth, cache, budgets, multi-tenancy,
and provider-specific settings.
