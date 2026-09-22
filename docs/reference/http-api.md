# HTTP API reference

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
| `/api/search/hybrid` | Native BM25 + vector with RRF fusion; unavailable BM25 data is reported explicitly |
| `/api/search/graph-augmented` | Semantic seeds + multi-hop graph traversal. `edge_collection` defaults to the application-written `document_relations`; pass `"facts"` to rank construct-built facts, which is the only collection where accepted `relation_rank_hint` neurons can apply. Enterprise responses computed in the request (fresh or cache-assisted) add `warnings` with code `inert_rank_hints` when accepted rank hints exist but `edge_collection` is not exactly `facts`; strong cache hits omit `warnings`. See the [decision record](../decisions/decision_graph_augmented_edge_collection.md). |

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
| POST | `/api/construct/directed` | Synchronous taxonomy-directed extraction over 1–32 chunks; one main-provider completion, deterministic evidence gates, and atomic replacement of all occurrences for supplied chunks. Empty valid output replaces with zero; malformed output preserves occurrences. Missing space is auto-created rule-less; active deployments fence this legacy writer. No directed job kind. [Limits and examples](../examples/construction/README.md).. Gate rejections are returned as `skips` strings and, since v2.7.22, as structured `refusals` rows recorded in the generated `construction_refusals` ledger (see the [refusal ledger decision](../decisions/decision_construction_refusal_ledger.md)) |
| POST | `/api/construct/ingest` | Ground chunks and atomically reconcile text, mentions, and independently keyed fact occurrences (accepted neurons applied; native atomic-batch capability required; write scope). Sanitized-key collisions fail closed; legacy chunks without raw `chunk_id` require a derived-collection rebuild. |
| POST | `/api/construct/evaluate` | Recall + restraint vs an eval spec (read scope despite POST) |
| POST | `/api/construct/answer-eval` | Answer-level recall/restraint through the graph-augmented trace (read scope; needs the completion provider; optional `two_pass`, `evidence_sentences`) |
| POST | `/api/construct/advise` | Gate advisor: per rule, where `require_in_sentence` is safe (suggestion) vs the author's call (REVIEW flag) — deterministic, read scope |
| POST | `/api/construct/propose` | Gap-directed neuron proposals, stored `proposed` (write scope). Skipped gaps are also recorded as `refusals` rows with gate codes in the `construction_refusals` ledger |
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
[operator reference](../operations/jobs.md#side-view-generation-and-configuration).
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
not external bytes.

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
quorum, or HA surface. Its dated verification is retained in the
[M21 decision](../decisions/decision_m21_verified_artifact_consumption.md).

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
Dated release-binary verification is recorded in the
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
back up and replicate separately. M23 does not extract text, run OCR, parse containers, materialize the
prepared corpus into tenant document/chunk/entity/mention collections, rebuild
the complete operational graph, publish or deploy artifacts, execute staged
code, switch a consumer, or add distributed scheduling, replication, quorum,
consensus, or HA. The [M23 decision](../decisions/decision_m23_reproducible_raw_document_prepared_corpus_processing.md)
retains the dated restart/recovery, prepared-output and CAS tamper, revocation
and cleanup measurements.

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
availability, independent replication, encryption, RPO/RTO, or HA. Native
database recovery must be composed with the CAS bundle and externally
retained configuration/trust/secrets.

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
values. Direct operator database-file access is outside the HTTP/Lua product
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
| POST | `/api/batch` | Atomic multi-op writes using Native all-or-nothing batches |

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
