# Decision: M15 trustworthy foundation

> Current storage scope (2026-09-12): [Native-only storage](decision_native_only.md)
> supersedes this record's runtime-backend choices and adapter-specific paths.
> Both editions now use Native; public HTTP/Lua queries are parsed CGQL.
> Storage-independent contracts below remain applicable. Earlier backend
> behavior, configuration and verification are retained as dated history,
> not current setup instructions. Use the [operator guides](../operations/README.md).

**Status:** Implemented and verified (2026-07-17).

## Context

The post-M14 product audit found four classes of drift between the documented
contract and the executable system:

1. Authenticated callers could reach opaque AQL with ordinary data-write
   authority, and several direct Lua mutation bindings were not independently
   gated. System collections were guarded by name but still had payload,
   returned-edge, and runtime-bound traversal bypasses.
2. Construction keyed one canonical triple as one edge and treated ingestion as
   additive. Revisions could retain stale facts, identical triples from multiple
   chunks overwrote provenance, and readable sanitized keys could silently
   collide.
3. Arango unbounded scans truncated at 100 rows, traversal score semantics
   differed by backend, and weak/strong cache paths disagreed on freshness,
   response shape, and deleted documents.
4. Readiness returned success-shaped HTTP responses on failed pings, native
   inherited an unconditional ping, and multi-tenant cache statistics could
   report fallback zeros rather than the active tenant.

M15 treats these as one foundation milestone because authorization,
provenance, query parity, cache coherence, and operational truth are mutually
dependent trust boundaries.

> **Superseded by M18 (2026-07-18):** the Admin-only opaque-AQL design below is
> retained as dated M15 evidence, not the current contract. A live ArangoDB
> probe showed that Unicode escapes in backtick identifiers bypass textual
> system-collection screening. M18 disables public opaque backend-native query
> text in HTTP and Lua for every role; parsed CGQL and typed operations remain.

## Decision

### D1. Separate parsed query authority from opaque query authority

- Explicit CGQL under `POST /api/search/query` remains parsed and read-only for
  read-scoped callers.
- Opaque backend-native passthrough, including AQL, requires `admin`. It is not
  safely classifiable as read-only from query text.
- Lua typed CRUD, edge, and batch mutations require `documents:write`; opaque
  backend-native `graph.query()` requires `admin`. When authentication is
  disabled, both mutation gates remain closed.
- `GuardedBackend` rejects underscore-prefixed collections and system vertex
  endpoints in direct documents, batches, edge payloads, returned edges, edge
  lists, and traversal results. Native CGQL executes against the facade rather
  than an unguarded inner backend, so runtime bind variables do not bypass it.
- Arango hybrid-search field names are dynamic bind variables (`doc[@field]`),
  never interpolated AQL text.

### D2. Store construction evidence as occurrences and replace a revision atomically

- A fact edge is a deterministic occurrence keyed by space, chunk, canonical
  triple, and trigger span. Mentions are keyed by space, chunk, and entity.
  Identical triples in other chunks or spaces remain independent evidence.
- A supplied native chunk revision replaces its chunk document, mentions, and
  fact occurrences in one batch. Occurrences no longer grounded by the revised
  text are deleted; other chunks are untouched. The replacement sequence is
  serialized within the writer process from snapshot read through atomic
  commit.
- Construction requires a backend that declares atomic-batch support. Native
  declares it; maintenance-mode Arango fails before construction creates
  collections or writes entities.
- Chunk rows retain raw `space_id`, raw `chunk_id`, `content_hash`, and
  `construction_schema: occurrence-v1`. Sanitized identifiers must contain an
  ASCII letter or digit, and stored raw identities must match before replacing
  a readable key. Global entity keys likewise require exact canonical
  name/type agreement; aliases remain first-definition-wins.
- A first occurrence-model re-ingest removes legacy fact edges that still carry
  matching `space_id` and `evidence_chunk_id`. A legacy chunk without raw
  `chunk_id` cannot be distinguished from a key collision, so ingestion fails
  closed. Operators must rebuild the derived `chunks`, `mentions`, and `facts`
  collections from source chunks. Provenance already overwritten by the legacy
  model cannot be recovered automatically.

### D3. Make backend and cache behavior one contract

- `limit: None` means an unbounded scan on both native and Arango; offset-only
  Arango scans use client-side skipping without an implicit 100-row cap.
- Traversal path score is the product of edge confidence (default 1) and
  `path_decay` at each depth on both backends.
- Only strong cache matches may return without fresh retrieval. Above-floor
  weak matches are weighted RRF inputs to a fresh semantic/hybrid/graph run;
  graph augmentation reruns live traversal and does not reuse weak cached
  `graph_facts`. At or below the configured floor, cache weight is zero and the
  response is classified as fresh.
- All strong paths unwrap cached `SearchHit` envelopes to the same public row
  shape as fresh responses. Deleted or unresolvable cache-only ids are filtered
  without consuming the requested limit.
- Result-cache indexing has one lock order, removes expired index entries, and
  probes the next live similarity candidate rather than letting a stale closest
  key shadow it.
- Managed data, relationship, construction, neuron-transition, batch,
  read-write CGQL, write-capable Lua, and opaque backend-query routes perform
  dependency-safe result invalidation. An
  invalidation generation prevents an in-flight pre-write search from
  repopulating stale results after that invalidation. Embedding-cache entries
  remain independent. Direct storage mutations outside the server require an
  explicit result-cache clear because they cannot participate in this barrier.

### D4. Readiness and statistics report real active state

- `/health/database` returns HTTP 200 only when `GraphBackend::ping()` succeeds
  and HTTP 503 when it fails.
- Native ping acquires its state and, for persistent mode, performs a live redb
  read that checks schema metadata and opens the core tables. It no longer
  inherits unconditional success.
- A routed cache statistics snapshot resolves the active tenant's cache. The
  unauthenticated database health route in multi-tenant mode resolves only the
  implicit `default` tenant; it is not an aggregate readiness report for every
  opened tenant.

## Consequences and limits

- Native is the supported construction backend. Arango remains useful for
  conformance and existing deployments, but no partial construction fallback is
  allowed.
- The construction mutex protects one writer process. CogniGraph remains a
  single-persistent-writer product; M15 does not introduce distributed
  coordination or shared-volume replicas.
- Cache coherence is guaranteed for the server mutation-capable routes named
  above. Direct storage mutations deliberately require an operator cache clear.
- Degradation detection, repair proposal, review, and neuron retirement remain
  invoked procedures. Atomic re-ingestion makes stored occurrences truthful for
  supplied chunks; it does not invent replacement pathways.

## Verification

The required workspace gates passed on 2026-07-17:

```text
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
```

The release binary was then exercised over real HTTP against a persistent
native redb store with authentication and the query cache enabled:

- `/health` and `/health/database` returned 200; cache statistics reported the
  enabled active cache.
- A `script-runner` direct Lua create returned 403, the same typed operation as
  an `editor` returned 200, and an editor read of `_users` returned 403.
- Two chunks grounding the same triple produced two fact occurrences. Revising
  one chunk to remove its licensing trigger grounded zero replacement facts and
  left exactly the other chunk's occurrence. The revised chunk text was stored.
- A distinct raw chunk id mapping to an existing sanitized key returned 400 and
  did not overwrite the stored identity.
- After graceful shutdown and restart, the revised chunk, remaining fact
  occurrence, and Lua-created document were still present; database readiness
  remained 200.
- A separate auth-disabled release process allowed pure Lua execution (200) but
  refused a direct Lua mutation (403).
- A release process pointed at an unreachable Arango endpoint stayed live at
  `/health` (200) while `/health/database` returned 503 with `disconnected`.

A live ArangoDB 3.12.9-1 service was then verified through the configured
`ARANGO_*` credentials. An authenticated version request and `RETURN 1`
cursor request succeeded before the test run. With
`COGNIGRAPH_VECTOR_SEARCH_MODE=fallback`, all five Arango integration tests
passed, including the shared backend contract (CRUD and conflicts, unbounded
and offset-only scans, traversal scoring, filtered scans, and vector fallback).

The first auth-enabled release startup exposed a real integration defect:
ArangoDB reserves underscore-prefixed names, but the client created `_users`,
`_tokens`, and `_tenants` without `isSystem: true`. Startup therefore failed
with `illegal name: collection name invalid`. The client now adds the system
flag only for underscore-prefixed collection creates and `isSystem=true` only
for their drops; exact request-shape tests preserve ordinary collection calls.
The corrected server created all three as system collections and bootstrapped
the administrator successfully.

The same run also caught a backend-contract gap behind the documented
first-write behavior: native materialized a missing collection, while Arango
returned 404. Arango document creates, edge creates, and relationship upserts
now retry only a genuine missing-collection response after ensuring the correct
collection type. The shared contract creates all three collection forms from
their first write so this behavior cannot drift by backend again.

The auth-enabled Arango release binary then passed these real-HTTP checks:

- `/health/database` returned 200 with `connected`.
- Admin AQL succeeded, while the same opaque AQL from an editor returned 403.
  A viewer could still run explicit read-only CGQL over Arango data.
- Admin Lua could run backend-native AQL, while editor Lua received 403.
- Public document and explicit CGQL access to `_users` both returned 403,
  including for Admin.
- A first document write materialized its document collection, and a first
  relationship upsert materialized its edge collection. Repeating the same
  relationship updated the existing edge rather than duplicating it.
- A 1,205-row AQL result crossed the first cursor page and returned all rows.
- Arango's unsupported atomic batch route returned its declared capability
  error without inserting the marker document. Construction ingest likewise
  failed before creating `chunks`, `entities`, `mentions`, or `facts`.

The supplied credential was database-scoped and could not create a disposable
database, so the live run used the configured database after a collection
preflight. Fixed integration collections and isolated HTTP fixtures were absent
before the run and removed afterwards; newly created control collections were
also removed with the required system flag. Existing canonical `documents`,
`embeddings`, and `document_relations` collections were preserved.
