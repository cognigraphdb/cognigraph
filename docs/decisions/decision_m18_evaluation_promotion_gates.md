# Decision: M18 evaluation promotion gates

**Status:** Implemented and verified (2026-07-18).

## Context

M16 made `construct.evaluate` durable, and M17 made its queue bounded,
tenant-fair, repairable, and operable. Those milestones preserve an evaluation
specification and its result, but a succeeded job still proves only that one
measurement ran. It does not identify a candidate artifact, bind all effective
configuration, pin comparable corpus/graph/oracle revisions, prove an
independent replay, or record an attributed promotion decision.

The current construction evaluator is deliberately small. It reports distinct
expected facts found and forbidden facts triggered, while `recall_ok` means
`found == total` and `restraint_ok` means zero violations. Those booleans are
useful diagnostics, not promotion authority. The evaluator also accepts empty
denominators and silently ignores malformed fact strings, so M18 must validate
promotion inputs more strictly than the legacy evaluation path.

Historical Semantic Neurons blind/hostile reports, the WebNLG pilot, and the
DailyMed clinical and calibration artifacts remain important research evidence.
They were not produced from an M18-frozen context and do not satisfy the M18
revision, replay, comparability, or oracle-separation contract. They therefore
remain diagnostic and cannot be imported or relabeled as promotable evidence.

M18 adds a governed selection record, not an automatic deployment system. The
unit of promotion is one tenant-local target `{space_type, channel}`. A channel
is an authored deployment lane such as `development`, `staging`, or
`production`; M18 stores which candidate is selected for that target but does
not deploy binaries, rebuild a graph, or reconfigure an external consumer.

## Decision

### D1. Limit the foundation to context-bearing deterministic construction evaluations

The only promotable M18 measurement is a durable `construct.evaluate` job that
received a complete `PromotionContext` before execution. This first scope does
not promote:

- legacy durable evaluation jobs without a promotion context;
- synchronous `/api/construct/evaluate` results;
- answer-level, LLM-judge, stochastic, WebNLG, or clinical-reference scores;
- fixture reports or manually assembled headline percentages;
- an evaluation context attached after a job ran.

Those results may be linked as diagnostics, but they cannot satisfy a gate or
serve as an immutable baseline. Supporting stochastic or externally scored
lanes requires a later versioned evidence adapter; it is not inferred from a
similar-looking recall or precision field.

M18 does not silently promote a gate-passing candidate. An automated evaluator
may recommend or block, but only an explicit attributed Admin decision changes
the selected head for a target.

### D2. Freeze the complete `PromotionContext` before job execution

Submission resolves, validates, and freezes a versioned context alongside the
existing evaluation specification. The frozen job execution payload and its
digest include both. A client-input digest computed before stored-spec
resolution is not sufficient.

The context contains at least:

```text
schema_version
target { space_type, channel }

candidate {
  kind, id, revision,
  artifact { uri, media_type, bytes, digest },
  candidate_digest
}

effective_configuration {
  construction_config_digest,
  evaluation_config_digest,
  eval_spec_digest,
  preprocessing_digest,
  scorer_id, scorer_version, scorer_artifact_digest
}

revisions {
  corpus: RevisionAttestation,
  graph: RevisionAttestation,
  oracle: RevisionAttestation
}

case_manifest {
  artifact, digest, reviewed_case_union_digest,
  exclusions, exclusions_digest,
  expected_distinct, forbidden_distinct
}

policy {
  schema_version, policy_id, policy_revision, source_digest,
  allowed_candidate_kinds, allowed_candidate_differences,
  evaluator_id, metric_semantics_version,
  recall, restraint, exclusions,
  required_runs, require_exact_replay, oracle
}

reproducibility
oracle_separation
```

Digests are algorithm-prefixed, currently `sha256:<64 lowercase hex
characters>`. Candidate, case, and revision identities are bound by those
declared digests rather than by a URI alone. The server validates their shape
and cross-field bindings, canonicalizes the resolved context, and computes its
digest; it never trusts a client-supplied aggregate digest or pass/fail
boolean. It does not fetch the referenced artifact bytes. In particular, the
pinned scorer and oracle-verifier hashes are operator-trusted semantics
identities derived from fixed server-known identity strings, not proof that
CogniGraph cryptographically verified an external executable.

Baseline job ids, original/replay roles, evidence ids, and promotion-head
references do not exist yet and are therefore not part of `PromotionContext`.
They are supplied and validated when the four completed jobs are registered as
one evidence bundle. A baseline or replay relationship cannot be attached to or
used to reinterpret an individual job after it ran.

Changing a candidate, policy, case manifest, revision, reproducibility field,
oracle attestation, or effective configuration requires new evaluation jobs.
The evidence registrar later requires the two jobs assigned to each
original/replay pair to have equal frozen execution-context digests.

`fixtures/m18/promotion-evaluation.json` is the checked complete authoring
example for a native `POST /api/jobs` submission. Its Rust test recomputes the
EvalSpec digest, validates the closed context and backend binding, and applies
the pinned oracle policy; it is a schema example, not evidence that its example
identities refer to production artifacts.

### D3. Validate cases strictly and register one immutable four-job evidence bundle

Promotion-context validation fails closed before execution when:

- the schema or metric semantics are unknown;
- the evaluation contains no questions, expected facts, or forbidden facts;
- a question id is empty or duplicated;
- a fact is malformed or has an empty source, relation, or target;
- an expected fact is also forbidden;
- declared distinct counts, exclusions, or reviewed-case union do not match
  the canonical case manifest.

Duplicate mentions across questions remain inspectable, but promotion scores
the canonical distinct-fact union. M18 v1 does **not** implement excluded-case
scoring: the case-manifest exclusion list and policy allowed-reason list must be
empty, policy `max_count` must be zero, and `exclusions_digest` must identify
the canonical empty list. Any non-empty exclusion fails context validation.
A later evidence-schema version must define how excluded cases affect
denominators before exclusions can be enabled.

Context-bearing evaluation jobs retain the ordinary M16/M17 execution and
terminal-write lifecycle. M18 does not auto-create evidence before or during a
job's succeeded transition.

After all required work exists, `POST /api/promotions/evidence` registers one
immutable evidence bundle from four explicitly ordered source fields:

```text
candidate_original_job_id
candidate_replay_job_id
baseline_original_job_id
baseline_replay_job_id
expected_head_decision_id
```

The ordering assigns original/replay and candidate/baseline roles; those roles
are not inferred from timestamps or mutable labels on the jobs. All four jobs
must be succeeded, tenant-incarnation-local `construct.evaluate` jobs with
valid frozen promotion contexts. The registrar loads the authoritative hot or
archived job records, recomputes canonical digests and counts, and refuses
legacy jobs or client-submitted score projections.

The immutable `_cognigraph_evaluation_evidence` bundle contains:

- tenant, tenant incarnation, target, schema version, evidence id, and
  evidence digest;
- the four ordered job ids plus each job's status, attempt, recoveries, actor,
  timestamps, submitted-input digest, frozen-execution/context digest, EvalSpec
  digest, and normalized result digest;
- candidate and baseline artifact/configuration/revision identities copied
  from the four authoritative job contexts;
- the identical resolved policy bytes and digest copied from all four jobs;
- exact recall `{found,total}` and restraint `{violations,total}` projections
  for candidate original/replay and baseline original/replay;
- canonical sorted missing and violation cases, exclusions, and
  reviewed-case-union digests;
- reproducibility and oracle-separation projections copied from the source
  contexts;
- explicit pair-equivalence, baseline-comparability, regression, denominator,
  recall, restraint, and oracle gate results;
- the observed `expected_head_decision_id` and the resulting rollback target
  that a later decision must compare again.

Evidence registration requires an `Idempotency-Key`. The evidence identity is
derived from tenant incarnation and its key hash, while the canonical four-job
request is hashed separately. An exact replay returns the same bundle; changed
input conflicts. Once inserted, the bundle is never updated. A divergent
immutable-key conflict degrades promotion health and is never overwritten.

Structural failures such as a missing job, legacy context, digest mismatch,
invalid policy, or mismatched target prevent bundle creation. A structurally
valid bundle is still persisted when a measured recall, restraint, regression,
comparability, or oracle gate fails; its immutable failed assessment is what an
Admin may reject. Registration never drops failed evidence merely because it
is not promotable.

### D4. Use a versioned resolved integer policy with independent hard gates

The promotion policy is fully resolved and copied into each job context before
the job runs. Loading current policy during evidence registration or at
decision time would reinterpret observed results and is forbidden. Evidence
registration requires byte-equivalent resolved policy bytes and digests across
all four source jobs. A policy change requires four new jobs and a new evidence
bundle.

M18 v1 treats the authenticated operator who authors this resolved policy as a
trust root. CogniGraph validates the closed schema and applies the authored
integer gates exactly, but it does not look up an independently approved policy
registry or prevent that operator from choosing permissive ratios and limits.
The policy identity/digest makes that choice visible and immutable; it does not
provide separation of duties.

The minimum v1 resolved policy contains:

```text
schema_version = 1
policy_id, policy_revision, source_digest
allowed_candidate_kinds
allowed_candidate_differences [
  candidate_identity,
  construction_configuration,
  graph_revision,
  resolved_configuration
]
evaluator_id = "construct.evaluate"
metric_semantics_version = "cognigraph.distinct-fact-set.v1"

recall {
  min_expected_distinct > 0,
  min_ratio_numerator,
  min_ratio_denominator > 0,
  max_missing,
  max_additional_missing_vs_baseline
}

restraint {
  min_forbidden_distinct > 0,
  min_ratio_numerator,
  min_ratio_denominator > 0,
  max_violations,
  max_additional_violations_vs_baseline
}

exclusions {
  allowed_reason_codes = [],
  max_count = 0,
  require_same_manifest_as_baseline = true
}

required_runs = 2
require_exact_replay = true

oracle {
  required_status = "verified",
  verifier_name = "cognigraph.oracle-separation-manifest",
  verifier_version = "1",
  verifier_artifact_digest = pinned semantics identity,
  required_stages = [
    candidate_build: oracle reads forbidden,
    candidate_tuning: oracle reads forbidden,
    construction: oracle reads forbidden,
    evaluation: oracle reads allowed
  ]
}
```

The policy does not carry a caller-supplied aggregate digest. Evidence
registration computes the canonical policy digest and stores it with the
bundle. `allowed_candidate_differences` is a closed, non-empty, sorted, unique enum
allowlist. A differing candidate identity, construction configuration, graph
revision, or resolved configuration is comparable only when its exact enum is
present; unknown difference dimensions are rejected by the closed schema.

Ratio numerators must not exceed their denominators. All comparisons use
integer cross multiplication:

```text
recall passes when
  found * min_ratio_denominator >= total * min_ratio_numerator

restraint passes when
  (total - violations) * min_ratio_denominator
    >= total * min_ratio_numerator
```

The absolute missing/violation and baseline-regression limits must also pass.
Recall and restraint are evaluated separately for candidate original and
candidate replay. There is no average, weighted composite, or Admin override
that can hide one failed gate behind the other.

M18 v1 persists and gates only the aggregate distinct expected/forbidden fact
sets exposed by `EvalSpec` and the current evaluator. Per-suite, per-stratum,
and ontology-level groundability gates require a future evaluator/evidence
schema; the foundation does not claim those dimensions.

There is no universal denominator default. Every policy must author positive
expected and forbidden minima appropriate to its frozen case manifest. A
policy with absent or zero minima is invalid rather than permissive.

### D5. Require an immutable comparable baseline and reject stale heads

The baseline is the ordered baseline original-plus-replay job pair copied into
an immutable M18 evidence bundle, not a legacy job or historical report. Both
jobs must be context-bearing, succeeded, replay-equivalent, and evaluated under
the exact resolved policy carried by the candidate pair.

The server compares at least this tuple:

- tenant, tenant incarnation, `space_type`, and channel;
- evaluator and metric-semantics versions;
- corpus and oracle manifest digests;
- case-manifest, reviewed-case-union, EvalSpec, and exclusion digests;
- scorer, preprocessing, construction, and evaluation configuration digests,
  except for candidate dimensions the policy explicitly permits to differ;
- source-corpus and graph-lineage attestations;
- deterministic execution and result-normalization versions.

The candidate artifact and its resulting graph revision may differ from the
baseline because those are often the change being measured. Both graph
attestations must nevertheless prove lineage from the same source corpus and
bind the exact candidate and construction configuration. A policy may narrow
the closed four-value difference set described in D4; it cannot authorize any
other difference or remove corpus, oracle, case, evaluator, scorer,
reproducibility-environment, or graph-lineage comparability. Because v1
exclusions are empty, candidate and baseline case manifests are exactly equal.

The candidate denominator and reviewed-case union cannot shrink relative to
the baseline. Candidate missing facts and violations must remain within the
policy's independent absolute and regression limits.

If the target already has a promoted head, the baseline candidate and the
evidence-registration `expected_head_decision_id` must name that exact applied
decision. A later head change makes the bundle stale and blocks promotion with
a conflict. A target with no head supplies an explicit null expected head and a
bootstrap baseline pair; absence of a head never waives four-job evidence or
baseline comparison.

### D6. Require an original run, an independent replay, and verified oracle separation

Evidence registration requires four separately executed durable jobs:

1. candidate original;
2. candidate independent replay;
3. baseline original;
4. baseline independent replay.

The evidence-registration request assigns these roles through its four ordered
job-id fields. Each job runs the evaluator rather than copying a result. Within
the candidate pair and baseline pair, frozen execution/context digests,
normalized integer totals, per-case missing/violation artifacts, and result
digests must be identical. Candidate and baseline pairs must carry the same
target, policy, cases, evaluator semantics, corpus/oracle revisions, and other
comparison dimensions required by D5. Every candidate run must independently
pass recall, restraint, denominator, regression, revision, and oracle checks. A
single run cannot promote even if its score is perfect.

Each frozen job context supplies reproducibility metadata: deterministic and
dirty-tree flags, source commit and source-tree digest, executable digest,
toolchain and target, backend kind and version, command digest, resolved
non-secret configuration digest, optional seed, and optional provider/model.
M18 v1 requires deterministic clean-tree execution and requires provider and
model to be absent. The evidence bundle copies those fields and adds the
normalized result digest from each completed source job.

Oracle separation is represented by a verified attestation, not a submitted
boolean:

```text
status: verified | failed | unverifiable
promotion_oracle_digest
stage_projections [
  {stage: candidate_build,  read_set_digest, oracle_reads: 0},
  {stage: candidate_tuning, read_set_digest, oracle_reads: 0},
  {stage: construction,    read_set_digest, oracle_reads: 0},
  {stage: evaluation,      read_set_digest, oracle_reads}
]
verifier_name, verifier_version, verifier_artifact_digest, verifier_run_id
attested_by, verified_at_ms, attestation_uri, attestation_digest
overlapping_document_ids, overlapping_case_ids,
overlapping_content_digests, overlapping_artifact_digests
```

Only `verified` passes. The policy and attestation stage arrays must have the
exact four stages above in that order. Candidate build, tuning, and construction
must report zero oracle reads; evaluation may read the promotion oracle. All
four overlap counters must be zero, and the promotion-oracle digest must equal
the frozen oracle revision manifest digest. The attestation content digest is
recomputed with its digest field removed. The verifier name, version, and
semantics-identity hash are pinned, and `attested_by` must name that verifier.
These checks bind an operator-supplied attestation consistently; they do not
cryptographically verify the verifier executable or independently observe
filesystem/database access.

WebNLG's current test boundary is procedural and its relation lanes receive
oracle-derived entity surfaces. Its historical score is therefore diagnostic
under M18 v1. Likewise, DailyMed calibration is not held-out clinical
validation, and stored blind/hostile fixture reports lack M18 contexts and
independent durable replays.

### D7. Require external immutable revision attestations at the current backend boundary

`GraphBackend` has no portable immutable graph revision or snapshot identity.
M18 therefore requires explicit external `RevisionAttestation` records for the
corpus, graph, and oracle:

```text
kind, revision_id, manifest_artifact, manifest_digest
candidate_digest and configuration_digest where applicable
source_lineage
immutability_method
issuer, issued_at
verification_artifact, verification_digest
```

The server validates the schema, digest format, candidate/configuration
bindings, immutable declaration, and equality required by policy. The
attestation issuer remains an operator trust root: CogniGraph does not fetch the
manifests, verify a signature, or claim that a mutable live backend became
immutable merely because a caller supplied an id. If the operator cannot
establish stable content for both original and replay, the context must not be
marked immutable and promotion is blocked.

This is an explicit foundation limit, not a backend revision feature. A future
portable snapshot API can replace the external graph attestation only through a
new evidence-schema version.

### D8. Separate immutable evidence and decisions from the derived promotion head

M18 uses three protected tenant-local system collections:

- `_cognigraph_evaluation_evidence` contains immutable four-job evidence
  bundles;
- `_cognigraph_promotion_decisions` contains immutable attributed decisions;
- `_cognigraph_promotion_heads` contains one derived current head per
  `{space_type, channel}`.

Evidence and decisions are authoritative tenant snapshot data. Their identities
include the tenant incarnation, so deleting and recreating a tenant cannot
inherit an old candidate or head. They are inaccessible through public
document, CGQL, Lua, batch, traversal, search, and backend-native query
surfaces. M17 job archival does not archive, replace, or delete them.

**Security supersession (2026-07-18):** the earlier Admin-only opaque-query
boundary is withdrawn. A live probe showed that backtick Unicode escapes can
bypass textual protected-name screening. `/api/search/query` now accepts only
parsed read-only CGQL for every role, and Lua `graph.query()` is disabled on an
AQL backend even for Admin. On a parsed-CGQL backend, `lua:execute` callers may
use `graph.query()` read-only; permission-gated CGQL mutation additionally
requires `documents:write`.

Protected-name/handle checks and the dynamic/document/collection/graph
capability denylist remain defence-in-depth for internal server-authored query
paths, not authorization to expose public raw AQL.

The head key is deterministic from tenant incarnation, `space_type`, and
channel. A head contains only the current generation, decision id, candidate
identity/digest, evidence-bundle id/digest, resolved-policy digest, explicit
prior rollback target evidence, and update timestamp. It is a projection and
may be discarded and rebuilt; it is never promotion authority by itself.

### D9. Make promote, reject, and rollback explicit Admin decisions

Promote, reject, and rollback mutations require:

- an authenticated Admin in the target tenant;
- a bounded non-empty reason;
- an `Idempotency-Key`;
- the exact target and evidence-bundle reference appropriate to the action.

Promote and rollback additionally require `expected_head_decision_id` as a
compare-and-set precondition. The comparison is against the currently applied
decision id, never merely a candidate or evidence id. Re-promoting the same
candidate or bundle still creates a different applied decision, so a stale
operator request cannot pass through an ABA transition.

The Admin HTTP surface is explicit:

- `POST /api/promotions/evidence` registers the immutable four-job bundle;
- `GET /api/promotions/evidence` and
  `GET /api/promotions/evidence/{id}` list and inspect bundles;
- `POST /api/promotions/evidence/{id}/promote` applies a passing bundle;
- `POST /api/promotions/evidence/{id}/reject` records a rejection;
- `GET /api/promotions/decisions` and
  `GET /api/promotions/decisions/{id}` list and inspect decisions;
- `GET /api/promotions/current/{space_type}/{channel}` reads the derived head;
- `POST /api/promotions/current/{space_type}/{channel}/rollback` applies the
  current head's recorded prior target using explicit `to_evidence_id` and
  `expected_head_decision_id` fields;
- `GET /api/admin/promotions/status` exposes protected-store and reconciliation
  health;
- `POST /api/admin/promotions/reconcile` performs target-bound dry-run or
  applied head reconciliation;
- `POST /api/admin/promotions/recover` performs full tenant-authority
  validation, repairs every derived head, and clears the tenant mutation latch
  only after the complete pass succeeds.

These routes read protected records through the promotion service rather than
generic document access.

The decision id is derived from tenant incarnation and the idempotency-key
hash. Repeating the same key and canonical request returns the same immutable
decision. Reusing it with different input returns a conflict.

Every decision stores:

```text
schema_version, digest_algorithm, decision_id, decision_digest, action, target
tenant, tenant_incarnation
actor { user_key, username, role = admin }
reason, created_at_ms
idempotency_key_hash, request_digest
evidence_id, evidence_digest, policy_digest, gate_assessment_digest
expected_head_decision_id, predecessor_decision_id
resulting_selection {
  generation, evidence_id, evidence_digest, candidate_digest, policy_digest,
  prior_evidence_id
} | null
```

`promote` requires every candidate gate in the evidence bundle to pass and the
currently applied decision id to equal both the request CAS and the bundle's
frozen expected-head linkage. `reject` records why a bundle was refused and
never changes the head; an Admin may reject even a gate-passing candidate.

`rollback` supplies both the expected current applied decision id and the exact
target evidence-bundle id/digest. The target must equal the prior rollback
target recorded by the current promotion chain. A concurrent or intervening
promotion therefore conflicts even when it selected byte-identical evidence.
Rollback does not erase the superseded promotion, invent a new candidate, or
pretend that old evidence was evaluated under a newer policy.

There is no force-promote path *relative to the frozen authored policy*. A
failed or incomplete gate can be followed by a reject decision or new evidence,
never an override of the immutable result. This is not a claim that M18 v1
enforces an independently approved organization-wide quality bar: the same
Admin trust domain can author a permissive policy and later promote evidence
that passes it. A policy registry, distinct policy-approver role, and signed
policy revisions are a later governance milestone.

### D10. Persist decisions first and reconcile derived heads from their chain

For promote and rollback, the server holds the singleton promotion transition
lock and performs this order:

1. reconcile and read the authoritative decision chain and current derived
   head;
2. compare the requested `expected_head_decision_id` with the applied decision
   id, then validate target, explicit target evidence, baseline, replay,
   revisions, policy, oracle separation, and authorization;
3. insert the immutable decision containing its predecessor and intended
   resulting head;
4. upsert `_cognigraph_promotion_heads` from that decision.

The committed decision is authoritative. A stop or storage failure after step
3 can leave a stale or missing head, but cannot leave an unattributed promotion.
The API reports the durable decision id and degraded head state; replaying the
same idempotent request or running reconciliation repairs the projection.

Reconciliation validates immutable digests and follows predecessor/generation
links. It never selects a winner by timestamp. A fork, cycle, missing
predecessor, divergent immutable duplicate, or two decisions claiming the same
generation degrades promotion health and blocks further mutation until the
authoritative conflict is resolved. Reject decisions remain in history but do
not participate in the head-changing chain.

The degraded-health latch is a tenant mutation fence. New evidence and new
promote/reject/rollback decisions fail while it is set. Exact idempotent replays
are checked before the fence so a replay of a committed decision can still
repair its missing head. `POST /api/admin/promotions/recover` materializes and
validates all tenant evidence and decisions, removes malformed or orphaned head
projections, reconciles every target, and clears the latch only on complete
success.

Target reconciliation is bounded to 10,000 actionable decisions. Promote and
rollback admission fails before creating the 10,001st head-changing decision;
blocked and rejected decisions do not consume this target-chain limit. A target
that reaches it must continue in a new channel. This is a foundation lifecycle
bound, not automatic compaction.

Startup, tenant resume, and snapshot import rebuild heads before promotion
mutations become ready. Head loss is recoverable; evidence or decision loss is
not treated as a cache miss.

### D11. Preflight immutable snapshot conflicts and rebuild heads after import

Native snapshot export preserves evaluation evidence and promotion decisions.
Import preflight examines their complete incoming identities and digests before
mutating the tenant store:

- an absent immutable document may be imported;
- the same key with byte-equivalent canonical content is an idempotent match;
- the same key with a different digest is a hard conflict;
- a decision that forks or contradicts the existing target chain rejects the
  import;
- imported `_cognigraph_promotion_heads` documents are ignored as authority.

After the immutable preflight and import succeed, all heads are rebuilt from
the decision chains before the tenant resumes. A failed preflight leaves the
existing evidence, decisions, and heads unchanged. Existing active-job import
fencing remains in force.

Snapshot export and import require an authenticated tenant Admin even when the
rest of a development server would otherwise run with auth disabled. Import is
a trusted-root restore boundary: for every evidence run, preflight reconstructs
the promotion source from the incoming hot/archive job record or the already
stored authoritative job and requires it to equal the source frozen in the
evidence bundle. Divergent hot/archive copies are rejected. This detects
incoherent provenance inside the restored authority, but CogniGraph does not
verify a backup signature. A trusted Admin could provide a coherently rewritten
snapshot, so signature, custody, and transport authentication belong to the
surrounding backup system.

External corpus, graph, and oracle artifacts are referenced by immutable digest
rather than copied implicitly into a tenant snapshot. Import does not claim
those external artifacts are available; missing or unverifiable references
continue to block future promotion while preserving the historical decision.

## Acceptance matrix

| Area | Acceptance contract | M18 status |
|---|---|---|
| Scope | Only context-bearing deterministic `construct.evaluate` jobs are promotable; all legacy and historical results remain diagnostic | Verified 2026-07-18 |
| Frozen context | Candidate, configuration, revisions, cases, closed resolved policy, reproducibility, and oracle attestation are frozen before execution and covered by the execution digest; baseline/replay roles are assigned only during registration | Verified 2026-07-18 |
| Strict cases | Malformed, empty, duplicate-question, overlapping, zero-denominator, manifest-mismatched, or non-empty-exclusion cases fail before execution | Verified 2026-07-18 |
| Evidence | One immutable, tenant-incarnation-bound bundle is registered from ordered candidate original/replay and baseline original/replay succeeded jobs | Verified 2026-07-18 |
| Policy | Resolved integer policy applies positive denominators, the closed difference allowlist, and independent recall/restraint gates without composite or override | Verified 2026-07-18 |
| Baseline | Candidate compares only with an immutable M18 baseline pair over the exact comparable tuple and current expected head | Verified 2026-07-18 |
| Reproducibility | Original and separately executed replay produce identical normalized evidence and independently pass every gate | Verified 2026-07-18 |
| Oracle/revisions | Exact four-stage verified-status oracle attestation and externally attested corpus/graph/oracle identities are mandatory; operator trust boundaries remain explicit | Verified 2026-07-18 |
| Decisions | Admin actions require reason and idempotency; promote/rollback use applied-decision CAS, and rollback names exact target evidence | Verified 2026-07-18 |
| Reconciliation | Decisions commit before derived heads; restart/target reconcile/full tenant recovery repair heads, with a mutation fence on degraded authority | Verified 2026-07-18 |
| Snapshots | Authenticated-Admin import preflights immutable conflicts and source-job provenance without claiming signature verification; derived heads rebuild from decisions | Verified 2026-07-18 |
| Deployment boundary | Per-target heads record governed selection only; the singleton remains non-HA and performs no external deployment | Explicitly preserved |

## Consequences and limits

- Each evidence bundle requires four deterministic evaluations: original and
  independent replay for both candidate and baseline. This is accepted as the
  minimum reproducibility and regression evidence for a promotion decision.
- Existing evaluation history cannot seed promotion by relabeling old records.
  A current candidate and baseline must both be measured again under M18.
- Positive forbidden-fact denominators are mandatory. A suite that measures
  only recall is useful diagnostics but cannot promote under this contract.
- Exclusions are unsupported in v1; every reviewed case remains in the scored
  union until a later schema defines exclusion-aware denominators.
- Immutable evidence and decision history increase tenant storage permanently.
  M18 defines no destructive retention or purge policy for governance records.
- A target channel is capped at 10,000 actionable promote/rollback decisions.
  Use a new channel after that bound; M18 does not compact the immutable chain.
- Full status, recovery, and snapshot-authority validation materialize all
  tenant evidence and decisions in memory. Indexed target reconciliation is
  bounded, but streaming tenant-wide recovery is future scale work.
- A promotion head is a selected revision pointer, not proof that an external
  service deployed it or that already materialized facts were retracted.
- Revision immutability and oracle read-set isolation currently depend on
  operator-trusted external attestations. CogniGraph validates and binds their
  fields but cannot independently observe arbitrary filesystem, model-training,
  or database-admin access or verify the scorer/verifier executable.
- Snapshot restore is likewise an authenticated-Admin trust boundary with
  internal provenance checks, not signed-backup verification.
- Public opaque AQL and Lua AQL are unavailable. This narrows maintenance
  flexibility in exchange for a defensible protected-collection boundary.
- M18 preserves the single-process writer and transition-lock boundary. It adds
  no lease, quorum, distributed scheduler, replica coordination, or high
  availability. Multiple servers mutating the same tenant store are
  unsupported.
- Application-level immutability does not constrain an out-of-band database
  administrator. Divergent protected records degrade health and block
  promotion; they are not silently repaired or overwritten.

## Verification evidence (release gate passed 2026-07-18)

The release build and all standard Rust gates passed after the final public
query-boundary hardening:

```sh
cargo build --release -p cognigraph-server -p cognigraph-cli
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

The focused server suite reported 136 passing tests. Together with the
workspace integration and documentation-contract tests, it covers strict case
validation, independent recall/restraint/regression/oracle gates, exact replay,
revision and baseline comparability, immutable conflict handling, stale CAS,
decision-first projection failure, full recovery, snapshot preflight, protected
collections, and bounded metrics.

The freshly rebuilt v2.5.0 release server was first run against isolated
persistent Native stores with auth enabled. A clean 35-check HTTP run created
nine real durable evaluations: baseline, candidate A, candidate B, and
oracle-unverifiable candidate C original/replay pairs plus one legacy job. It
registered three immutable evidence bundles and five decisions. Exact evidence,
promote, and rollback replays returned the original records; changed evidence
input, legacy promotion evidence, and a stale expected head returned HTTP 409.
Reject did not change the head, candidate B promotion advanced it, rollback
selected the recorded candidate A predecessor, and candidate C produced an
attributed `blocked` decision without changing the head.

Native operator status performed full validation and reported three evidence
bundles, five decisions, one active head, and no repair requirement. Public raw
AQL with a backtick Unicode-escaped protected identifier returned HTTP 403. An
authenticated snapshot was then imported into a fresh Native store after its
derived head was deliberately replaced with forged content. Import rebuilt the
head from immutable decisions; a second process restart preserved the rollback
head and the failed-gate evidence. Both isolated stores and all temporary
artifacts were removed after verification.

The `.env` database-scoped credentials returned HTTP 200 from the configured
database and identified ArangoDB 3.12.9-1; the account remains intentionally
unauthorized for `_system`. The same v2.5.0 release binary then completed the
real HTTP lifecycle against ArangoDB: nine succeeded jobs, three evidence
bundles, five decisions, one rollback-selected head, idempotent replays,
changed-input and stale-CAS conflicts, an attributed reject, and an
oracle-unverifiable blocked decision. Full status and `/health/promotions`
reported healthy. Raw AQL with the Unicode-escape probe and Lua `graph.query`
both returned HTTP 403.

The live Lua denial initially used stale `requires admin` wording even though
Admin is deliberately denied on AQL backends. The message was corrected to
name the explicitly enabled unsafe opaque-query capability, the exact Rust
gates and release build were repeated, and a final live Arango smoke test
confirmed HTTP 403 with the honest capability text. Its ephemeral Admin was
removed and the absence verified afterward.

After a real server stop and restart, ArangoDB retained exactly the three
evidence bundles, five decisions, and rollback head. The test then deleted only
that run's derived head directly in ArangoDB. Full status detected the missing
projection, latched authority unhealthy, and `/health/promotions` returned HTTP
503. A new reject mutation was fenced with HTTP 503. Authenticated
`POST /api/admin/promotions/recover` rebuilt the head from the immutable
decision chain, restored healthy status, and left the decision count at five.

Cleanup removed exactly nine hot jobs, nine catalog rows, three evidence
bundles, five decisions, one head, two uniquely keyed probe facts (including
the seed from a rejected harness-label attempt), and the ephemeral bootstrap
Admin. No archive or tenant row had been created. A final database query
verified zero matching probe records in every affected collection, the release
process was stopped, and the test port was released. Credentials and JWTs were
never printed or stored in the repository; initialized empty schema collections
remain available for the configured CogniGraph test database.
