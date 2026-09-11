# Decision: M26 verified Semantic Repair materialization

**Status:** Implemented and verified (2026-07-19).

## Context

M25 makes one exact M22 construction candidate authoritative only after an
immutable PolicyAuthor revision, an independent PolicyApprover review, and
selection by the existing signed promotion head. Its governed-ingest route can
apply caller-supplied chunks, but each invocation is an isolated operation. It
does not prove that a complete prepared corpus was materialized, retain a
rollback generation, show the exact candidate-versus-baseline impact, or make
selection and deployed graph state distinct, observable states.

M22 and M23 already provide the missing deterministic evidence for one narrow
closure. Their candidate evaluation receipts retain the exact sorted semantic
fact rows derived from a verified canonical prepared corpus and candidate. M26
reuses that authority to construct one complete, bounded occurrence projection,
records its impact, and makes deployment a separate signed act. It does not
change evaluation or promotion authority and it does not make an LLM verdict
authoritative.

## Decision

### D1. Preserve M18-M25 authority and add a separate M26 generation contract

M26 adds no promotion context, job, artifact-consumption plan or receipt,
preparation receipt, corpus-to-graph derivation receipt, evidence, promotion
decision, promotion head, Semantic Repair revision, or Semantic Repair review
generation. All M18-M25 records retain their exact historical meaning.

The M26 build accepts only a current M22 or M23 promotion selection whose
complete authority validates, whose exact candidate resolves to an approved
M25 revision, and whose candidate original and replay carry equal valid
corpus-to-graph derivation material. M18-M21 selections remain readable but do
not contain enough reproducible graph material to build an M26 generation.

M26 adds these protected tenant-local collections:

```text
_cognigraph_semantic_repair_generations
_cognigraph_semantic_repair_deployment_decisions
_cognigraph_semantic_repair_deployment_heads
```

The first two are immutable authority. The head is a derived projection. All
three remain inaccessible through generic document, graph, batch, CGQL, Lua,
search, embedding, and backend-native query surfaces.

### D2. Build synchronously from the current exact authority

Generation build is an explicit synchronous operation. Its request binds:

```text
target { space_type, channel }
expected_promotion_head_decision_id
```

The server holds the promotion transition lock only while it resolves and
freezes the current promotion head, selected evidence, approved M25 revision
and review, and their exact digests. It releases the lock before reading CAS
bytes or deriving rows. Before storing the completed generation it reacquires
the lock and rejects the build as stale unless the exact same promotion head
and M25 authority still resolve.

The build requires the configured read-only tenant-incarnation CAS. It opens,
length-checks, and SHA-256-checks the prepared `corpus.json` selected by the
candidate receipt, requires canonical JSON, and requires its canonical digest
to equal the exact prepared-corpus digest in the M22/M23 authority. For M23 the
existing preparation receipt and raw-document address remain part of the
validated chain; M26 does not rerun raw-document preparation a second time.

It uses the candidate embedded in the approved M25 revision, requires its
canonical digest to equal the selected M22/M23 candidate address, resolves the
same pinned effective configuration and vetoes, and invokes the existing
deterministic grounder. The candidate and baseline receipt fact arrays are
independently revalidated before impact is computed.

A timeout, cancellation, missing CAS byte, stale head, validation failure, or
capacity error writes no generation and changes no deployed rows. There is no
M26 durable job, checkpoint, partial generation, or background retry. An exact
client retry may reproduce the same natural generation id. The synchronous
HTTP operation is canceled by the configured request timeout; M26 does not
claim the separate fixed 300-second deadline used by M21-M23 durable artifact
consumption.

### D3. Freeze one supported materialization plan and strict limits

The only supported plan is schema version 1:

```text
materializer_id = cognigraph.semantic-repair-materializer
materializer_version = 1
materialization_abi = cognigraph.complete-target-occurrence-projection.v1
max_chunks = 1,000
max_semantic_facts = 10,000
max_total_rows = 50,000
max_canonical_generation_bytes = 16 MiB
```

Its checked semantics digest covers candidate resolution, occurrence key
construction, chunk content hashes, entity identity, mention construction,
fact trigger spans and attribution, canonical ordering, impact set semantics,
and the projection schemas below. The complete plan has a canonical plan
digest. A modified plan or caller-selected limit is not accepted.

The limits are checked with overflow-safe arithmetic before the immutable
record is stored. `max_total_rows` covers the combined entity, chunk, mention,
and fact arrays. Canonical generation bytes cover the complete stored record
except backend-added metadata. Candidate and baseline semantic-fact inputs are
additionally bounded by the existing M22/M23 derivation plan; M26 intentionally
narrows both sides of its impact envelope to 10,000 facts, while the
materialized candidate corpus is also narrowed to 1,000 chunks. This keeps
every public added, removed, and unchanged impact count within the OpenAPI
bound.

One tenant incarnation may retain at most 64 M26 generations, at most 16 for
one `space_type`, and at most 256 MiB of canonical generation records in total.
One space may have at most 10,000 head-changing deployment decisions. Capacity
exhaustion fails closed; M26 has no delete, eviction, or garbage collection.

### D4. Store one immutable complete target occurrence projection

One generation record contains at least:

```text
schema_version = 1
digest_algorithm
tenant, tenant_incarnation
semantic_repair_generation_id, semantic_repair_generation_digest
target { space_type, channel }
source_evidence_id, source_evidence_digest
source_candidate_original_job_id, source_candidate_original_receipt_digest
source_candidate_replay_job_id, source_candidate_replay_receipt_digest
promotion_head_decision_id, promotion_head_projection_digest
candidate_digest
semantic_repair_revision_id, semantic_repair_revision_digest
semantic_repair_review_id, semantic_repair_review_digest
prepared_corpus_digest
derivation_plan_digest, derivation_material_digest
materialization_plan_digest
projection {
  entities, chunks, mentions, facts,
  entity_count, chunk_count, mention_count, fact_count,
  entities_digest, chunks_digest, mentions_digest, facts_digest,
  semantic_facts_digest, projection_digest
}
impact
created_at_ms, created_by
idempotency_key_hash, request_digest
```

The four arrays are sorted by their deterministic `_key` and have unique keys.
Chunks, mentions, and facts all carry the target `space_id`. Mention endpoints
name a target chunk and a required shared entity. Fact endpoints name required
shared entities, and every fact retains the deterministic evidence chunk,
trigger text and span, narrative provenance, neuron id, and reviewer
attribution produced by the existing grounder. The semantic projection of a
fact is exactly `{source, relation, target, evidence_chunk_id}`.

The server projects all candidate facts to that four-field semantic form,
sorts and deduplicates them, and requires exact object equality and canonical
digest equality with both the candidate-original and candidate-replay M22/M23
receipt facts. Score equivalence, count equality, or digest equality without
object equality is insufficient. This is the bridge from evaluated semantic
facts to the complete stored occurrence projection.

`semantic_repair_generation_id` is the lowercase, unprefixed SHA-256 canonical
digest of this stable identity:

```json
{
  "tenant": "...",
  "tenant_incarnation": "...",
  "target": { "space_type": "...", "channel": "..." },
  "source_evidence_id": "...",
  "source_evidence_digest": "sha256:...",
  "candidate_digest": "sha256:...",
  "semantic_repair_revision_digest": "sha256:...",
  "semantic_repair_review_digest": "sha256:...",
  "prepared_corpus_digest": "sha256:...",
  "derivation_material_digest": "sha256:...",
  "materialization_plan_digest": "sha256:...",
  "projection_digest": "sha256:...",
  "impact_digest": "sha256:..."
}
```

The selected promotion decision is recorded and revalidated, but it is not
part of the natural id. A later signed promotion rollback can therefore reuse
the exact retained generation for the same evidence and content instead of
inventing duplicate graph bytes. The complete record digest covers the
observed promotion decision and all other stored fields.

The generation is append-only. There is no update or delete API. Same natural
id with different content, actor, idempotency key, or canonical request is an
immutable conflict. Direct database or snapshot divergence latches M26 health
and fails closed.

### D5. Bind exact candidate-versus-baseline impact

The generation embeds an impact receipt with schema version 1:

```text
candidate_facts_digest, baseline_facts_digest
added_count, removed_count, unchanged_count
added_facts, removed_facts
added_facts_digest, removed_facts_digest
candidate_occurrence_projection_digest
impact_digest
```

Impact is a set comparison over the exact sorted unique four-field semantic
fact rows in the candidate and baseline M22/M23 receipts:

- `added_facts = candidate - baseline`;
- `removed_facts = baseline - candidate`; and
- `unchanged_count = |candidate intersection baseline|`.

The added and removed arrays are canonical and independently digested. Their
counts must add back to the candidate and baseline fact counts without
overflow. The receipt also binds the complete candidate occurrence projection
digest, so a Promoter reviews both semantic change and the exact material to be
deployed. M26 does not claim that every added or removed fact was caused by one
particular neuron; it reports the exact generation difference, not a causal
attribution experiment.

### D6. Make deployment a distinct Promoter-signed act

Building a generation does not deploy it. Activation accepts an externally
signed governance statement under:

```text
cognigraph.semantic-repair-deployment-intent.v1
```

Its closed payload binds at least:

```text
requested_action = activate | rollback
target
semantic_repair_generation_id, semantic_repair_generation_digest
impact_digest
promotion_head_decision_id, promotion_head_projection_digest
candidate_digest
semantic_repair_revision_id, semantic_repair_revision_digest
semantic_repair_review_id, semantic_repair_review_digest
expected_deployment_head_decision_id = null | <64 lowercase hex>
rollback_target_generation_id = null | <64 lowercase hex>
reason
idempotency_key_hash
promoter_registration_id, promoter_principal_id
signed_at_ms
```

Both nullable fields are required in JSON. Bootstrap activation uses explicit
`null` for the expected deployment head. Normal activation uses explicit
`null` for the rollback target. Rollback requires both the exact current
deployment decision and the exact prior generation id. Omission, an unexpected
`null`, or a stale populated value fails closed.

The signer must own an active root-certified M19 key with purpose `promoter`;
the authenticated actor must have the Promoter role and own the registration.
Stable principal identity must remain distinct from the M25 author and
approver and from the artifact attestors already separated by the selected
promotion authority. The product server and CLI never receive a private key.
Prospective revocation blocks a fresh activation or rollback but does not
invalidate a deployment decision admitted while its key was active.

The activation signer need not be the same Promoter principal that signed the
earlier selection. The new signature is the explicit deployment decision; it
cannot bypass the selected promotion head, M25 approval, evaluated impact, or
stale-head checks.

One immutable deployment decision contains at least:

```text
schema_version = 1
digest_algorithm
deployment_decision_id, deployment_decision_digest
tenant, tenant_incarnation, space_type
action = activate | rollback
target { space_type, channel }
semantic_repair_generation_id, semantic_repair_generation_digest
impact_digest
promotion_head_decision_id, promotion_head_projection_digest
candidate_digest
expected_deployment_head_decision_id = null | <64 lowercase hex>
predecessor_deployment_decision_id = null | <64 lowercase hex>
resulting_selection {
  generation,
  semantic_repair_generation_id,
  semantic_repair_generation_digest,
  target,
  candidate_digest,
  prior_semantic_repair_generation_id
}
deployment_intent
actor, reason, created_at_ms
idempotency_key_hash, request_digest
```

`deployment_decision_id` is the unprefixed lowercase SHA-256 of:

```text
tenant NUL tenant_incarnation NUL semantic-repair-deployment NUL
idempotency_key_hash
```

It identifies an act, not graph content. Activating A, then B, then A after a
rollback therefore produces three decisions while both A deployments reference
the same immutable generation. An exact replay requires the same authenticated
actor, idempotency hash, canonical signed request, and decision id.

### D7. Keep one deployed generation per space and switch it atomically

Deployment state is keyed by `{tenant, tenant_incarnation, space_type}`, not by
channel. The head records the generation's full `{space_type, channel}` target.
Only one channel can therefore be deployed for a space at one time. This is
intentional: the existing logical `chunks`, `mentions`, and `facts` collections
carry `space_id` but no channel, so two channel heads cannot truthfully own the
same physical rows.

The derived head contains only schema version 1, digest algorithm, tenant,
incarnation, `space_type`, applied deployment decision id, resulting selection,
update time, and projection digest. Its natural key is the tenant/incarnation-
scoped canonical digest of `space_type`. A head never contains the complete
generation rows and is never accepted as authority without its contiguous
decision chain and immutable generation.

Before activation, the server holds the singleton transition lock and:

1. fully revalidates promotion, M19-M25 governance, M22/M23 derivation, the
   immutable generation, impact, and signed deployment intent;
2. requires the current promotion head decision and candidate to equal the
   generation authority named by the signed intent;
3. compares `expected_deployment_head_decision_id` with the current derived
   deployment head;
4. reads and bounds the complete existing target-space chunks, mentions, and
   facts that will be replaced;
5. validates required shared entity keys, names, and types; and
6. constructs one Native `execute_batch` transaction.

That one transaction:

- inserts any absent required shared entities;
- deletes every existing `chunks`, `mentions`, and `facts` row whose
  `space_id` equals the target space;
- inserts the generation's exact chunks, mentions, and facts;
- inserts the immutable signed deployment decision; and
- creates or replaces the derived deployment head.

All effects commit or none do. Exact replay reads the committed decision and
does not rewrite target data. A response failure after commit is recovered by
the same Idempotency-Key.

The replaced target side is independently capped at 50,000 chunk, mention, and
fact rows and 16 MiB of canonical JSON. A larger or malformed legacy target is
not partially migrated; activation returns a capacity or validation failure
before constructing the write batch. The transaction is therefore bounded by
the checked old projection, the checked new projection, required entity
inserts, one decision, and one head.

Existing rows for every other space remain untouched. Legacy rows in the
target space are deliberately replaced; M26 does not preserve an unsigned
target-space overlay beside a deployed governed generation.

Every server occurrence writer uses the same tenant/incarnation-aware admission
check under the singleton transition lock: ordinary and durable-job ingestion,
governed ingestion, and directed construction. A deployed space is rejected
before directed space creation or any completion-provider call. Directed
construction resolves the tenant incarnation after acquiring the lock and
holds it through provider execution and atomic reconciliation. A deployment
therefore waits for already admitted ingestion; subsequent ingestion sees the
deployment and fails with HTTP 409. Paused tenants and degraded promotion
authority also fail admission. Holding the existing global lock through a
completion can delay other governance transitions until the bounded request
finishes or is cancelled.

Entities are shared across spaces and have no `space_id`. M26 never deletes an
entity and does not claim that `entities` is an exact union for the deployed
generation. A missing required entity may be inserted in the atomic switch; an
existing entity must have the same deterministic key, canonical name, and
entity type or activation fails. Existing aliases are not treated as
generation-owned rollback state. The immutable generation retains the
candidate's aliases for audit; an absent entity is inserted with those aliases,
while a compatible existing entity keeps its existing alias array. Candidate
alias behavior is already frozen in the derived occurrences and semantic
facts.

### D8. Require control-plane rollback before deployed rollback

An M26 deployment rollback cannot choose authority independently. First, the
existing signed promotion rollback must make the prior evidence and candidate
current again. Then a fresh Promoter-signed M26 rollback may name the retained
generation for exactly that newly current authority and the current deployed
head's explicit prior generation.

Rollback creates a new immutable deployment decision and increments the
deployment selection generation; it does not modify or delete either retained
graph generation. Its atomic target replacement is the same operation as
activation. A generation missing from immutable storage, a generation outside
the one-step prior link, or a promotion head that has not rolled back fails
before any graph write.

### D9. Validate recovery and Native snapshot unions

Status, startup validation, explicit recovery, and Native stored-plus-incoming
snapshot preflight validate:

- every generation natural id, record digest, authority reference, limit,
  projection row, endpoint, count, canonical-byte count, and nested digest;
- every candidate-versus-baseline impact set and digest;
- every deployment signature, active-at-admission key use, separation rule,
  idempotency binding, predecessor, prior-generation link, and monotonic
  deployment generation;
- the derived deployment head; and
- the currently materialized target rows against the generation selected by
  the last valid deployment decision.

An imported head is not authority. Same-id/same-content immutable records are
idempotent; divergent records, forked decisions, missing generations, invalid
signatures, or a stored-plus-incoming capacity violation reject the snapshot
before mutation.

The explicit Admin recovery path may reconstruct a missing or stale derived
deployment head and reapply the exact retained generation's target rows in one
Native atomic transaction after all immutable authority validates. It may
insert a missing required entity, but it does not overwrite a conflicting
shared entity or repair a corrupt immutable generation. Such conflicts remain
repair-required and need operator restoration.

### D10. Fail before writes on non-atomic backends

M26 generation build, activation, rollback, and recovery of present M26
authority require a backend that explicitly advertises all-or-nothing
`execute_batch`. The server checks that capability before CAS reads, M26
collection creation, generation insertion, or graph mutation.

The current Native backend is the only supported M26 materialization backend.
General startup and operator status remain compatible with maintenance-mode
ArangoDB: when no M26 authority is present, startup recovery is a no-op and
status reports M26 disabled. An incoming snapshot containing M26 authority is
rejected before import. An M26 mutation, or recovery of present M26 authority,
returns a capability failure before writing any M26 record or graph row. Its
M18-M25 authority remains readable and its operator-native backup boundary is
unchanged. M26 adds no ArangoDB transaction emulation or backend-specific
feature work.

### D11. Keep the foundation deliberately narrow

M26 does not automatically build or activate a generation after promotion. It
adds no drift monitor, scheduler, durable materialization job, checkpoint,
background retry, or cross-space transaction. It does not qualify LLM judges,
prompts, models, or confidence and does not turn LLM output into governance.

M26 does not add physical per-generation collection routing. Retained
generations are immutable records; only the currently deployed target
projection is copied into the existing logical collections. Ordinary reads and
CGQL therefore keep their existing collection contract. They see the deployed
rows for that space and compatible legacy rows for other spaces, but no query
can select a retained generation or a second channel through those generic
collections.

M26 adds no generation delete, garbage collection, retention automation,
entity reference counting, global exact entity-union guarantee, embedding
generation, vector-index generation, full-database generation, signed server
execution attestation, external transparency log, distributed writer, lease,
consensus, quorum, replication, HA, RPO, or RTO claim.

## Acceptance matrix

| Area | Acceptance contract | M26 status |
|---|---|---|
| Source authority | Current M22/M23 promotion plus approved exact M25 revision; every older wire unchanged | Verified |
| Build | Synchronous CAS-verified deterministic projection; no partial durable work | Verified |
| Bounds | 1,000 chunks, 10,000 semantic facts, 50,000 total rows, 16 MiB canonical generation | Verified |
| Equality | Candidate occurrence facts project exactly to original and replay receipt facts | Verified |
| Impact | Exact candidate-minus-baseline and baseline-minus-candidate semantic rows plus full candidate projection digest | Verified |
| Deployment authority | Explicit active Promoter-signed intent bound to generation, impact, selection, and deployment CAS | Verified |
| Atomicity | One Native transaction replaces exact target rows and commits decision/head; Arango writes nothing | Verified |
| Rollback | Signed promotion rollback first, then explicit signed deployment rollback to retained prior generation | Verified |
| Compatibility | Other-space legacy rows and generic query contracts remain; target rows are deliberately replaced | Verified |
| Recovery | Immutable-union validation plus atomic derived-head/active-target reconstruction | Verified |
| Boundaries | No job/scheduler, automatic activation, physical routing, LLM qualification, GC, entity-union, or HA | Explicitly preserved |
| Verification | Exact Rust gates and release-binary persistent-Native plus configured live-ArangoDB boundary probes | Verified |

## Required verification

Completion requires automated coverage for:

- exact M22 and M23 builds, original/replay equality, natural-id replay, and
  strict candidate-versus-baseline added/removed/unchanged impact;
- CAS missing, wrong-length, wrong-digest, non-canonical corpus, candidate
  mismatch, receipt mismatch, stale promotion head, selected-but-unapproved,
  and pre-M22 evidence rejection with no stored generation;
- each chunk, semantic-fact, total-row, per-record byte, per-space, tenant,
  aggregate-byte, and deployment-chain capacity boundary;
- unknown fields and explicit JSON omission/null/populated regressions for both
  required nullable fields in the signed deployment intent;
- wrong key purpose, actor ownership, signature, target, incarnation,
  generation, impact, candidate, revision/review, current promotion,
  same-principal separation, prospective revocation, idempotency conflict, and
  stale deployed-head rejection;
- exact atomic replacement of target chunks/mentions/facts while another
  space's rows remain byte-equivalent; shared entity insert, compatible reuse,
  conflict rejection, and no entity deletion on rollback;
- activation A, activation B, signed promotion rollback B to A, and signed
  deployed rollback B to retained A with monotonic deployment decision history;
- injected batch failure with no partial target rows, decision, or head;
- restart/status/recovery, direct active-row/head tamper, immutable-generation
  tamper, and Native stored-plus-incoming snapshot conflicts;
- ArangoDB capability rejection before CAS reads and with zero M26 and graph
  writes;
- `cargo fmt --all -- --check`;
- `cargo clippy --all-targets -- -D warnings`;
- `cargo test --all`; and
- authenticated release-binary persistent-Native and configured live-ArangoDB
  lifecycles with exact baseline preservation and cleanup.

## Verification evidence

### Directed construction fence — 2026-09-08

The CG-3 regression extends the signed lifecycle below: a real deployment
waits while a directed completion is held pending, then atomically replaces
that ingestion's target rows. A later directed request is rejected before its
provider runs and preserves the complete snapshot. Separate route tests cover
paused/degraded admission and successful undeployed ingestion.

Release-server HTTP checks imported the signed A/B/rollback fixture into
disposable resident and paged Native stores. In each mode, including after a
graceful restart, ingestion targeting a retained deployed chunk returned 409,
made zero provider calls, and preserved the full exported snapshot. An
undeployed space returned 200 and grounded one fact in each mode. See the
[CG-3 remediation report](../issues/directed-fence-2026-09-08.md) for current
validation and scope. The concurrent deployment check runs in the signed Rust
lifecycle; it is not a claim of concurrent HTTP deployment testing.

### Original milestone verification — 2026-07-19

Verification completed on 2026-07-19. The focused
`verified_semantic_repair_materializes_switches_and_recovers_exact_authority`
lifecycle passed in 81.04 seconds. Its exercised contract includes
collision-safe concurrent natural-id replay; strict required-null handling;
isolated wrong-purpose, ownership, signature, author, approver, and artifact-
attestor separation rejection; exact `+1/-1/1 unchanged` candidate impact on
one pinned corpus; removal of stale target chunk, mention, and fact rows; exact
per-collection A/B activation; missing-entity insertion and preservation;
promotion-first rollback; one-step M26 rollback; other-space preservation;
active-row/head/immutable-authority tamper detection; snapshot preflight
conflicts; untrusted-head reconstruction; tenant-incarnation isolation; and
recovery of the exact active projection. The source-validation regression
passed in 16.18 seconds. The exact workspace format,
Clippy-with-warnings-denied, and full test gates also completed; the full server
suite reported 235 passing tests in 87.71 seconds.

An authenticated release-binary persistent-Native probe imported the complete
23-collection authority snapshot, resolved the exact Promoter actor, replayed
both generation build and deployment idempotently, and materialized the exact
active `DISTRIBUTES` and `SUPPLIES` facts while preserving one other-space
fact. The current deployment resolved to decision
`96182db8d048d31ef875d473b3030b8dc1d58b69aa3666255ade82b561c5d2f8`;
generation `51ae723397a0168a2a0a415da4bf7aa4b2f0942a7d5afb46465fdb6c8610cff9`;
the CLI listed both retained generations. Status was healthy with no repair
required, explicit recovery repaired zero heads, and a graceful restart over
the same redb file preserved the exact current head, facts, and status.
Snapshot import rebuilt the derived M26 head rather than trusting the imported
pointer. The tested release binaries had SHA-256 digests
`8931b4e574db021904dfa3ab073bdc019a246bf7dad821b53834ef84e4c700e5`
for `cognigraph-server` and
`f5ae77420c151a726aab10714ef3c3b8acb8ee45f35f63eb272f17b85193e5f2`
for `cognigraph`.

The configured live-ArangoDB probe authenticated against Enterprise 3.12.9-1.
General release-binary startup and operator status succeeded and reported M26
disabled. An authenticated M26 generation build returned HTTP 503 because the
backend does not advertise atomic batches; the rejection completed in 304 ms.
The check confirmed that the three M26 authority collections remained absent,
`entities`, `chunks`, and `mentions` remained absent, all five pre-existing
application collections remained at zero rows, and the database was cleaned
back to its exact 13-collection baseline (five application plus eight ArangoDB
system collections).
