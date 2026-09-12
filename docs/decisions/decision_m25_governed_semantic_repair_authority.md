# Decision: M25 governed Semantic Repair authority

> Current storage scope (2026-09-12): [Native-only storage](decision_native_only.md)
> supersedes this record's runtime-backend choices and adapter-specific paths.
> Both editions now use Native; public HTTP/Lua queries are parsed CGQL.
> Storage-independent contracts below remain applicable. Earlier backend
> behavior, configuration and verification are retained as dated history,
> not current setup instructions. Use the [operator guides](../operations/README.md).

**Status:** Implemented and verified (2026-07-19).

## Context

M14 made Semantic Neurons operational: a base ontology plus accepted alias,
relation-hint, and relation-blocker neurons can deterministically ground an
evidence-bearing graph. M18-M24 then added strict evaluation gates, signed
separation of duties, exact external-artifact attestations, verified artifact
consumption, reproducible raw-document-to-graph derivation, and durable CAS
recovery.

One authority gap remains between those two lines of work. The M22/M23
promotion chain selects the exact digest of a typed construction candidate,
but the ordinary `space_types`, `neurons`, `review_policies`, and `eval_specs`
collections remain mutable data-plane documents. A caller with a generic write
capability can bypass the intended neuron lifecycle, alter an accepted neuron,
or change the ontology and evaluation inputs without creating an immutable,
independently reviewed authority record. The construction-derived collections
can likewise receive generic writes that are indistinguishable from governed
construction to downstream readers.

M25 closes that narrow gap. It makes one exact, already-supported M22
`candidate.json` value an immutable signed Semantic Repair revision, requires
an independent signed approve or reject decision, and resolves governed
construction only when the existing promotion head selects that exact
candidate digest. It does not add another promotion system or turn selection
into deployment.

## Decision

### D1. Keep the M22 candidate contract byte-for-byte unchanged

An M25 semantic revision embeds the existing closed M22
`ConstructionCandidateArtifact` schema version 1 without adding fields or
changing its meaning:

```text
schema_version = 1
kind
id
revision
base_space_type
accepted_neurons
```

The candidate continues to admit exactly the construction-relevant neuron
kinds supported by M22: alias, relation hint, and relation blocker. Rank hints
remain outside the reproducible construction candidate.

The server canonicalizes the embedded candidate with the existing
NFC-normalized, integer-only canonical JSON rules and recomputes its SHA-256
digest. A caller-supplied candidate digest is never trusted. The candidate's
space id must equal the revision target's `space_type`, and all existing M22
ordering, uniqueness, ontology-reference, configuration-size, and grounding-
work checks remain in force.

M25 introduces no M22 artifact-schema revision and no M18-M24 context, job,
receipt, evidence, decision, head, preparation, consumption, or signed-intent
generation.

### D2. Store one immutable PolicyAuthor-signed semantic revision

M25 adds the protected tenant-local collection:

```text
_cognigraph_semantic_repair_revisions
```

One revision record contains at least:

```text
schema_version = 1
tenant, tenant_incarnation
semantic_repair_revision_id, semantic_repair_revision_digest
target { space_type, channel }
base_promotion_head_decision_id = null | <64 lowercase hex>
candidate
candidate_digest
author_registration_id, author_registration_digest
author_principal_id
signed_at_ms
author_signature
created_at_ms
created_by
```

The signed statement domain is
`cognigraph.semantic-repair-revision.v1`. The author must use an active
root-certified M19 key with purpose `policy_author`; the authenticated caller
must be the registration's subject user. The statement binds the exact tenant,
incarnation, target, observed base promotion-head decision, complete typed
candidate, recomputed candidate digest, author registration, stable principal,
and signing time. `base_promotion_head_decision_id` is required in JSON: it is
explicit `null` for a target with no head or the exact currently applied
decision id for an existing target. Omission and stale values fail closed.

`semantic_repair_revision_id` is the lowercase SHA-256 canonical digest of the
following integer-only canonical JSON identity, with the `sha256:` prefix
removed:

```json
{
  "tenant": "...",
  "tenant_incarnation": "...",
  "target": { "space_type": "...", "channel": "..." },
  "base_promotion_head_decision_id": null,
  "candidate_digest": "sha256:..."
}
```

For a non-empty target, the base field contains the 64-character lowercase
decision id instead of `null`. The base binding is part of both the signed
statement and natural identity, so the same candidate proposed from another
head is a distinct revision.

The collection is append-only. An idempotent replay must repeat the same
natural id, authenticated actor, `Idempotency-Key` hash, and canonical signed
request digest; any difference is an ordinary immutable conflict. Such a
rejected mutation does not itself degrade health. Health latches only when
stored authority fails integrity validation. No API updates or deletes a
revision.

The product server and CLI continue to accept no signing private key. A signer
constructs and signs the exact statement outside CogniGraph, using the
`cognigraph-governance` protocol.

### D3. Require one independent signed approve or reject decision

M25 adds the protected tenant-local collection:

```text
_cognigraph_semantic_repair_reviews
```

One review record contains at least:

```text
schema_version = 1
tenant, tenant_incarnation
semantic_repair_review_id, semantic_repair_review_digest
target
base_promotion_head_decision_id = null | <64 lowercase hex>
semantic_repair_revision_id, semantic_repair_revision_digest
candidate_digest
author_principal_id
approver_registration_id, approver_registration_digest
approver_principal_id
decision = approve | reject
reason
signed_at_ms
approver_signature
reviewed_at_ms
reviewed_by
```

The signed statement domain is `cognigraph.semantic-repair-review.v1`. The reviewer
must use an active root-certified key with purpose `policy_approver`, the
authenticated caller must own that registration, and
`approver_principal_id` must differ from `author_principal_id`. A second key
for the same stable principal is not independent. The review must copy the
revision's exact target, base promotion-head decision, revision digest,
candidate digest, and author principal. Its base field is also required as
explicit `null` or the exact populated decision id; omission is invalid.

`semantic_repair_review_id` is the lowercase SHA-256 natural id of:

```text
tenant NUL tenant_incarnation NUL semantic-repair-review NUL
semantic_repair_revision_id
```

Exactly one final decision exists for a revision. Approve and reject cannot
race into two records because they have the same natural id. Reconsideration
requires a new signed semantic revision; accepted authority is never changed
in place. Review records are immutable and use the same exact
actor/request/idempotency replay, signature, time, tenant, and incarnation
rules as the revision. Key revocation is prospective at each act: the author
must be active when the server admits both the revision and a later review of
it, and the approver must be active when it admits the review. A revocation
therefore blocks new revision or review authority under the affected chain,
but later revocation of either key does not rewrite a review already admitted
while both were active.

An approval authorizes only the exact candidate digest in the named revision.
It does not promote it, choose a channel head, or modify a graph.

### D4. Keep the existing promotion head as the sole selection authority

M25 does not create a second semantic head or a second promotion decision
chain. The existing M18-M24 promotion head for `{space_type, channel}` remains
the only selected-candidate authority.

A governed Semantic Repair resolution succeeds only when all of the following
hold:

1. the current promotion head and its complete immutable decision/evidence
   chain validate under their historical M18-M24 generation;
2. the selected candidate identity and digest resolve to one immutable M25
   semantic revision in the same tenant, incarnation, and target;
3. the revision and review bind the compatible predecessor head from which the
   selected promote decision was authored (or the matching prior promote base
   reached by rollback);
4. the revision's embedded typed candidate recomputes to that exact selected
   candidate digest;
5. one immutable `approve` review validates for the exact revision and
   candidate digest; and
6. author, approver, and M19 promoter separation remains valid.

A missing revision, a rejection, a digest mismatch, multiple conflicting
records, an invalid signature, or a stale/foreign target fails closed. An
approved revision that the promotion head does not select remains inert. A
selected candidate without an approved M25 revision may remain readable as
historical promotion evidence, but it is not a governed Semantic Repair
resolution.

Existing signed promote and rollback decisions already bind exact candidate
and evidence authority. Rollback therefore restores the prior governed
revision only when the newly selected prior digest independently satisfies
the same M25 resolution checks.

### D5. Protect semantic authority and construction-derived collections from generic writes

M25 reserves generic mutation of these legacy semantic-control collections:

```text
space_types
neurons
review_policies
eval_specs
```

It also reserves generic mutation of the construction-derived collections:

```text
entities
chunks
mentions
facts
```

Documents, batch, CGQL mutation, Lua, and generic graph-write surfaces must
reject writes to those names before any backend mutation. Reads remain
compatible so existing queries, inspections, and historical records keep
working. Dedicated server construction and authority components use their
internal typed paths; the protection is not a blanket ban on legitimate
construction work.

The two new underscore-prefixed M25 collections remain unreachable through
all generic read and write surfaces, following the existing system-collection
boundary.

This closes ordinary API bypasses. It is not a claim that a deployment
operator with direct database/filesystem control cannot alter bytes; recovery
and snapshot preflight must detect divergent immutable authority and fail
closed.

### D6. Preserve legacy history without retroactive authority

Existing documents in `space_types`, `neurons`, `review_policies`,
`eval_specs`, and derived collections remain readable under their historical
meaning. M25 does not synthesize signatures, approvals, candidate digests, or
review history for them.

Normal M25 adoption uses a fresh promotion target channel and a newly signed
revision. An operator may deliberately construct a new typed candidate from
legacy material and submit it as a new signed revision; that is prospective
authorization of exact current bytes, not retroactive proof of how the legacy
state was authored or reviewed.

Legacy neuron lifecycle and LLM-review results remain diagnostic. They cannot
by themselves satisfy governed resolution after M25. Current public reads stay
compatible, while mutation documentation moves to the signed revision/review
workflow.

### D7. Validate recovery and snapshot unions, then derive nothing new

Semantic-authority status and recovery validate every stored revision and
review, their root-certified key registrations and prospective revocations,
their target and candidate bindings, and the existing promotion authority
they reference.

Native snapshot preflight validates the stored-plus-incoming union before
applying any bytes. Same-id/same-content records are idempotent; a divergent
immutable record, missing referenced registration or revision, invalid
signature, principal collision, foreign incarnation, or invalid selected
binding rejects the import. Snapshot import does not trust an imported
selection outside the existing promotion-head reconciliation rules.

ArangoDB retains its current operator-native backup boundary. M25 adds no new
CogniGraph application snapshot surface there.

Each tenant incarnation is limited to 10,000 revisions and 10,000 reviews.
The unchanged M22 limit remains 8 MiB of canonical JSON per embedded candidate,
and all revision candidates together are limited to 64 MiB of canonical JSON.
Admission, recovery, current resolution, and Native stored-plus-incoming
snapshot preflight recompute those checked limits and fail closed rather than
evicting immutable history.

Authority scans use 256-key keyset pages and fetch full records one at a time,
so no backend response materializes every candidate. Public revision listing
starts at the validated cursor, returns at most eight records, and uses one
key-only `limit + 1` lookup plus at most eight full-record reads; review lists
retain their 100-record maximum. A direct database operator can still insert
one arbitrarily large malformed document, which one full-record read must
materialize before the application can reject it. That residual database-
administrator risk remains inside the direct-storage trust boundary described
in D5; API submissions are independently protected by transport and per-
candidate limits.

### D8. Keep the foundation deliberately narrow

M25 adds no durable semantic-repair job, scheduler, drift monitor, or automatic
proposal loop. It adds no judge qualification record, new artifact kind, or
model/prompt attestation. LLM suggestions remain upstream, non-authoritative
inputs until a PolicyAuthor signs the exact candidate and an independent
PolicyApprover approves it.

The same delivery closes three fail-open defects in the legacy, non-authority
LLM review lane: aliases are no longer eligible for automatic acceptance until
they have a kind-specific packet, malformed screener or judge JSON becomes
`needs_human` at zero confidence, and an omitted audit sampling rate uses the
documented `0.1` default. Closed fully-required OpenAI judge schemas request
strict structured output, and the server still validates the returned JSON.
These are safety corrections, not signed judge qualification or evidence that
model confidence is calibrated.

M25 does not write, upload, fetch, or deliver CAS bytes. The unchanged M22/M23
artifact and evaluation paths continue to carry and verify `candidate.json`.

Most importantly, a selected governed revision is a control-plane selection,
not a deployed graph generation. M25 does not automatically re-ingest a
corpus, retract materialized facts, switch query consumers, preserve multiple
materialized generations, or atomically replace an operational graph. M26 now
owns one narrower closure: an explicitly invoked, bounded, Native-only complete
target projection with verified impact, retained immutable generations, and a
separately signed atomic activation or rollback. It deliberately does not
retroactively turn M25 selection into deployment or add automatic activation,
multi-channel generic routing, a durable materialization job, generation
garbage collection, or distributed operation.

M25 also adds no distributed writer, lease, consensus, quorum, replication,
or high-availability claim.

## Acceptance matrix

| Area | Acceptance contract | M25 status |
|---|---|---|
| Candidate | Exact unchanged M22 typed candidate; canonical digest recomputed server-side | Verified |
| Revision | Immutable PolicyAuthor-signed tenant/incarnation/target/candidate record | Verified |
| Review | One immutable independent PolicyApprover-signed approve or reject decision | Verified |
| Selection | Existing M18-M24 promotion head remains the sole selected-candidate authority | Verified |
| Resolution | Selected digest must equal one independently approved revision's exact embedded candidate | Verified |
| Mutation boundary | Generic writes to legacy semantic-control and construction-derived collections fail; reads remain compatible | Verified |
| Recovery | Full authority validation and stored-plus-incoming Native snapshot preflight fail closed | Verified |
| Compatibility | M18-M24 wire authority and legacy readable history keep their original meanings | Verified |
| Boundaries | No repair job, LLM qualification, new artifact kind, CAS delivery, deployment, generation switching, or HA | Explicitly preserved |
| Verification | Rust gates plus authenticated release-binary persistent-Native and live-ArangoDB authority lifecycles | Verified |

## Required verification

Completion requires:

- exact and idempotent revision/review submission;
- rejection of tampered candidate bytes/digests, signatures, targets,
  purposes, tenant incarnations, new acts under stale/revoked keys, and
  same-principal review, while valid pre-revocation history remains valid;
- approve/reject natural-id race coverage;
- selected-but-unapproved and approved-but-unselected failure cases;
- promotion rollback resolution to an exact prior approved revision;
- generic write rejection through documents, batch, CGQL, Lua, and graph
  surfaces while reads remain compatible;
- recovery and Native stored-plus-incoming snapshot conflict tests;
- explicit JSON omission/null/populated regressions for any optional signed
  fields introduced during implementation;
- `cargo fmt --all -- --check`;
- `cargo clippy --all-targets -- -D warnings`;
- `cargo test --all`; and
- authenticated release-binary persistent-Native and configured live-ArangoDB
  authority, restart, recovery, tamper, revocation, rollback, and exact-cleanup
  lifecycles.

## Verification evidence

The final repository validation completed successfully:

```text
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets -- -D warnings
PASS: cargo test --all
PASS: cargo build --release -p cognigraph-server -p cognigraph-cli
```

The frozen release build completed in 30.66 seconds at package version 2.5.0.
Its server binary digest was
`99d987799f3e564c7166b7091e3db12a17fad11cb52d07b70c1e585cda9b188f`;
the CLI digest was
`55deb83ba7d79f65ee32eac2c9beeb01c807723b91123870e7f9dacf9a864fa4`.

The focused authority evidence also passed 19 Semantic Repair server tests,
three CLI contract tests, nine OpenAPI drift tests, the legacy alias-policy
compatibility regression, and an independent frozen-diff audit with no release
blocker.

```text
PASS Native release-binary lifecycle (7.34 seconds):
  storage: isolated persistent Native/redb tenant authority
  authority: distinct M19 PolicyAuthor, PolicyApprover, and Promoter chain
  strict wire: explicit null accepted; omitted required nullable base returned 422
  selection: selected-unreviewed returned 409; A -> B -> rollback restored A
  revocation: new review after author revocation returned 403; already admitted
    historical A remained valid
  mutation boundary: protected document, embedding, collection, batch, graph,
    CGQL, Lua, and raw-query paths failed before mutation
  materialization: governed ingest added exactly 2 entities, 1 chunk,
    2 mentions, and 1 fact from the selected candidate
  recovery: divergent snapshot-union import failed without a partial marker;
    CLI list/show/current and restart recovery returned the same authority
  cleanup: release server stopped and all isolated Native paths were removed

PASS ArangoDB release-binary lifecycle (205.57 seconds):
  server: ArangoDB 3.12.9-1 with configured credentials
  isolation: configured database began as an exact 27-collection zero-row
    baseline; disposable database creation was unavailable
  authority/wire/selection/revocation: same signed chain, 422 omission,
    selected-unreviewed 409, A -> B -> rollback A, review-after-revocation 403,
    and historically valid A behavior as Native
  mutation boundary: protected document, embedding, collection, batch, graph,
    CGQL, Lua, and raw AQL paths all failed before mutation
  materialization boundary: governed ingest failed closed on the non-atomic
    backend and entities/chunks/mentions/facts all remained at zero rows
  recovery: restart preserved authority; direct deletion of exactly one head
    made health repair-required, recovery reported repaired_heads=1, and
    current resolution returned A again
  exact cleanup: removed 33 probe records and restored the identical
    27-collection zero-row baseline, including collection types and index
    signatures, with no M25 revision/review collections left behind; release
    server and temporary paths were removed
```

The observed times are end-to-end probe wall times, not throughput, RPO, RTO,
availability, or scale benchmarks. A temporary direct-Arango probe client also
needed a non-default HTTP user agent because an upstream Cloudflare layer
rejected Python's default agent; this was harness-only and did not change the
CogniGraph contract or repository.
