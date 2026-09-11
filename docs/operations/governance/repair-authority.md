# Repair Authority

### M25 governed Semantic Repair authority

M25 does not add another promotion generation. It binds the unchanged M22
schema-v1 typed construction candidate to two new immutable signed records and
resolves it through the existing M18-M24 promotion head:

1. Start with a canonical `ConstructionCandidateArtifact` containing one base
   space type and the sorted accepted alias, relation-hint, and relation-blocker
   neurons. The candidate's `base_space_type.id` must equal the target
   `space_type`; rank hints remain outside this reproducible construction
   candidate.
2. An authenticated `policy-author` whose active root-certified registration
   has purpose `policy_author` signs the exact
   `cognigraph.semantic-repair-revision.v1` statement outside CogniGraph. Submit
   it with `POST /api/semantic-repairs/revisions` and an `Idempotency-Key`.
   CogniGraph recomputes the canonical candidate digest and natural revision
   id; a caller-provided mismatch is rejected. The statement must carry
   `base_promotion_head_decision_id` equal to the currently applied decision,
   or explicit JSON `null` for a fresh target. Omission and a stale value fail
   closed.
3. A different stable principal, authenticated as `policy-approver` and using
   an active `policy_approver` registration, signs one final approve or reject
   statement under `cognigraph.semantic-repair-review.v1`. Submit it to
   `POST /api/semantic-repairs/revisions/{revision_id}/review` with an
   `Idempotency-Key`. Approve and reject share one natural review id, so the
   immutable revision cannot acquire two final decisions.
4. Run the unchanged M23 evaluation and signed promotion workflow for the same
   target. Approval alone is inert.
   `GET /api/semantic-repairs/current/{space_type}/{channel}` succeeds only if the
   current existing promotion head selects that exact approved candidate
   digest. An approved but unselected revision, selected but unapproved
   candidate, rejection, stale target, signature failure, or digest mismatch
   fails closed. The applied promoter's stable principal must also differ from
   both the revision author and reviewer.
5. Explicitly invoke `POST /api/construct/governed-ingest` with the same target
   and at most 1,000 supplied chunks. The server resolves the current M25
   authority again, takes the base space and accepted neurons directly from
   the immutable candidate, and atomically reconciles the derived graph on a
   backend with batch capability. It does not copy the candidate into the
   legacy authority collections first.

An existing signed rollback may select a prior candidate, but it does not
bypass M25. `current` and governed ingest resolve that rollback only when the
prior digest has exactly one compatible approved revision anchored to the
appropriate predecessor and still satisfies three-principal separation.

The promotion transition lock stays held through that one atomic graph batch,
so its selection cannot race its writes. It is not a corpus-wide generation
transaction: a larger corpus needs multiple requests, and another promotion
may occur between them. Inspect and retain each response's head, candidate,
revision, and review ids; if a single materialized generation is required, wait
use the M26 generation build and signed deployment flow below rather than
treating multiple governed-ingest calls as atomic deployment.

The CLI mirrors the signed-control flow:

```sh
cognigraph semantic-repair revision submit @semantic-revision.request.json \
  --idempotency-key semantic-revision-1
cognigraph semantic-repair revision list --limit 8
cognigraph semantic-repair revision show REVISION_ID
cognigraph semantic-repair review submit REVISION_ID \
  @semantic-review.request.json --idempotency-key semantic-review-1
cognigraph semantic-repair review list --limit 50
cognigraph semantic-repair review show REVIEW_ID
cognigraph semantic-repair current SPACE_TYPE CHANNEL
```

Revision lists default to 8 records and accept pages up to 8; their compact
summaries omit the potentially 8 MiB candidate and detached signature, so use
the detail route/`revision show` for complete authority. Review lists default to
25 and accept pages up to 100; their summaries likewise omit the detached
signature and request/idempotency internals, while `review show` returns the
complete public record.
Each tenant incarnation is capped at 10,000 immutable revisions and 10,000
immutable reviews. Revision candidates remain individually capped at 8 MiB of
canonical JSON and are additionally capped at 64 MiB in aggregate per tenant
incarnation. Capacity exhaustion fails new submissions rather than evicting
history, and admission, recovery, current resolution, and Native snapshot
preflight recompute the checked aggregate. Whole-authority scans use 256-key
keyset pages and load full records one at a time. Public revision pages use one
cursor-bound key-only lookahead plus at most eight full reads, bounding retained
candidate data to 64 MiB plus record/JSON overhead; review pages retain the
25-record default and 100-record maximum. These are correctness and memory
guards, not throughput promises.

Direct database/filesystem administration remains an operator trust boundary.
A directly inserted single malformed document can be larger than the API's
transport and candidate limits, so the application must materialize that one
record before rejecting it; it never bulk-loads all M25 candidate payloads in
one backend response.

The checked public-only placeholders live in [`fixtures/m25/`](../../../fixtures/m25).
They contain no usable key or signature. The server and CLI are pre-signed JSON
passthroughs and never accept private material.

M25 also closes the ordinary API bypass around semantic state. Generic writes
to `space_types`, `neurons`, `review_policies`, `eval_specs`, `entities`,
`chunks`, `mentions`, and `facts` are rejected through collection/document,
embedding, batch, graph, Lua, and CGQL mutation surfaces. Generic document and
parsed-CGQL reads remain compatible. Backend-native raw query text cannot be
safely classified as read-only, so any raw query or collection bind value that
names one of these managed collections is rejected. Direct database or
filesystem administration remains an operator trust boundary.

The existing dedicated draft, neuron, and legacy construction routes retain
their typed internal access, but their mutable rows do not become M25 authority
retroactively. In particular, an existing `review_policies/{space}` document
and its `qualified_judges` list are not a signed model-qualification ledger.
The LLM judge is an upstream authoring aid: only eligible `relation_hint`
proposals can enter its auto-accept lane, and malformed taint-screener or
quality-judge output queues for human review. A legacy policy may still name
`alias`, but aliases always queue; blockers and rank hints likewise never
auto-accept. A candidate becomes governed only through the separate
PolicyAuthor signature, independent PolicyApprover review, and exact promotion
head match.

Selection does not automatically run governed ingest. M25 has no repair
scheduler, drift monitor, durable semantic-repair job, automatic corpus
re-ingestion, graph-generation registry, atomic consumer switch, signed judge
qualification, new M20 artifact kind, CAS delivery, distributed writer, or HA
claim. M26 adds the explicit generation and Native switch without making any of
those actions automatic. See the
[M25 decision](../../decisions/decision_m25_governed_semantic_repair_authority.md).
