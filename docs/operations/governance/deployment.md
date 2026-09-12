# Deployment

### M26 verified Semantic Repair generation and deployment

M26 leaves every M18-M25 promotion, evidence, job, artifact, preparation,
derivation, revision, review, and head generation unchanged. It adds one
separate explicit graph generation and deployment chain:

1. Ensure the current `{space_type, channel}` promotion selects M22 or M23
   evidence and resolves to the exact approved M25 candidate. Building from
   M18-M21 evidence is rejected because those generations do not carry the
   complete reproducible semantic-fact derivation required by M26.
2. As an authenticated Promoter, submit
   `POST /api/semantic-repairs/generations` with an `Idempotency-Key`, the exact
   target, and `expected_promotion_head_decision_id`. The request is not signed,
   but it is actor- and idempotency-bound. The server freezes the current
   authority, then releases the transition lock while it reads CAS bytes and
   derives the projection.
3. The builder requires `COGNIGRAPH_ARTIFACT_SOURCE=local-cas`, verifies the
   selected canonical `corpus.json` by length and SHA-256 address, resolves the
   exact approved candidate, and reruns the deterministic grounder. Its sorted
   `{source, relation, target, evidence_chunk_id}` rows must equal both the
   candidate-original and candidate-replay derivation receipts; the baseline
   pair must also agree. It computes exact added, removed, and unchanged
   semantic facts and binds that impact to the complete entity/chunk/mention/
   fact occurrence projection. A stale head or any CAS, derivation, timeout, or
   capacity failure stores no partial generation and changes no graph rows.
4. Inspect the complete immutable generation, then have an active
   root-certified Promoter sign
   `cognigraph.semantic-repair-deployment-intent.v1` outside CogniGraph. The
   authenticated principal must own that registration and remain distinct from
   the M25 author and approver and every Artifact Attestor in the selected
   authority. The signed statement binds the exact generation, impact,
   promotion head/projection, candidate, M25 revision/review, expected deployed
   head, action, reason, registration/principal, signed time, and hash of the
   raw Idempotency-Key.
5. Submit the public signed envelope to
   `POST /api/semantic-repairs/generations/{generation_id}/deploy`. One Native
   transaction inserts compatible missing shared entities; replaces all
   `chunks`, `mentions`, and `facts` for the target space; and commits the
   immutable deployment decision plus derived head. Existing entities must
   have the same canonical name and type; they are never overwritten or
   deleted. Rows for every other space remain untouched.

Both nullable deployment fields are required. Bootstrap activation uses
explicit JSON `null` for `expected_deployment_head_decision_id`; later
activation binds the current deployed decision. Activation always uses
explicit `null` for `rollback_target_generation_id`. Omission is invalid.

Deployment state is keyed by space, not channel. Exactly one generation is
served through the existing logical collections for a space, because those
rows carry `space_id` but no channel. Retained generations remain immutable
audit/rollback records; ordinary reads cannot route to another generation or
channel.

Rollback cannot choose policy authority. First complete the ordinary signed
promotion rollback so the prior M22/M23 evidence and approved M25 candidate
are current again. Then sign a new M26 `rollback` intent whose expected
deployed decision is the current M26 head and whose
`rollback_target_generation_id` is the retained one-step-prior generation.
The same Native atomic replacement creates another immutable deployment
decision; it does not modify either retained generation.

The CLI mirrors the HTTP surface:

```sh
cognigraph semantic-repair generation build \
  --target-space SPACE_TYPE --channel CHANNEL \
  --expected-promotion-head PROMOTION_DECISION_ID \
  --idempotency-key semantic-generation-1
cognigraph semantic-repair generation list --limit 8
cognigraph semantic-repair generation show GENERATION_ID
cognigraph semantic-repair generation deploy GENERATION_ID \
  @deployment-intent.request.json --idempotency-key semantic-deployment-1
cognigraph semantic-repair deployment current SPACE_TYPE
```

Generation list responses are compact and cursor-paged; use detail for the
complete projection and impact. The fixed plan admits at most 1,000 chunks,
10,000 candidate semantic facts, 10,000 baseline semantic facts, 50,000 total
projection rows, and 16 MiB of canonical generation JSON. One tenant
incarnation retains at most 64 generations, 16 per space, and 256 MiB of
canonical generation records; one space admits at most 10,000 head-changing
deployment decisions. The target being replaced is independently capped at
50,000 chunk/mention/fact rows and 16 MiB. Capacity exhaustion fails closed;
there is no delete, eviction, or garbage collection.

M26 build, deployment, rollback, and recovery of present M26 authority require
all-or-nothing Native batch execution. Capability checks precede CAS reads,
collection creation, authority writes and graph mutation. Snapshot preflight
validates incoming authority before import. The checked public placeholders live in
[`fixtures/m26/`](../../../fixtures/m26). The server and CLI accept no private key
material.

The [M26 decision](../../decisions/decision_m26_verified_semantic_repair_materialization.md)
retains the 2026-07-19 release-binary authority replay, materialization, recovery
and restart evidence. Those measurements describe that revision.

M26 is not an automatic self-healing controller. Promotion does not trigger a
build, and build does not trigger deployment. There is no drift monitor,
scheduler, durable materialization job, checkpoint, background retry,
cross-space transaction, LLM/prompt qualification, physical per-generation
routing, entity reference counting, vector-index generation, generation GC,
distributed writer, quorum, replication, or HA. See the
[M26 decision](../../decisions/decision_m26_verified_semantic_repair_materialization.md).

The root setting and every `public_key` use canonical unpadded base64url.
Verification-key ids have the form
`ed25519:sha256:<64 lowercase hexadecimal characters>`. Signature statements
use NFC-normalized, lexicographically ordered, integer-only canonical JSON and
are domain-separated by tenant, tenant incarnation, schema, and record kind.
The HTTP service and CLI accept only public keys and already signed request
bodies. Generate, store, sign with, back up, rotate, and recover all private
keys outside CogniGraph; never put one in `.env`, an API request, a snapshot,
or a fixture.

Root-certified key revocation is prospective. Admission checks both signed
time and server-observed acceptance time: a revoked key cannot authorize a new
record, while valid pre-revocation history remains verifiable. Rotating a key
does not create a new principal because separation of duties uses stable
`principal_id`, not username or key id.

The checked authoring example is
[`fixtures/m18/promotion-evaluation.json`](../../../fixtures/m18/promotion-evaluation.json).
It is a complete `POST /api/jobs` JSON body for the native backend. Replace its
example candidate/revision/configuration identities with real immutable inputs,
keep `reproducibility.backend` equal to the active backend, and recompute the
oracle `attestation_digest` whenever any attestation field changes. Submission
also requires the referenced space type to exist. For M19, change the context
to schema version 2 and add the exact server-resolved governance binding; the
public-only signed-request and binding templates live in
[`fixtures/m19/`](../../../fixtures/m19). They contain placeholders and are not
valid authority until the operator replaces every value, recomputes natural
ids/digests, and signs the exact statements externally. For M20, use context
schema version 3, retain that governance binding, and add the exact five-slot
set returned by the artifact-binding resolver. Public-only manifest,
attestor-registration, resolver, and context-binding templates live in
[`fixtures/m20/`](../../../fixtures/m20) and have the same placeholder-only status.
For M21, use context schema version 4, retain the exact M19 and M20 bindings,
add the server's one supported consumption-plan schema version 1 from
[`docs/examples/m21-consumption-plan.json`](../../examples/m21-consumption-plan.json),
and set `reproducibility.backend` to `artifact-snapshot`. The five manifests
must use the formats and singleton entrypoints listed above, and every blob must
already exist in the tenant-incarnation CAS scope. Do not submit a receipt: the
server creates `artifact_consumption` only after a successful verified
evaluation.
For M22, use a fresh context-v5 target, plan schema version 2 from
[`docs/examples/m22-derivation-plan.json`](../../examples/m22-derivation-plan.json),
the singleton prepared `corpus.json`, and the exact two-entry candidate/graph
package described above. The public authoring boundary and strict wire-shape
examples live in [`fixtures/m22/`](../../../fixtures/m22). They are explanatory
templates, not signed authority or a complete attestable package.
For M23, use another fresh context-v6 target and plan schema version 3 from
[`docs/examples/m23-preparation-plan.json`](../../examples/m23-preparation-plan.json).
Replace the M22 singleton corpus artifact with the exact canonical two-entry
`corpus.json`/`documents.json` package described above; retain the M22 graph
package and the M21 oracle/scorer/verifier contracts. Stage all exact bytes in
the tenant-incarnation CAS before submission. Do not submit a preparation
receipt or claim that an M22 job has M23 preparation authority.
For M25, keep that M23 promotion generation unchanged and use a fresh target
channel. Embed the exact unchanged M22 typed candidate in the PolicyAuthor-
signed semantic revision, then submit the independent PolicyApprover review.
The public-only request shapes live in [`fixtures/m25/`](../../../fixtures/m25).
Neither record creates a graph or changes the promotion head; the existing M23
evaluation and promoter workflow must select the same candidate digest before
governed resolution or explicit governed ingest can succeed.

Evidence registration names four distinct succeeded job IDs in explicit order:

```json
{
  "candidate_original_job_id": "...",
  "candidate_replay_job_id": "...",
  "baseline_original_job_id": "...",
  "baseline_replay_job_id": "...",
  "expected_head_decision_id": null
}
```

The original/replay pairs must have identical frozen execution/context. For
M18-M20 they must also have identical normalized result digests. M21-M23 instead
require identical verified-material digests plus identical recall, restraint,
missing, and violation projections; their full result digests intentionally
differ because each result embeds a receipt bound to its own job and attempt.
Candidate and baseline must share the target,
EvalSpec/case union, evaluator/scorer semantics, policy, corpus/oracle revision,
graph lineage, and reproducibility environment; only differences explicitly
listed in the closed policy allowlist may vary. All four source jobs must carry
the exact oracle-stage contract above. The server records independent
denominator, recall, restraint, baseline-regression, replay, comparability, and
oracle-separation gates. A genuine gate failure is retained as immutable
evidence and produces a `blocked` decision if promotion is attempted;
malformed authority is rejected. There is no weighted score or force-promote
override relative to the frozen policy. M18 v1 retains its original
operator-authored policy meaning for historical and diagnostic reads. New M19
evidence requires the independently approved signed revision frozen in the v2
context. M20 v3 additionally requires active, exact subject-bound artifact
attestations in all four contexts. M21 v4 requires the same authority plus the
pinned loader plan and a valid durable consumption receipt on every source
job. Each original/replay pair must carry the same verified material; candidate
and baseline share corpus, oracle, scorer, and verifier material while the
existing graph-difference rule remains in force. A valid M20 signature
authenticates an accountable exact-byte claim. An M21 receipt binds the exact
job execution, locally verified material, and canonical score from the verified
graph and oracle. Its unkeyed hash does not independently prove authorship, and
M21 still does not prove external availability, custody, semantic truth,
corpus-to-graph derivation, mapped-code identity, or host/process integrity.
M22 v5 additionally requires the pinned derivation plan and a valid nested
derivation receipt on all four jobs. Replay pairs must reproduce identical
derivation material, candidate and baseline share the prepared corpus and plan,
and evidence carries an explicit derivation-authority digest. This closes
M21's prepared-corpus-to-evaluation-facts gap only; it does not prove upstream
preprocessing, complete persistent graph reconstruction, independent execution
attestation, or host integrity.
M23 v6 additionally requires the pinned preparation plan and valid nested
preparation receipt on every source job. All four receipts must bind one
identical signed raw-document set, prepared corpus, and preparation plan;
evidence carries explicit preparation authority. This proves the bounded
strict-UTF-8-to-prepared-chunks function, not extraction/OCR fidelity, complete
materialization, deployment, or host integrity.

The tenant governance and promotion APIs are:

```text
GET  /api/governance/status
GET  /api/governance/keys[?limit=N&cursor=C]
POST /api/governance/keys
GET  /api/governance/keys/{registration_id}
POST /api/governance/keys/{registration_id}/revoke
GET  /api/governance/revocations[?limit=N&cursor=C]
GET  /api/governance/revocations/{revocation_id}
GET  /api/governance/policies[?limit=N&cursor=C]
POST /api/governance/policies
GET  /api/governance/policies/{policy_revision_id}
POST /api/governance/policies/{policy_revision_id}/approve
GET  /api/governance/approvals/{approval_id}
GET  /api/governance/bindings/{approval_id}
GET  /api/governance/artifact-attestations[?limit=N&cursor=C]
POST /api/governance/artifact-attestations
GET  /api/governance/artifact-attestations/{attestation_id}
POST /api/governance/artifact-bindings/resolve

GET  /api/semantic-repairs/revisions[?limit=N&cursor=C]
POST /api/semantic-repairs/revisions
GET  /api/semantic-repairs/revisions/{revision_id}
POST /api/semantic-repairs/revisions/{revision_id}/review
GET  /api/semantic-repairs/reviews[?limit=N&cursor=C]
GET  /api/semantic-repairs/reviews/{review_id}
GET  /api/semantic-repairs/current/{space_type}/{channel}
GET  /api/semantic-repairs/generations[?limit=N&cursor=C]
POST /api/semantic-repairs/generations
GET  /api/semantic-repairs/generations/{generation_id}
POST /api/semantic-repairs/generations/{generation_id}/deploy
GET  /api/semantic-repairs/deployments/current/{space_type}

POST /api/promotions/evidence
GET  /api/promotions/evidence[?limit=N&cursor=C]
GET  /api/promotions/evidence/{id}
POST /api/promotions/evidence/{id}/promote
POST /api/promotions/evidence/{id}/reject
GET  /api/promotions/decisions[?limit=N&cursor=C]
GET  /api/promotions/decisions/{id}
GET  /api/promotions/current/{space_type}/{channel}
POST /api/promotions/current/{space_type}/{channel}/rollback
GET  /api/admin/promotions/status
POST /api/admin/promotions/reconcile
POST /api/admin/promotions/recover
```

Admin submits root-signed key registration/revocation requests and retains
status, reconciliation, and recovery. `policy-author` creates policy revisions;
`policy-approver` approves them; those same two purpose-bound roles separately
author and review M25 Semantic Repair revisions; `promoter` registers evidence,
submits signed promote/reject/rollback intents, builds M26 generations, and
submits their signed deployment intents; `artifact-attestor` submits
signed external manifest claims. Admin and all four specialized roles may
inspect tenant-local governance, promotion, and Semantic Repair authority, but
no role can perform another principal's mutation.

Key, policy, approval, artifact-attestation, Semantic Repair revision/review/
generation/deployment, evidence, promote, reject, and rollback mutations require an
`Idempotency-Key`. The signed promotion statement
contains the reason, action, evidence/gate/policy/approval digests, optional
artifact-authority, consumption-authority, derivation-authority, and
preparation-authority digests,
promoter registration, signed timestamp, target, expected head, optional
rollback target, and SHA-256 hash of that exact header value. M23 uses domain
`cognigraph.promotion-intent.v4`, M22 uses v3, M21 uses v2, and earlier signed
generations retain `cognigraph.promotion-intent.v1`. The server recomputes these
values; do not
edit a request after signing it. Promote and rollback compare
`expected_head_decision_id` with the currently applied decision ID, preventing
an ABA transition after rollback. Rollback names the exact prior evidence in
the current chain. Replays return the original immutable record; reuse of a
key with another request returns HTTP 409.

The CLI mirrors the API:

```sh
cognigraph governance status
cognigraph governance key list --limit 50
cognigraph governance key show REGISTRATION_ID
cognigraph governance key register @author-key-registration.json \
  --idempotency-key register-author-1
cognigraph governance key revoke REGISTRATION_ID @key-revocation.json \
  --idempotency-key revoke-author-1
cognigraph governance revocation list --limit 50
cognigraph governance revocation show REVOCATION_ID
cognigraph governance policy create @signed-policy.json \
  --idempotency-key policy-1
cognigraph governance policy list --limit 50
cognigraph governance policy show POLICY_REVISION_ID
cognigraph governance policy approve POLICY_REVISION_ID @signed-approval.json \
  --idempotency-key approval-1
cognigraph governance approval show APPROVAL_ID
cognigraph governance binding resolve APPROVAL_ID
cognigraph governance artifact attest @signed-artifact-attestation.json \
  --idempotency-key attest-corpus-1
cognigraph governance artifact list --limit 50
cognigraph governance artifact show ARTIFACT_ATTESTATION_ID
cognigraph governance artifact binding resolve @artifact-bindings.resolve.json

cognigraph semantic-repair revision submit @semantic-revision.request.json \
  --idempotency-key semantic-revision-1
cognigraph semantic-repair revision list --limit 8
cognigraph semantic-repair revision show REVISION_ID
cognigraph semantic-repair review submit REVISION_ID \
  @semantic-review.request.json --idempotency-key semantic-review-1
cognigraph semantic-repair review list --limit 50
cognigraph semantic-repair review show REVIEW_ID
cognigraph semantic-repair current SPACE_TYPE CHANNEL
cognigraph semantic-repair generation build \
  --target-space SPACE_TYPE --channel CHANNEL \
  --expected-promotion-head PROMOTION_DECISION_ID \
  --idempotency-key semantic-generation-1
cognigraph semantic-repair generation list --limit 8
cognigraph semantic-repair generation show GENERATION_ID
cognigraph semantic-repair generation deploy GENERATION_ID \
  @deployment-intent.request.json --idempotency-key semantic-deployment-1
cognigraph semantic-repair deployment current SPACE_TYPE

cognigraph promotion evidence create @evidence.json --idempotency-key evidence-1
cognigraph promotion evidence list --limit 50
cognigraph promotion evidence show EVIDENCE_ID
cognigraph promotion promote EVIDENCE_ID @signed-promote-intent.json \
  --idempotency-key promote-1
cognigraph promotion reject EVIDENCE_ID @signed-reject-intent.json \
  --idempotency-key reject-1
cognigraph promotion current SPACE_TYPE production
cognigraph promotion decisions --limit 50
cognigraph promotion rollback SPACE_TYPE production @signed-rollback-intent.json \
  --idempotency-key rollback-1
cognigraph promotion status
cognigraph promotion reconcile SPACE_TYPE production --dry-run
cognigraph promotion recover
```

The product HTTP API and CLI are pre-signed JSON passthrough surfaces. They do
not generate keys, handle private material, compute natural ids, or sign
requests. The reusable `cognigraph-governance` crate supplies strict canonical
JSON, Ed25519 key-material, signing, and verification primitives for external
operator tools and tests; custody and execution remain outside the server and
CLI.

Existing M18 context-v1 through M23 context-v6 jobs, evidence, decisions, and
heads retain their historical meaning and remain readable and recoverable. M21
does not fabricate consumption receipts for earlier generations, M22 does not
fabricate derivation receipts or rewrite M21 corpus provenance as derivation,
and M23 does not fabricate preparation receipts or reinterpret M22 prepared
chunks as raw-document proof. Every `{space_type, channel}` target may contain
only one authority generation, so version-6 evidence cannot extend a target
containing v1-v5 evidence. Use a fresh target channel for normal M23 migration.
Existing rollback behavior remains exact and chain-local; it is not retroactive
preparation/derivation proof or permission to bridge generations.
M25 does not change those versions or bridge those generations. Existing
semantic-control and derived rows stay readable as legacy data, but a new
governed resolution requires a separately signed and approved revision whose
candidate digest is exactly the one selected by the current head.

`_cognigraph_governance_keys`,
`_cognigraph_governance_key_revocations`,
`_cognigraph_promotion_policy_revisions`,
`_cognigraph_promotion_policy_approvals`,
`_cognigraph_artifact_attestations`,
`_cognigraph_semantic_repair_revisions`,
`_cognigraph_semantic_repair_reviews`,
`_cognigraph_semantic_repair_generations`,
`_cognigraph_semantic_repair_deployment_decisions`,
`_cognigraph_evaluation_evidence`, and
`_cognigraph_promotion_decisions` are protected immutable authority.
`_cognigraph_promotion_heads` and
`_cognigraph_semantic_repair_deployment_heads` are only projections.
Promotion promote/rollback writes its decision first, then its head; startup,
reads, idempotent replay, and Admin reconciliation repair a missing or stale
promotion head from the contiguous decision chain. M26 deployment commits its
target projection, immutable decision, and derived deployment head in one
Native transaction. A malformed or forked authoritative chain latches tenant
promotion health. New evidence, promote/reject, and rollback mutations are then fenced;
an exact idempotent replay remains available so it can repair a committed
decision's head. `POST /api/admin/promotions/recover` fully validates tenant
authority including registrations, prospective revocations, policies,
approvals, artifact attestations, v2/v3/v4/v5/v6 context bindings, artifact,
M21 consumption, M22 derivation, and M23 preparation receipts/authority digests,
M25 semantic revision/review signatures and candidate/head bindings, and signed
intents, plus M26 immutable generations, impact, signed deployment chains,
derived deployment heads, and currently materialized target projections; it
removes malformed or orphan head projections, reconciles every target, and clears the
mutation fence only after the entire pass succeeds.

Native snapshot export/import is an authenticated-Admin restore boundary.
Import validates the existing-plus-incoming union of public key registrations,
prospective revocations, signed policies/approvals/artifact attestations/intents,
immutable Semantic Repair revisions/reviews/generations/deployment decisions,
evidence/decisions, and referenced hot/archive jobs against the
externally pinned root before changing tenant state. Divergent immutable
content, signature/reference/principal errors, and divergent hot/archive copies
fail preflight. The snapshot carries canonical artifact manifests, M21-M23
durable job receipts, signed authority records, and retained M26 generation
projections, never the referenced corpus, graph, oracle, scorer, or verifier
bytes from the local CAS. The root trust anchor itself is never imported. Head
projections are ignored and rebuilt after the additive import. Preflight proves
that the stored-plus-incoming target rows already equal validated immutable M26
authority before any additive snapshot write. Post-import recovery then
rebuilds derived heads and, if later reconciliation detects target drift,
replaces that target projection atomically while promotion and deployment
writes are fenced.

This proves internal provenance consistency, not backup custody or freshness.
CogniGraph does not sign a whole snapshot, publish a transparency checkpoint,
or detect replay of an older but otherwise valid complete backup. Sign,
timestamp, transport, and retain backups and external artifact bytes outside
CogniGraph if that threat is in scope.

`GET /health/promotions` is a generic unauthenticated readiness signal and
returns HTTP 503 when recovery has latched an authoritative repository error.
Detailed tenant-local counts/errors live at `promotion status`. `/metrics`
adds fixed-cardinality evidence, decision, blocked, replay, reconciliation, and
repair counters; tenant, target, candidate, actor, evidence, and error text are
never labels. `GET /api/governance/status` gives authenticated tenant-local
root configuration and actor visibility without exposing secrets. Detailed
promotion status includes bounded artifact-attestation totals and per-kind
counts. M20 artifact signatures make an exact byte-manifest claim attributable;
they do not make it true or prove that a pre-M21 evaluator used those bytes.
For context v4, M21 bypasses the live `GraphBackend` score input, verifies the
tenant-scoped local-CAS bytes, and validates the durable consumption receipt.
That receipt binds the exact job, verified material, and canonical outcome, but
its unkeyed hash does not authenticate itself. It is not independent custody,
derivation, freshness, semantic-truth, mapped-code, or remote-attestation
evidence.

For context v5, M22 also reconstructs the canonical evaluation graph from the
verified prepared corpus and candidate and requires exact byte equality before
scoring. Its nested receipt and signed derivation-authority binding prove
internal governed consistency of that bounded function, not upstream
raw-document preprocessing, full persistent graph state, independent execution
attestation, or host integrity.

For context v6, M23 first reconstructs the canonical prepared corpus from the
verified strict-UTF-8 raw-document package and requires exact byte equality.
Its nested preparation receipt and signed preparation-authority binding extend
the governed address chain upstream without changing M22 history. They do not
prove extraction/OCR fidelity, preserve CAS custody inside the database,
materialize full operational graph state, or independently attest execution.

The process-local transition lock remains a singleton boundary, not HA,
consensus, or a database quorum. The selected `{space_type, channel}` head is
only a control-plane pointer and never triggers work. M26 provides a separate
explicit signed Native graph-projection switch; it does not deploy a binary,
run automatically, route external traffic, restart a service, or switch an
external consumer.

Two scale limits are explicit in the foundation. One target channel may have
at most 10,000 actionable (head-changing promote/rollback) decisions; start a
new channel before that lifecycle limit. Full status/recovery and snapshot
validation currently materialize tenant-wide evidence and decision authority
plus governance authority in memory before checking all chains. Routine target
reconciliation uses a target index, but bounded/streaming tenant-wide authority
recovery is future scale work.

M26 separately caps one tenant incarnation at 64 retained generations, one
space at 16 retained generations and 10,000 deployment decisions, and all
canonical generation records at 256 MiB. These are hard history limits without
automatic retention or eviction; start a new space lifecycle before exhausting
them.
