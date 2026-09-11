# Decision: M19 signed governance and separation of duties

**Status:** Implemented and verified (2026-07-18).

## Context

M18 made evaluation evidence, promotion decisions, and the selected target head
durable and fail closed. It also left one important trust boundary explicit:
the same tenant `Admin` can author the resolved policy, register evidence, and
promote a candidate that passes that policy. Policy and decision digests make
the resulting history internally consistent, but they do not prove that an
independent principal approved the quality bar or authorized the selection.

M19 closes that governance gap without changing M18's evaluation semantics.
It authenticates a policy author, one independent policy approver, and the
promoter as three distinct principals. It does not turn signatures into proof
that an external corpus, graph revision, or oracle assertion is true, and it
does not turn a promotion head into a deployment.

The foundation deliberately remains small. One host-wide externally pinned
Ed25519 root public key certifies tenant-scoped signing keys. The product
server and CLI have no accepted private-key input: private-key fields are
rejected and no accepted service configuration, request, record, fixture,
environment value, or snapshot uses or persists them. The product HTTP service
and CLI verify or transport supplied signatures; they do not generate keys,
sign statements, escrow keys, or recover them. The reusable
`cognigraph-governance` crate provides strict local
canonicalization, key-material, signing, and verification primitives for
separately operated tools and tests without moving custody into the server.

## Decision

### D1. Preserve M18 evaluation semantics and add governance authority around them

M19 does not change the four-job evidence model, distinct-fact recall and
restraint gates, exact replay, baseline comparability, denominator checks,
oracle-stage checks, stale-head compare-and-set, decision-first persistence,
or derived-head reconciliation.

The new authority boundary is:

```text
root-certified policy-author key
        -> signed immutable policy revision

root-certified policy-approver key, different principal
        -> one signed approval of that exact revision and target

PromotionContext v2
        -> exact immutable binding to both signed records
        -> four deterministic evaluation jobs
        -> immutable evidence

root-certified promoter key, third principal
        -> signed intent for that exact evidence and current head
        -> immutable promotion decision
        -> repairable selected head
```

A cryptographically valid but permissive policy is still the policy that its
author and approver chose. M19 enforces accountable authorization and
separation of duties; it does not invent a universal quality threshold.

### D2. Pin one host-wide Ed25519 root public key outside tenant storage

The server accepts one host-wide Ed25519 root public key from explicit startup
configuration. The configured value is the canonical unpadded base64url
encoding of exactly 32 raw public-key bytes. Its key id is the lowercase
SHA-256 digest of those bytes, algorithm-prefixed as
`ed25519:sha256:<64 lowercase hexadecimal characters>`.

The root key is a trust anchor, not a tenant document. Tenant storage may hold
its expected key id for audit, but changing such a document cannot replace the
externally pinned root. If the root is absent, malformed, or differs from a
stored governance record, signed-governance mutation is unavailable and fails
closed. The server never falls back to unsigned Admin authority.

The product server and CLI accept no private key as configuration or governance
authority. In particular:

- CogniGraph does not accept a root or principal private key in configuration;
- private-key fields in HTTP or CLI request JSON are rejected before authority
  is accepted, used, or persisted;
- private keys are not persisted in native or ArangoDB collections;
- snapshot export/import never contains private key material;
- the HTTP and CLI surfaces accept already signed statements only;
- test fixtures use public keys and precomputed signatures or ephemeral keys
  held by the test process, never production key material.

Root rotation, multiple simultaneous roots, remote KMS signing, HSM support,
and certificate-chain federation are later protocol versions. Replacing the
host root in this foundation is an explicit maintenance migration, not an
Admin API call.

### D3. Use root-certified immutable principal-key registrations

M19 recognizes exactly three signing purposes:

```text
policy_author
policy_approver
promoter
```

Each immutable key registration contains at least:

```text
schema_version
tenant, tenant_incarnation
registration_id, registration_digest
principal_id
subject_user_key
verification_key {
  schema_version
  algorithm = ed25519
  key_id
  purpose
  public_key
}
public_key_digest
not_before_ms
not_after_ms | null
root_key_id
root_signature
possession_signature
```

The root signature covers the canonical registration body with the signature
field removed. The registered subject key signs the same statement as a proof
of possession; a root certificate for a public key that the subject cannot
operate is rejected. Registrations use the existing NFC-normalized,
lexicographically ordered canonical JSON and SHA-256 rules. The signed payload
is domain separated by schema version and object kind; signatures over a bare
digest or an object for another tenant, incarnation, purpose, or record type
are invalid.

`principal_id` is the separation-of-duties identity. It is stable across key
rotation. `subject_user_key` binds the cryptographic principal to the
authenticated tenant user allowed to submit the signed object. Usernames and
key ids do not establish principal independence.

One registered key has exactly one purpose. A second key for the same
`principal_id` remains the same principal and cannot satisfy another duty.
Unknown algorithms, purposes, schema versions, fields, encodings, or canonical
forms fail closed.

Key registrations and revocations are protected immutable tenant authority.
They are inaccessible through generic document, query, Lua, batch, graph, and
search surfaces.

### D4. Make revocation prospective and preserve historical verification

A revocation is a separate immutable root-signed record that names the exact
registration and public-key digest, supplies a bounded reason, and carries an
effective timestamp. It never updates or deletes the registration.

Revocation is prospective:

- an object durably accepted before the revocation became effective remains
  valid historical authority;
- the revoked key cannot authorize any new policy, approval, or promotion
  intent at or after the effective time;
- submitting an old signature after revocation does not make it historical;
  admission uses server-observed operation time as well as signed time;
- a revocation cannot be backdated to reinterpret an already committed
  decision in this foundation.

Rotation registers a new key id. The old and new registrations may overlap,
but both retain the same `principal_id`; rotation therefore cannot bypass
separation of duties. Key ids are never reused for different public keys.

All signed statements carry `signed_at_ms`. The server rejects zero timestamps,
timestamps outside key validity, and timestamps unreasonably in the future.
Timestamps never order promotion decisions: M18's predecessor, generation, and
head compare-and-set remain the ordering authority.

### D5. Store signed immutable policy revisions

A policy author submits an immutable policy-revision record containing:

```text
schema_version
tenant, tenant_incarnation
target { space_type, channel }
policy_revision_id
resolved_policy
resolved_policy_digest
author_registration_id
author_principal_id
signed_at_ms
author_signature
policy_revision_digest
```

The signature statement binds the exact canonical resolved policy, tenant,
tenant incarnation, target, policy-revision identity, author registration,
and signing time. The server still validates the complete closed M18 resolved
policy schema and computes its digest itself. It never accepts a caller's pass
boolean or aggregate policy digest as authoritative.

Policy revisions are append-only. Reusing an immutable identity with identical
canonical bytes is an idempotent match; different bytes are a conflict and
degrade governance health. A new threshold, candidate-difference allowlist,
case policy, oracle policy, or other resolved field requires a new signed
revision.

### D6. Require one independently signed approval

One immutable policy approval contains:

```text
schema_version
tenant, tenant_incarnation
target
approval_id, approval_digest
policy_revision_id, policy_revision_digest
resolved_policy_digest
author_principal_id
approver_registration_id
approver_principal_id
decision = approve
signed_at_ms
approval_signature
```

The approver key must have purpose `policy_approver`; the authenticated caller
must match its `subject_user_key`; and `approver_principal_id` must differ from
the policy's `author_principal_id`. A different key with the same principal id
does not count as an independent approval.

M19 foundation requires exactly one referenced approval. Multi-approver
thresholds, weighted votes, groups, delegation, conditional approvals, and
break-glass self-approval are out of scope. This single independent approval,
followed by a third promoter principal, is the minimum three-principal control.

An approval cannot float to a later policy revision or another target. Any
change to the signed policy bytes requires another approval. Approval
revocation as a separate governance action is future work; policy use can be
stopped prospectively by revoking the author or approver key, while historical
decisions remain verifiable under D4.

### D7. Require an exact `PromotionContext` v2 governance binding

Only `PromotionContext` schema version 2 can produce new M19 promotion
evidence. It retains all M18 context fields and adds a closed governance
binding containing at least:

```text
root_key_id
author_registration_id, author_registration_digest
policy_revision_id, policy_revision_digest
resolved_policy_digest
approval_id, approval_digest
approver_registration_id, approver_registration_digest
author_principal_id
approver_principal_id
```

The context continues to embed the exact resolved policy used by evaluation.
The server loads the referenced protected governance records and requires
byte-equivalent ids, digests, target, tenant incarnation, policy content, key
purposes, signatures, and distinct principals. It never resolves a floating
"current policy" during evidence registration or decision.

The context digest covers the governance binding. Candidate original/replay
and baseline original/replay must therefore freeze the same signed policy and
approval in addition to satisfying all M18 comparison rules. Evidence copies
the governance record identities and digests so recovery can validate the
complete authority union.

Candidate, configuration, case-manifest, reproducibility, corpus, graph,
oracle, scorer, and verifier assertions remain governed by the existing M18
structural and cross-field checks. M19 policy signatures authorize the quality
bar; they do not cryptographically prove those external assertions.

### D8. Require a third principal's signed promotion intent

Promote, reject, and rollback requests carry a signed promotion intent. Its
canonical statement binds at least:

```text
schema_version
tenant, tenant_incarnation
target
requested_action
evidence_id, evidence_digest
policy_revision_id, policy_revision_digest
approval_id, approval_digest
gate_assessment_digest
expected_head_decision_id
rollback_target_evidence_id | null
reason
idempotency_key_hash
promoter_registration_id
promoter_principal_id
signed_at_ms
```

The promoter registration must have purpose `promoter`; its authenticated
`subject_user_key` must be the caller; and its `principal_id` must differ from
both policy author and approver. The server recomputes every referenced digest
and the idempotency-key hash, then verifies the Ed25519 signature immediately
before committing the decision under the existing singleton promotion lock.

The immutable promotion decision stores the complete signed intent, promoter
registration/digest, and the three principal ids. A valid signature does not
bypass failed gates or stale-head CAS. A signed `promote` request for failed
evidence still produces the existing attributed `blocked` action and no head
change. A signed rollback remains limited to the exact explicit prior evidence
recorded by the current chain.

Admin authentication alone cannot create a new M19 decision. Operational
Admin authority remains available for read-only inspection, status,
reconciliation, and recovery, but it cannot author, approve, or promote.

### D9. Keep all governance authority immutable and protected

M19 adds protected tenant-local collections for key registrations,
revocations, policy revisions, and approvals. Their exact names are an
implementation detail recorded with the API/schema work, but they follow the
same system-collection protection and tenant-incarnation identity rules as M18
evidence and decisions.

Signatures do not replace canonical record digests, idempotency, or immutable
insert semantics. Both are required:

- canonical digests bind records and support deterministic reconciliation;
- Ed25519 signatures authenticate the root-certified principal's statement;
- idempotency prevents request retries from creating alternate authority;
- protected storage prevents ordinary API capabilities from editing records.

An invalid signature, wrong purpose, foreign tenant/incarnation, principal
collision, missing governance record, immutable conflict, stale head, or
revoked key fails before any new decision or head is written. Persisted
governance corruption latches tenant promotion health and fences new governance
mutations; it is never repaired by overwriting signed authority.

### D10. Validate the stored-plus-incoming governance union on recovery and import

Full promotion recovery validates the union of:

- root-certified key registrations and prospective revocations;
- signed policy revisions and their exact independent approval;
- v2 source jobs and immutable evidence governance bindings;
- signed promotion intents in decisions;
- M18/M19 predecessor chains and derived heads.

Native snapshot preflight performs the same validation over existing plus
incoming protected authority before mutating the tenant. Same-id/same-content
records are idempotent; divergent immutable content, a missing referenced
registration/policy/approval, an invalid signature, a principal collision, or
an invalid mixed-version chain rejects the import. Imported head projections
remain non-authoritative and are rebuilt only from validated decisions.

Recovery and snapshot import never use a revocation to reinterpret valid
pre-revocation history. They do prevent the revoked key from authorizing a new
record after its effective time.

This is signed content validation, not signed snapshot freshness. M19 does not
sign a whole backup, publish a transparency checkpoint, detect replay of an
older complete valid snapshot, prove snapshot custody, or add ArangoDB snapshot
support. Those remain responsibilities of the deployment and backup system.

### D11. Keep M18 v1 history readable but remove it from new promotion authority

Existing M18 `PromotionContext` v1 jobs, evidence, decisions, and selected
heads remain readable and recoverable as historical records. Their original
meaning is not rewritten, and M19 never fabricates signatures for them.

M18 v1 contexts may continue to run as diagnostic evaluation jobs, but they
cannot register new promotion evidence after M19 enforcement. Existing v1
evidence cannot receive a new promote or reject decision. A signed M19
rollback may name only the exact prior v1 evidence already recorded in the
current contiguous decision chain; this narrow safety path does not approve a
new candidate or retro-sign the legacy evidence. All other new promotion
authority requires four newly executed context-v2 jobs, v2 evidence, and a
signed promoter intent.

The foundation migration path is a fresh target channel. Bridging a non-empty
M18 target into an M19 chain, retroactive attestation, or automatically
selecting a candidate during upgrade is out of scope. Existing M18 heads remain
visible until an operator deliberately moves consumers to a separately
governed target; CogniGraph itself performs no such move.

### D12. Preserve the singleton selection-only boundary

M19 retains one process-local transition lock and one supported writer per
tenant store. It adds no distributed lease, quorum, replica coordination,
consensus, or high availability. The single policy approval is a governance
approval, not a database quorum.

Promotion continues to select one tenant-local `{space_type, channel}` head.
It does not deploy a binary, rebuild or replace a graph, route traffic, restart
a service, retract already materialized facts, or notify an external consumer.
Those actions require a separate deployment control plane that consumes the
signed selection deliberately.

### D13. Separate host lifecycle authority from tenant governance authority

M19 uses a separate `COGNIGRAPH_HOST_ADMIN_PASSWORD` bootstrap credential for
the control-plane `host-admin` identity. Bootstrap passwords require enabled
authentication and a configured JWT secret. HostAdmin holds only the
`TenantAdmin` scope: it may create, suspend, activate, or delete tenants, set
tenant quotas, and provision the first Admin for an active tenant that has
none. It cannot read tenant data or operate tenant governance.

A tenant Admin can manage same-tenant users, submit externally root-signed
public key registrations and revocations, inspect authority, and run promotion
reconciliation/recovery. It cannot acquire `TenantAdmin`, author or approve a
policy, or decide a promotion. Suspending the default data tenant does not
invalidate the independent HostAdmin identity. This split prevents the
credential that controls tenant lifecycle from silently becoming a tenant
governance superuser.

## Acceptance matrix

| Area | Acceptance contract | M19 status |
|---|---|---|
| Root | One externally pinned host-wide Ed25519 public key; server/CLI private-key fields are rejected and no private signing key is accepted, used, or persisted there | Verified 2026-07-18 |
| Keys | Immutable root-certified tenant/incarnation-bound registrations and prospective revocations for exactly author, approver, and promoter purposes | Verified 2026-07-18 |
| Separation | Author, one approver, and promoter are three distinct stable `principal_id` values; key rotation cannot evade the check | Verified 2026-07-18 |
| Policy | Closed resolved policy revision is immutable and signed by its authorized author | Verified 2026-07-18 |
| Approval | Exactly one independent authorized approver signs the exact revision, digest, and target | Verified 2026-07-18 |
| Context | `PromotionContext` v2 freezes exact key, policy, approval, target, and digest bindings in all four source jobs | Verified 2026-07-18 |
| Intent | Promote/reject/rollback intent is signed by the distinct promoter and stored in the immutable decision | Verified 2026-07-18 |
| Gates | Signatures never bypass M18 replay, denominator, recall, restraint, regression, oracle, baseline, or stale-head gates | Verified 2026-07-18 |
| Revocation | Revocation blocks future authorization without rewriting valid pre-revocation history | Verified 2026-07-18 |
| Recovery | Status, recovery, reconciliation, and snapshot preflight validate the stored-plus-incoming governance union and rebuild only derived heads | Verified 2026-07-18 |
| Compatibility | M18 v1 authority remains readable/diagnostic and cannot create new evidence or promote/reject decisions; a signed rollback may restore exact prior chain evidence without retro-signing it | Verified 2026-07-18 |
| Host boundary | Separate HostAdmin owns tenant lifecycle only; tenant Admin owns trust bootstrap/recovery but neither receives the other's scopes | Verified 2026-07-18 |
| Boundary | External artifact truth, snapshot freshness, deployment, HA, and distributed mutation remain explicitly out of scope | Explicitly preserved |

## Explicit non-goals and limits

- M19 does not fetch or hash the bytes named by candidate, corpus, graph,
  oracle, scorer, verifier, case-manifest, or reproducibility URIs.
- A signature authenticates who made or approved a statement; it does not make
  an authorized attester's statement true.
- Corpus/graph/oracle immutability and oracle read-set isolation retain M18's
  external-assertion boundary.
- M19 does not add signed snapshot manifests, freshness counters, an external
  transparency log, rollback protection for an entire valid old backup, or
  proof of custody.
- Root-key compromise, principal-private-key compromise before prospective
  revocation, and signer-device security remain deployment risks.
- There is no policy delegation, group membership, threshold approval,
  break-glass self-approval, or emergency force-promote path.
- Existing exclusions, aggregate-only evaluation, 10,000-actionable-decision
  target bound, and tenant-wide materialization limits remain unchanged.
- M19 does not make ArangoDB strategic and adds no backend-specific governance
  semantics.

## Verification evidence (release gate passed 2026-07-18)

The release build and exact repository Rust gates passed after the final M19
authority and recovery hardening:

```sh
cargo build --release --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Focused suites reported 13 passing `cognigraph-governance` tests, 8 auth unit
tests plus 2 auth integration tests, and 155 server tests including focused
signed-governance and promotion coverage. Coverage includes strict
canonicalization and NFC
identity, weak/invalid key material, purpose/domain/tenant/incarnation binding,
proof of possession, exact immutable idempotency, active-key intervals,
prospective revocation, stale JWT rejection, distinct-duty enforcement,
signed policy/approval/intent verification, failed-gate blocking, v1 migration
rules, decision/head recovery, source-job provenance, and stored-plus-incoming
snapshot-union conflicts.

The freshly rebuilt v2.5.0 release binaries were then exercised over real HTTP
against isolated persistent Native stores with authentication enabled, a
non-default tenant, separate `host-admin`, and exactly one tenant `admin`
bootstrap. Three distinct users and root-certified keys completed the signed
policy, independent approval, context-v2 evaluation, evidence, promote,
reject, and rollback lifecycle. Cross-duty use, a tampered signature, and a
request containing private-key material were rejected. A failed evaluation
gate produced an immutable `blocked` decision without moving the head.

A prospective revocation rejected new use of the affected key while an exact
pre-revocation idempotent replay continued to return the original historical
record. A real process restart recovered the signed authority and selected
head. Full operator recovery remained healthy. A snapshot with tampered signed
authority was rejected before mutation; source-job and mixed-generation
conflicts were likewise covered by integration tests. Tenant isolation held,
suspension fenced tenant operations, and the separate HostAdmin successfully
reactivated the tenant without acquiring tenant governance authority. All
isolated Native stores and temporary signing/harness artifacts were removed.

The `.env` database-scoped ArangoDB credentials returned HTTP 200 from the
configured database and identified ArangoDB 3.12.9-1; the same credentials
were intentionally unauthorized for the server-wide root endpoint. The same
release server completed the signed lifecycle against the live ArangoDB
backend, including tamper, cross-duty, private-key-field, revocation, and
historical replay checks. A real restart preserved the signed authority and
selection state, and recovery remained healthy.

Cleanup deleted only that Arango probe's records. A final database query
verified zero matching rows in the users, governance keys, revocations,
policies, approvals, evaluation evidence, promotion decisions, heads, jobs,
and job catalog collections. The release process was stopped and its port was
released. Credentials, JWTs, and private signing keys were never printed or
stored in the repository.
