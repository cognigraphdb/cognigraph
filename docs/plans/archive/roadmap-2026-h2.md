# Roadmap 2026 H2 — CGQL v2, Semantic Neurons, and governed operations

Written 2026-07-04, after Phase 10 closed (312 tests at that historical
checkpoint, packaged, operable, administrable; see implementation-plan.md
for the current delivered surface).
Ordering is deliberate: **CGQL v2 first** (the database story), **Semantic
Neurons second** (the differentiator against other GraphRAG offerings —
governed knowledge construction is the moat; storing graphs + vectors is
commodity). Hygiene items interleave anywhere.

Working conventions apply throughout: design discussions before contested
semantics, decision records per call, benchmarks-first for optimizations,
corpus extension for every CGQL change, hard exit-code gates per commit.

## Current position (2026-09-09)

CGQL v2 and the workload follow-ups **D1–D12 are implemented**. D1a uses Native
adjacency for endpoint-equality listings; D1b indexes materialized correlated
equality candidates. D2 adds correlated traversal, D3–D9 add expressions and
helpers, D10 defers projection-only calculations, D11 limits retry work to the
phase that needs it, and D12 adds directed construction. The
[workload decision](decisions/decision_cgql_v2_workload_gaps.md) preserves the
original failures and their resolution; they are not upcoming implementation work.

The July 21 CRM re-run recorded **8 of 8 queries completing**, up from 4 of 8,
with agreeing row counts. The later July 22 release evaluation recorded a
**780 ms CGQL / 927 ms reference** total for its selected query formulations.
Those are historical measurements of one 254,076-document workload, not current
latency guarantees or general backend rankings. The
[benchmark record](benchmarks.md#d11-retry-only-the-phase-that-missed-2026-07-22)
also preserves the workload-dependent costs of correlated traversal and query
formulation. CG-23's source and example checks do not re-run that partner dataset.

Governed milestones M15–M26 remain delivered, with the singleton/Native and
other boundaries recorded below. The September review found defects after
those milestones; completion dates do not establish current correctness. The
[implementation plan](implementation-plan.md#review-checkpoint--2026-09-09)
records fixes through local commit `57d65af` (CG-36): formatting, strict Clippy,
and 961 reported Rust passes,
including eight Arango entries that early-returned without credentials and two
passing live embedding tests. CG-36 also passed 22 release review scenarios and
12 invalid-configuration rejections, with zero outbound judge attempts.
This is local verification; no new remote CI or published release is claimed.
The subsequent CG-24 documentation correction, committed as `b90a28a`, aligns the rev2
paper and HTML/PDF exports with distinct construction counts and per-question
answer units. Twelve offline release replays reproduced the construction scores;
the publication bundle passed local HTTP, text, and visual checks. CG-27's
uncommitted documentation reconciliation now records the completed WebNLG
experiments, exact artifact paths, and offline validation replay, preserving
the historical model and frozen test boundaries.

**Next work:** finish the open [CG issue registry](issues/README.md), which is
the active defect backlog. It includes research and reproduction gaps
(CG-25), modularity and decision indexing (CG-26/30).
Query-spec parity (CG-28), nested generated binding collisions (CG-37),
public query plan-error classification (CG-38), no-op startup derivative
invalidation (CG-35), and dedicated judge endpoint routing (CG-36) are resolved
with regression tests and runtime evidence.
The July research directions below remain proposals for later evaluation.
After CG-1 through CG-32 are resolved or explicitly closed, revisit the latest
DeepSeek, GLM, and other models against the economical `gpt-5.6-luna` baseline,
as recorded in the [deferred benchmark plan](issues/README.md#after-the-current-ticket-list).

## Historical delivery and research checkpoint (2026-07-20–22)

This section preserves the July observations and proposed research sequence.
Its test counts and experimental scores belong to those dates; the current
implementation and defect priorities are summarized above.

At the July 20 checkpoint, governed operations **M15–M26 were complete and
gate-green**; the workspace was
`cargo test --all` green (**772 tests**, fmt + clippy `-D warnings` clean). Arc A
(CGQL v2) and Arc B (Semantic Neurons authored-fixture + WebNLG pilot) closed
earlier. Two empirically-driven advances landed 2026-07-20 (merged PRs #38/#39/#40):

- **Context-expansion side-views** (retrieval recall) — a quarantined,
  write-protected per-tenant Q&A layer: a separate provider axis, an opt-in
  generation job, an opt-in hybrid-search leg, and a CRUD cascade. A live model
  benchmark showed the recall lift is **gated on embedding quality** — net-positive
  with `gemini-embedding-2`, negative with Ollama (`decision_sideviews_provider_and_benchmark.md`).
- **Semantic Neurons on real FDA labels** — the first governed construction run on
  real DailyMed labels, plus an A/B that localized the recall cap to **stage-1 entity
  extraction** and fixed it with **per-document drafting** (`draft_space_type_per_document`
  + a `per_document` flag). Grounding was never the problem; broad sampling starved the
  drug→condition relations (`decision_neurons_real_label_construction.md`).

- **Pilot-grade clinical graph, precision-measured** (2026-07-21) — 100 real FDA labels
  through the full governed pipeline as a durable background job: evidence-bound facts
  from 659 chunks in ~10 s, all 100 labels contributing, measured by an independent,
  pre-calibrated judge. The first build scored 71.5%; dropping administrative relations
  at draft time (D1, keyed on entity type rather than relation name) rebuilt it to
  **1575 facts at 85.0% evidence-supported / 92.5% label-correct**, closing the
  boilerplate failure class entirely (`decision_pilot_clinical_graph.md`).

- **Relation-semantics detector for the gate advisor** (2026-07-21) — the original
  endpoint-presence detector separated nothing on real labels (−2.5%); a second,
  deterministic detector asking whether the licensing sentence *asserts* the relation
  yields a **clean lane at 92.2% vs 75.0% flagged (+17.2%)**, catching 70% of errors,
  with per-fact verdicts persisted in the quarantined `fact_semantics` sidecar (never
  on the attested fact edge). Designed on one ontology and confirmed on a second —
  **three of six candidates were rejected by that holdout**, and a follow-up fix to
  abbreviation-period sentence splitting removed the last false-flag class
  (D2 in `decision_pilot_clinical_graph.md`).

- **Answer-quality head-to-head on real labels** (2026-07-21) — the fixture-kit result
  (graph 86% vs vector 36%) **does not reproduce** on real text with a drafted ontology:
  **vector 36%, graph 22%**. Diagnosis is precise and non-architectural — the graph arm
  hit **97% of its own ceiling**, grounding never misfired, every missing entity was
  already extracted, and **43% of expected facts sit in the graph under a different
  relation label** (true coverage 67%, not 24%)
  (`decision_answer_head_to_head_real_labels.md`).

**Relation-vocabulary normalization was attempted and REJECTED by measurement** (no code
change): the "+43 points" estimate was an artefact — a lexical fold buys +1.4, a semantic
map reaches 47.1% — and the domain-general fix (each document sees the vocabulary so far)
converged labels 22% while **halving** label-neutral answer coverage, so it was reverted.

- **CGQL v2 on an external 254k-document CRM workload** (2026-07-21–22) —
  the initial run completed four of eight questions; per-entity graph-hop queries
  were slow or incomplete. Backend adjacency alone did not fix the correlated
  case: D1b's index over materialized candidates was also needed. The subsequent
  run completed all eight, and D2–D12 were delivered by July 22. See the
  [dated results and corrected diagnosis](decisions/decision_cgql_v2_workload_gaps.md).

The next research sequence proposed in July was **rule-drafting density** —
the leading lever identified then for the ~33% genuinely
un-drafted relations, the stage-1 starvation pattern one layer up (~12 relations drafted
per label where labels assert many times that), accepted by re-running the head-to-head;
then, only if density is exhausted, a **fixed operator-supplied relation vocabulary**
(short and stable, never accumulated) as a second attempt at naming. Then: a supported
"align draft to stored entity identities" step, which every repeat run over a corpus now
needs; HA/replication; client SDKs.

NOTE on CI: the checked-in [workflow](../.github/workflows/ci.yml) is
**manual-trigger only** (`workflow_dispatch`) during active
development — the local commit gates (`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --all`) are the contract on every change, so we don't spend Actions minutes
re-deriving a signal we already have. Dispatch CI deliberately before a release
(`gh workflow run ci.yml`). A billing/spending-limit obstacle was recorded in
July; its current remote status was not checked for this documentation update.

## M26 — Verified Semantic Repair materialization (implemented 2026-07-19)

M26 closes the explicit M25 gap between an authoritative selected repair
candidate and the graph rows served for its space. It does not create another
promotion generation or selector. An explicit synchronous build consumes only
current M22/M23 derivation authority plus the exact approved M25 revision,
verifies the canonical prepared corpus in the tenant-incarnation CAS, and
retains one bounded immutable complete occurrence projection with exact
candidate-versus-baseline impact. A separate externally signed Promoter act
deploys or rolls back that retained generation on the Native backend. The
contract is recorded in the
[M26 decision](decisions/decision_m26_verified_semantic_repair_materialization.md).

- [x] No changes to M18-M25 context, job, artifact-consumption, preparation,
  derivation, evidence, decision, promotion-intent, head, Semantic Repair
  revision, or review wire meanings.
- [x] Synchronous, idempotent generation build from the exact current
  M22/M23+M25 authority, with CAS length/hash/canonical checks, deterministic
  grounding, original/replay semantic-fact equality, and no partial durable
  state on failure.
- [x] Immutable complete entity/chunk/mention/fact occurrence projection and
  exact added/removed/unchanged impact, under the frozen 1,000-chunk,
  10,000-semantic-fact, 50,000-row, and 16-MiB generation plan plus bounded
  tenant/space retention.
- [x] Separate `cognigraph.semantic-repair-deployment-intent.v1` Promoter
  signature, stable-principal separation, exact promotion/M25/impact bindings,
  required explicit nullable fields, and deployed-head compare-and-set.
- [x] One served generation per space and one Native atomic target replacement
  committing graph rows, immutable deployment decision, and derived head;
  signed promotion rollback must precede signed deployment rollback to the
  retained one-step-prior generation.
- [x] Full authority recovery and Native stored-plus-incoming snapshot
  validation/reapplication of the exact deployed target; maintenance-mode
  ArangoDB rejects before CAS reads, collection creation, authority writes, or
  graph mutation.
- [x] HTTP/CLI build, list, detail, deploy, and current-deployment surfaces plus
  checked public-only templates in [`fixtures/m26/`](../fixtures/m26/).

M26 remains an explicit Native-only singleton operation. Promotion does not
build, build does not deploy, and recovery does not choose new authority. It
adds no drift scheduler, durable materialization job, checkpoint/background
retry, LLM/prompt qualification, physical per-generation query routing,
generation deletion or GC, global exact entity-union guarantee, vector-index
generation, distributed writer, quorum, replication, or HA.

## M25 — Governed Semantic Repair authority (done 2026-07-19)

M25 connects Semantic Neurons to the signed candidate-selection foundation
without creating a second promotion system. One existing M22 schema-v1 typed
`candidate.json` becomes an immutable semantic revision signed by an existing
`PolicyAuthor`; an independent existing `PolicyApprover` signs one final
approve or reject decision. The existing M18-M24 promotion head remains the
sole selection authority, and governed resolution requires its selected exact
candidate digest to equal the candidate stored in one valid approved revision.
The bounded contract is recorded in the
[M25 decision](decisions/decision_m25_governed_semantic_repair_authority.md).

- [x] Immutable tenant/incarnation/target-bound semantic revisions containing
  the exact unchanged typed M22 candidate and its server-recomputed canonical
  digest.
- [x] Immutable independent signed review, with one natural id shared by
  approve and reject so a revision cannot acquire two final decisions.
- [x] Exact resolution through the existing promotion decision/evidence/head
  chain; no M25 head, override, force-select path, or alternate ordering.
- [x] Generic write protection for legacy semantic-control collections and
  construction-derived collections across documents, batch, CGQL, Lua, and
  graph surfaces, while reads and dedicated typed paths remain compatible.
- [x] Full authority recovery and Native stored-plus-incoming snapshot
  preflight, plus rollback, revocation, immutable-conflict, fresh-channel, and
  historical-version compatibility regressions.
- [x] Checked request templates in [`fixtures/m25/`](../fixtures/m25/),
  HTTP/CLI/operator documentation, exact Rust gates, and authenticated
  release-binary persistent-Native and configured live-ArangoDB authority
  lifecycles.

This is deliberately an authority foundation. It adds no durable repair job,
drift scheduler, judge qualification, new artifact kind, CAS delivery,
automatic corpus re-ingestion, materialized graph generation switching,
distributed coordination, or HA. LLM suggestions remain non-authoritative
until the exact resulting candidate is signed and independently approved;
M26 now provides the separate explicit Native materialization/deployment phase,
while continuous judge governance and automatic drift-trigger policy remain
later work.

## M24 — Durable CAS custody and verified restoration (done 2026-07-19)

M24 closes the address-durable/byte-durable recovery gap left explicitly by
M21-M23. An Admin-only read endpoint derives a deterministic bounded recovery
plan from one immutable M21-M23 evidence record and its exact candidate and
baseline M20 artifact sets. Offline operator tooling copies those
tenant-incarnation content addresses into a closed bundle, rehashes the copy,
and can restore a completely absent CAS scope through verified commit-last
publication. The contract is recorded in the
[M24 decision](decisions/decision_m24_durable_cas_custody_verified_restoration.md).

- [x] Canonical `cognigraph.artifact-recovery-plan.v1` with historical signed
  authority validation, sorted unique attestations and blobs, checked counts,
  a limit of 8,192 unique blobs, and a configurable 2 GiB default byte bound.
- [x] Shared local-CAS and bundle verifier used by both the server and CLI;
  fixed digest-derived paths only, with symlink/type/length/hash checks and no
  signed-location or logical-path dereference.
- [x] Closed portable bundle, destination reread, backup receipt, pinned
  plan/scope verification, absent-scope restoration, no-replace publication,
  final CAS reread, and a deterministic external restore receipt.
- [x] Exact historical compatibility: no context v7, job v5, consumption v4,
  evidence/decision v7, head v5, or promotion-intent v5.
- [x] Persistent-Native and live-ArangoDB release-binary verification plus
  adversarial tamper, wrong-scope, existing-target, restart, and cleanup cases.

The server remains read-only toward artifact bytes. M24 is not a remote fetch,
upload, synchronization, retention, encryption, replication, freshness, RPO,
RTO, quorum, or HA feature. Native database recovery still requires a backend
snapshot/cold copy; ArangoDB still requires operator-native database backup.
The CAS bundle composes with those database paths rather than replacing them.

## M23 — Reproducible raw-document-to-prepared-corpus processing (done 2026-07-18)

M23 closes M22's explicit upstream boundary for one deliberately narrow input:
already extracted, exact UTF-8 plain-text document bytes. A fresh context-v6
evaluation consumes a signed two-entry corpus package from the tenant-scoped
local CAS, reproduces its prepared chunks with a pinned mechanical preparer,
then continues through the unchanged M22 prepared-corpus-to-evaluation-facts
derivation. The signed `corpus.json` is accepted only when its object, canonical
bytes, length, and SHA-256 address exactly match the independently reproduced
output. The current contract is recorded in the
[M23 decision](decisions/decision_m23_reproducible_raw_document_prepared_corpus_processing.md),
[checked plan](examples/m23-preparation-plan.json), and
[`fixtures/m23/` authoring guide](../fixtures/m23/).

- [x] Closed consumption plan v3 with nested preparation plan v1 and the
  unchanged M22 derivation plan v1. The context preprocessing digest must equal
  the preparation-plan digest.
- [x] Exact `cognigraph.reproducible-prepared-chunk-corpus.v1` package with two
  sorted, non-executable `application/json` entries: canonical `corpus.json`
  and canonical `documents.json`.
- [x] Closed `documents.json` schema binding space, corpus revision, plan, and
  rows sorted by unique non-blank NFC/control-free id. Every row binds an id
  and NFC/control-free title of at most 1,024 UTF-8 bytes, exact
  `text/plain; charset=utf-8` media type, byte length, SHA-256 digest, and
  canonical unpadded base64url payload.
- [x] Pinned strict-UTF-8 preparation with Unicode 17.0.0 NFC and whitespace
  tables: strip exactly one leading BOM when present; reject unsupported
  controls; normalize CRLF/bare CR to LF and Unicode to NFC; enforce the
  per-document normalized-byte cap before whitespace collapse; trim Unicode
  whitespace from line edges; collapse interior whitespace runs; join adjacent
  non-blank lines; use blank lines as paragraph boundaries; reject a document
  with no non-blank paragraph; and enforce the aggregate normalized-byte cap
  after full collapse. Greedily pack without overlap, splitting an overlong
  paragraph after the latest fitting `.`, `!`, or `?` only when followed by
  whitespace or paragraph end, then at whitespace, then a UTF-8 boundary.
  Chunk ids bind the full lowercase SHA-256 of the NFC document-id bytes plus a
  zero-based eight-digit ordinal, and output rows are sorted.
- [x] Bounded preparation: at most 96 MiB of `documents.json`, 100,000
  documents, 4 MiB per raw document and 64 MiB total raw bytes, 8 MiB per
  post-newline/NFC document before whitespace collapse and 64 MiB total
  prepared-normalized bytes after full collapse, 8 KiB per chunk, 100,000
  chunks, and 48 MiB total prepared text. `corpus.json` retains M22's 64 MiB
  cap.
- [x] Exact prepared-corpus reproduction before M22 candidate resolution,
  grounding, graph reconstruction, and scoring. A normalized-equivalent but
  byte-different raw package remains a different signed corpus address.
- [x] Job v4, consumption receipt v3, derivation receipt v2, preparation receipt
  v1, context/evidence/decision v6, head v4, and signed
  `cognigraph.promotion-intent.v4` with explicit preparation-authority binding.
  Restart recovery, reconciliation, and Native stored-plus-incoming snapshot
  validation carry the same address chain.
- [x] Historical M18-M22 compatibility. M22 continues to mean “already prepared
  chunks to evaluation facts”; it never gains retroactive raw-document proof,
  and normal M23 adoption requires a fresh target channel.
- [x] Authenticated release-binary persistent-Native and configured live-ArangoDB
  lifecycle verification. Native passed in 14.46 seconds; live ArangoDB
  Enterprise 3.12.9-1 passed in 109.22 seconds, removed all 36 probe records,
  and restored zero pre-existing records. Both covered restart/recovery,
  signed inconsistent prepared-output rejection, `documents.json` CAS tamper,
  prospective revocation/history fencing, and isolated cleanup.

M23 is address-durable, not byte-self-contained. Compact receipts and Native
snapshots bind the signed manifest plus exact `documents.json` and `corpus.json`
addresses, but do not embed those CAS byte streams. Re-execution therefore
requires separately backed-up and replicated CAS custody; ArangoDB still has no
CogniGraph application snapshot. The input is already extracted UTF-8 text:
PDF/HTML/office/archive parsing, OCR, extraction fidelity, MIME sniffing, and
remote fetching remain outside CogniGraph. The output is an evaluation input,
not materialized tenant document/chunk/entity/mention/index state or a complete
operational graph. M23 does not publish artifacts, deploy a selected head,
execute staged code, switch consumers, distribute the scheduler, or add
replication, quorum, consensus, or HA.

## M22 — Reproducible corpus-to-graph derivation (done 2026-07-18)

M22 closes the specific derivation gap left by M21. A context-v5 evaluation
consumes one canonical prepared-chunk corpus, one canonical construction
candidate, and one claimed canonical evaluation graph from the tenant-scoped
local CAS. The server resolves the candidate, replays the pinned deterministic
Semantic Neurons grounder, and fails closed unless the resulting sorted,
evidence-bearing evaluation-fact projection is byte-for-byte identical to the
attested `graph.json`.

- [x] Closed consumption plan v2 and derivation plan v1, including pinned
  loader/deriver identities, semantics and ABI digests, retention limits, and
  deterministic complexity guards.
- [x] Singleton canonical `corpus.json` in
  `cognigraph.prepared-chunk-corpus.v1`, bound to space, corpus revision, and
  preprocessing digest, with sorted raw chunk identities and collision checks.
- [x] Exact two-entry `cognigraph.reproducible-evaluation-graph.v1` package:
  canonical `candidate.json` plus canonical `graph.json`, both covered by the
  M20 graph attestation.
- [x] Strict construction candidate resolution for the base ontology and
  accepted alias, relation-hint, and relation-blocker neurons; the resolved
  effective-configuration digest must equal the frozen context value.
- [x] Backend-free derivation of sorted unique
  `{source, relation, target, evidence_chunk_id}` rows using the pinned
  negation, sentence-gate, template, accepted-neuron, and veto semantics.
- [x] Exact reconstructed object, canonical-byte, facts-digest, and SHA-256
  content-address equality with the claimed graph before scoring.
- [x] Nested derivation receipt and authority through job v3, context/evidence/
  decision v5, head v3, and signed `cognigraph.promotion-intent.v3`, plus
  restart, recovery, reconciliation, and Native snapshot-union validation.
- [x] Historical M18-M21 compatibility and a mandatory fresh M22 target.
- [x] Exact Rust gates and authenticated release-binary persistent-Native and
  live-ArangoDB lifecycles, with observed evidence recorded in the M22
  decision.

The claim is deliberately narrower than the milestone shorthand. M22 begins
with already prepared chunks; it does not replay raw-document parsing,
normalization, segmentation, or chunking. It reproduces only the canonical
evidence-bearing fact projection used by evaluation, not persistent document,
chunk, entity, mention, index, trigger-span, or storage-key state. A receipt is
created and hashed by the evaluating server and gains governed accountability
when signed promotion intent binds its derivation authority; it is not an
independent attestation, trusted timestamp, or remote host proof. The local CAS
and selected head remain custody and selection boundaries, not deployment,
consumer switching, replication, quorum, or HA.

## M21 — Verified artifact consumption (verified 2026-07-18)

M21 turns the five signed M20 byte-manifest claims into fail-closed evaluation
inputs. An operator stages exact blobs in a read-only tenant-incarnation local
CAS. The server never dereferences a signed location or offers an upload/fetch
surface: it derives paths only from tenant scope and SHA-256, streams every
manifest entry under a byte budget, and records a durable consumption receipt.

- [x] Disabled-by-default `local-cas` source with existing absolute non-symlink
  root, tenant/incarnation isolation, fixed digest paths, and no CAS writes.
- [x] A 10,000-entry aggregate manifest cap, 4,096 unique digest+length-pair
  cap, and verification reuse when a cached read satisfies the next retention
  requirement.
- [x] Fixed 300-second asynchronous operation deadline with one-second durable
  cancellation, tenant-suspension, and shutdown polling; cooperative process
  control, not kernel I/O isolation.
- [x] Pinned context-v4 plan for complete corpus verification, singleton
  `graph.json` and `oracle.json` consumption, and startup-pinned
  `current_exe()` path-content matching for scorer/verifier without a
  mapped-code or verifier-verdict claim.
- [x] Immutable distinct-fact evaluation from the parsed verified graph and
  oracle rather than the live backend; corpus remains verified provenance and
  is not a graph-reconstruction input.
- [x] Durable job-schema-v2 receipt plus evidence/decision v4, signed
  consumption-authority intent, head v2, recovery, and Native snapshot-union
  validation.
- [x] M18-M20 compatibility with historical semantics and a mandatory fresh
  target for M21; singleton selection-only and non-HA boundaries preserved.
- [x] Exact format, Clippy, and full-test gates, with adversarial CAS,
  immutable-input, authority, recovery, and compatibility regressions.
- [x] Authenticated release-binary persistent-Native and live-Arango HTTP
  lifecycles with restart/recovery, tamper, revocation, and exact cleanup.

The claim remains deliberately bounded. The corpus-to-graph derivation is not
replayed, staged binaries match the executable-path digest pinned through
`current_exe()` at startup but are never executed, and this does not prove the
process's loaded code. The receipt is finalized after scoring and binds the
exact job, verified inputs, and canonical result, but its unkeyed hash has no
independent witness or remote-attestation signature. CAS custody, replication,
backup, staging atomicity, deployment, consumer switching, quorum, and HA stay
outside CogniGraph. The exact gates and live evidence are recorded in the M21
decision.

## M20 — Content-addressed external artifact attestations (verified 2026-07-18)

M20 closes the next explicit M19 boundary: corpus, graph, oracle, scorer, and
verifier identities gain signed canonical exact-byte manifests made by a
fourth, purpose-isolated Artifact Attestor principal. Each claim binds the
manifest to its exact evaluation subject and signed location observations.
`PromotionContext` v3 requires all five active bindings, evidence copies the
candidate/baseline sets, and the promoter's signed intent binds their aggregate
artifact-authority digest.

- [x] Closed, bounded, path-safe canonical manifest and five kind-specific
  usage subjects.
- [x] Root-certified Artifact Attestor role/purpose, immutable idempotent
  protected storage, prospective revocation, and list/show/resolve API/CLI.
- [x] Context/evidence/decision v3 bindings, original/replay equality,
  candidate/baseline comparison rules, and four-duty principal separation.
- [x] Full authority recovery and Native stored-plus-incoming snapshot
  preflight for signed records and manifests.
- [x] Exact Rust gates and release-binary Native/live-Arango HTTP lifecycle,
  restart/recovery, tamper, revocation, and cleanup evidence, plus stored full
  v3 promotion and Native snapshot-union regressions.

The trust claim is deliberately narrow. CogniGraph validates what the attestor
signed but never fetches a location, stores the external byte streams, or
independently proves their truth, availability, custody, or freshness. The
evaluator still reads the live tenant graph, so M20 does not prove it consumed
the attested graph bytes. Native snapshots contain authority records rather
than external artifacts; ArangoDB has no CogniGraph application snapshot.
V1/v2 history stays readable and recoverable, authority generations cannot mix
within a target, and normal M20 adoption uses a fresh v3 target.

## M19 — Signed governance and separation of duties (verified 2026-07-18)

M19 closes M18's single-Admin policy trust boundary without changing its
evaluation gates. One externally pinned host-wide Ed25519 root public key
certifies immutable tenant/incarnation-bound keys for exactly three purposes:
policy author, policy approver, and promoter. Product server/CLI private-key
fields are rejected; no accepted service configuration, request, record,
fixture, or snapshot uses or persists private signing material.

One author signs an immutable resolved policy revision. One independently
identified approver signs that exact policy, target, and digest. A
`PromotionContext` v2 freezes the exact root, key-registration, policy, and
approval identities across all four evaluation jobs. A third distinct
principal signs the promote, reject, or rollback intent, including evidence,
gate, policy, approval, reason, idempotency, rollback-target, and expected-head
bindings; the immutable decision stores that signed intent.

- [x] Externally pinned Ed25519 root and root-certified immutable principal
  key registrations.
- [x] Separate HostAdmin tenant-lifecycle bootstrap from tenant Admin trust
  bootstrap/recovery, with neither role receiving the other's scopes.
- [x] Prospective key revocation and rotation that preserve stable principal
  identity and valid pre-revocation history.
- [x] Signed immutable policy revision plus exactly one independent signed
  approval.
- [x] `PromotionContext` v2 exact governance binding and v2-only new promotion
  evidence.
- [x] Signed third-principal promotion intents stored in immutable decisions.
- [x] Full authority-union validation during status, recovery, reconciliation,
  and Native snapshot preflight.
- [x] Adversarial signature/principal/revocation coverage, standard Rust gates,
  and native/live-Arango release verification.

M18 v1 records remain readable and diagnostic but cannot create new evidence
or promote/reject authority. A signed rollback may restore only exact prior v1
evidence already in the decision chain; normal foundation migration uses a
fresh target channel and never retro-signs history. M19 authenticates
governance principals, not the truth of
external corpus, graph, oracle, scorer, verifier, or reproducibility claims.
It does not sign whole snapshots or prove backup freshness. The selected head
remains a single-process control-plane pointer: no automatic deployment, HA,
distributed quorum, or external consumer switching is added.

## M18 — Evaluation promotion gates (verified 2026-07-18)

M18 turns evaluation evidence into a governed selection decision. A succeeded
evaluation job is evidence, not automatic approval. One immutable bundle names
four context-bearing succeeded jobs: candidate original/replay and baseline
original/replay. Its closed v1 policy permits no excluded cases, uses a closed
candidate-difference allowlist, and applies independent denominator, recall,
restraint, regression, exact-replay, comparability, and oracle-separation gates.
The oracle contract has exactly four stages: build, tuning, and construction
cannot read the oracle; evaluation may. Scorer/verifier hashes are pinned
operator-trusted semantics identities, not executable verification.

Attributed idempotent Admin promote/reject/rollback decisions are immutable;
the current `{space_type, channel}` head is a repairable singleton control-plane
selection and never an automatic deployment. Degraded authority fences new
tenant mutations until full Admin recovery validates all authority and repairs
heads. Authenticated-Admin snapshot import cross-checks evidence against
hot/archive jobs but remains an unsigned trusted-root restore boundary. Public
opaque AQL, including Lua AQL, is disabled for every role after Unicode-escaped
identifiers demonstrated that textual screening was not a security boundary.

- [x] Closed context, policy, checked fixture, and four-job evidence model.
- [x] Independent integer gates, exact oracle stages, and comparable baseline.
- [x] Durable decisions, rollback, derived-head reconciliation, mutation fence,
  and full tenant recovery endpoint.
- [x] Snapshot conflict/provenance preflight and protected-query boundary.
- [x] Full Rust gates and release-binary native/live-Arango lifecycle evidence,
  including restart persistence, derived-head fault detection, mutation
  fencing, recovery, and Native snapshot head rebuilding.

Foundation scale limits remain explicit: a target channel accepts at most
10,000 actionable promote/rollback decisions, and full tenant recovery
materializes all evidence/decisions in memory. Streaming tenant-wide authority
validation and a post-limit lifecycle are future scale work. The authored
policy is also an Admin trust-root input in v1; a registry of signed policy
revisions plus a distinct policy-approver role is the next governance boundary
if policy authoring and promotion must be separated.

## M17 — Queue scale and governance (verified 2026-07-17)

M17 scales the M16 control plane without changing its singleton-writer safety
boundary. The default list path is a bounded, filter-bound cursor over a
repairable projected catalog. Global and per-tenant active limits apply
backpressure to new work while preserving idempotent replays. One central
round-robin dispatcher gives each ready tenant a turn, and long ingests yield at
durable batch checkpoints while retaining FIFO order within a tenant.

Terminal jobs become archive-eligible after the configured retention interval.
Archival is copy-first into a protected immutable archive, updates the catalog,
and only then removes the hot record; M17 has no purge operation. Bounded
three-phase reconciliation repairs missing/stale catalog rows, removes catalog
orphans and noncanonical scoped aliases, and resolves only lineage-equivalent
hot/archive duplicates. Quota lowering is linearized with final admission,
suspended jobs remain globally counted, and worker panic cleanup cannot strand
in-process dispatcher occupancy. If its durable failed transition cannot be
written, the job and capacity remain conservatively occupied. Admin status, dry-run
archive/reconcile operations, CLI controls, health degradation, and
fixed-cardinality metrics make those states visible.

- [x] **M17.1 Cursor catalog** — backend after-key scan plus tenant/filter-bound
  cursor and projected summaries; explicit capped offset compatibility only.
- [x] **M17.2 Capacity governance** — host total/per-tenant limits,
  `quotas.max_active_jobs`, HTTP 429, and `Retry-After: 1`.
- [x] **M17.3 Fair scheduling** — singleton central dispatcher, tenant
  round-robin, ingest checkpoint yielding, and intra-tenant FIFO.
- [x] **M17.4 Retention and repair** — immutable archive, no purge, bounded
  dry-run/apply archive, and catalog reconciliation.
- [x] **M17.5 Operator surface** — API/CLI status, metrics, health, and tenant
  quota administration.
- [x] **M17.6 Verification** — full Rust gates plus release-native and live
  ArangoDB lifecycle/restart probes, with evidence recorded in the decision.

M17 still does not add leases, shared-volume redb replicas, multi-process job
claims, distributed scheduling, or HA. See
`decisions/decision_m17_queue_scale_governance.md`.

## M16 — Durable governed operations (done 2026-07-17)

M16 makes large governed operations inspectable and restart-safe without
pretending CogniGraph is a distributed system. It adds persistent,
tenant-scoped, idempotent jobs for `construct.ingest` and
`construct.evaluate`; a single in-process worker; at-least-once recovery from
durable batch checkpoints; cooperative cancel and resume/restart retry; an
attributed transition history; API/CLI lifecycle controls; and aggregate
fixed-label metrics. The qualified `_cognigraph_jobs` collection remains
protected system state in each tenant store, so tenant isolation, snapshot
ownership, and delete quarantine follow the existing storage boundary.

The single-worker/singleton-writer constraint is deliberate. M16 does not add
shared-redb replicas, a distributed lease, priorities, cron, or HA. Persistent
native storage is required for restart recovery; in-memory mode can exercise
the API but cannot be durable. See
`decisions/decision_m16_durable_governed_operations.md` for the contract and
non-goals.

- [x] **M16.1 Contract** — freeze job kinds, idempotency, tenant incarnation,
  recovery, lifecycle, authorization, audit, metrics, and non-goals.
- [x] **M16.2 Durable model** — one tenant-local `_cognigraph_jobs` document
  embeds frozen execution input, current state, input digest, and append-only
  transition history so a state/event change is one replacement on every
  backend. The qualified name avoids Arango's built-in `_jobs` collection.
- [x] **M16.3 Worker and recovery** — one fair permit, atomic ingest batch
  checkpoints, graceful-stop fencing, and startup recovery of queued or
  interrupted work.
- [x] **M16.4 Lifecycle surface** — submit/list/status/cancel/retry over HTTP
  and the admin CLI, with owner-or-Admin mutations and cross-tenant 404s.
- [x] **M16.5 Operations** — cooperative tenant lifecycle/import fencing,
  bounded-cardinality metrics, and persistent backup semantics.
- [x] **M16.6 Verification** — full Rust gates and release-binary checks for
  idempotent replay/conflict, isolation, cancel/retry, audit/result persistence,
  and a forced-stop/restart 5,000-chunk ingest with 5,000 unique occurrences.
  Database-scoped Arango credentials additionally proved durable evaluation and
  projected list reads, plus fail-before-write ingestion; probe state was
  removed afterward.

The observed commands, state transitions, and cleanup are recorded in
`decisions/decision_m16_durable_governed_operations.md`.

## M15 — Trustworthy foundation (2026-07-17)

M15 closed the trust gaps discovered after the original arcs: it initially
narrowed opaque backend queries to Admin and completed Lua mutation gating;
M18's Unicode-escape finding supersedes that interim boundary by disabling
public raw AQL and Lua AQL for every role. Other M15 work includes system-collection
guards at storage and traversal boundaries; atomic occurrence-based
construction revisions with collision checks; unbounded-scan and traversal
score parity; fresh weak-cache fusion, dependency-safe invalidation, and
deadlock-free cache indexing; and readiness/cache telemetry that describes the
active backend and tenant. The historical 312-test checkpoint above remains a
dated checkpoint, not the current repository count. Detailed constraints and
migration notes are in `decisions/decision_m15_foundation.md`.

---

## Arc A — CGQL v2

- [x] **A1. Design discussion: joins, subqueries, multiple FOR** — decided 2026-07-04 (decision_cgql_v2.md) (the
  mutations-style session — user decides contested semantics). Questions to
  settle: nested FOR as cross-product + filter vs explicit join syntax;
  subqueries in LET (`LET x = (FOR ... RETURN ...)`) and their budget
  semantics; correlated access rules; how pushdown analysis composes when
  more than one source exists; what stays explicitly out (v2 non-goals).
- [x] **A2. Implementation milestones** — landed 2026-07-04, all three at once (the pipeline restructure made them one change) (each: grammar → AST → validation →
  planner → BOTH executors → corpus cases that must agree byte-identically):
  - [x] A2a. LET subqueries (uncorrelated + correlated)
  - [x] A2b. nested FOR / joins / array iteration
  - [x] A2c. correlated subqueries (per-row, shared budget)
- [x] **A3. Interpreter performance arc, phase 1** — landed 2026-07-04:
  the profile was allocation, not interpretation. Leaf-only identifier
  clones + sort-by-move: flagship scan 2.5x (0.85 ms), COLLECT 1.8x,
  DISTINCT 1.4x (benchmarks.md). Slot environments / expression
  pre-compilation DEFERRED with recorded trigger (aggregate-heavy
  workloads or Env >30% of profile) — post-fix overhead ~0.4-0.5 µs/row
  no longer justifies restructuring the executor again.
- [x] **A4. EXPLAIN ANALYZE** — landed 2026-07-05: executes the query
  (binds required, budgets apply; mutations rejected) and reports per-stage
  rows/runs + per-source rows/fetch_ms + totals. Row counts engine-
  comparable, timings explicitly not; instrumentation is free when not
  analyzing (per-stage branch, bench-verified).
- [x] **A5. Pushdown extensions** — landed 2026-07-05: IN-predicate
  pushdown (PredicateOp::In, contract-covered), VECTOR_SEARCH threshold
  from `_score >=` conjuncts, traversal min_confidence for exact depth
  1..1 (deeper pruning changes semantics — documented). model_name
  pushdown deliberately deferred: it scopes the search rather than
  filtering it; needs explicit syntax. OR→IN rewrite skipped (IN covers
  the need). **Arc A complete.**

## Arc B — Semantic Neurons (the differentiator)

**Status: complete (8 of 8), including the B1 follow-up: gap-bearing
authored-fixture runs (2026-07-06) — ontologies mechanically stripped of all
triggers. Under current distinct-fact semantics, the loop recovered 54/59
(92%) from 0/59 with 0/32 violations;
every unfilled gap a refusal (verbatim self-check or negation-aware
grounding), not a fabrication.**


Positioning note: provenance of the research is described at the domain
level only — digital forensics and pharmaceutical research, as the fixture
spaces already do. The framing lineage (neural self-repair through
rewiring along alternative paths) may be referenced generically.

- [x] **B1. Session-separated authored fixture evaluation** — run 2026-07-06
  on four assistant-authored, author-curated public-source kits assembled
  outside the implementation session. These are not an independent external
  evaluation. Current distinct-fact score: construction recall 59/59,
  restraint 0/32; historical per-question-mention score: 67/67 and 0/36;
  answer-level restraint 72/72 traps held across two completion models;
  answer recall 32/68 (model-dependent: 24/68 on the older model) — the
  answerer-selectivity bound, replicated. Honest limit: the ontologies
  were complete, so the repair loop never fired — a gap-bearing blind kit
  is the recorded follow-up (see positioning doc). **Arc B complete.**
- [x] **B2. End-to-end answer-quality eval** — landed 2026-07-05:
  `answer_eval` builds the same ranked trace as the graph-augmented route
  per question, has a model answer as a verbatim fact list, and scores
  answer-level recall/restraint. First live baseline (CrowdStrike +
  accepted neurons, gpt-4.1-mini): restraint perfect (0 forbidden incl.
  the Microsoft trap), per-question recall 5/6 — the "missing" fact was
  asserted under a different question (relevance-judgment variance, not
  retrieval; aggregate 6/6 facts asserted). Scorecard runner:
  `cargo run -p cognigraph-construct --example answer_quality`.
- [x] **B3. Violation-directed blocker proposing** — landed 2026-07-05:
  `propose_blockers_report` finds the chunks grounding each violated
  forbidden fact (with the fired trigger), prompts for verbatim veto
  phrases marking the illegitimate context, self-checks phrases
  symbolically, and SIMULATES coverage per proposal (suppressed vs
  uncovered chunks shown to the reviewer). Same skip/retry machinery;
  review stays human.
- [x] **B4. Proposer evidence retrieval via BM25** — landed 2026-07-05:
  `propose_neurons_via_backend` retrieves per-gap evidence with tantivy
  BM25 over the ingested chunks (endpoints + relation words), unioned
  with both-endpoint surface matches as the precision anchor. Pinned test:
  a chunk naming neither endpoint is unreachable by surface matching and
  found by BM25.
- [x] **B5. Validated `/api/neurons` route** — landed 2026-07-05: `POST /api/neurons`
  (ontology-validated, always stored proposed; space types read from a
  `space_types` collection), GET list with space/status filters, and
  explicit accept/reject/retire transitions — accepting re-validates
  against the space's STORED accepted set, so the hint/blocker conflict
  cannot be assembled one edit at a time. graph_facts now survive caching
  (cache entries carry a meta payload; QueryCache::put_results extended).
- [x] **B6. Graduation automation** — landed 2026-07-05: leave-one-out
  `graduation_report` (hint graduates when its fact grounds without it;
  blocker when its forbidden fact no longer grounds even unvetoed),
  `GET /api/neurons/graduation` computed from stored state, and
  `cognigraph neuron graduation SPACE` (plus neuron list/propose/accept/
  reject/retire CLI commands). Research parity pinned: all SOTU neurons
  flag covered_by_base; the CrowdStrike repairs do NOT flag. Flags only —
  retirement stays human.
- [x] **B7. Self-healing experiment** — landed 2026-07-05, THE EXPERIMENT
  HELD (docs/self-healing-experiment.md): a document revision collapsed
  the CrowdStrike space 6/6 → 1/6 (9 dead pathways: 4 neurons + 5 naive
  base rules, detected by the new degradation_report); live gap-directed
  rewiring restored 6/6 with restraint 0/3 throughout; graduation flagged
  the dead originals for pruning — including an organic covered_by_base
  case where the revision drifted back under a naive rule. Deterministic
  full-loop regression test; neurons proven to be the rewiring mechanism
  for dead BASE pathways too (the ontology is immutable to the loop).
- [x] **B8. Paper/positioning refresh** — landed 2026-07-05:
  docs/semantic-neurons/positioning.md — an evidence dossier consolidating
  every Arc B result with its honest caveat and code/test backing, mapped
  to paper concepts, competitive claim held to the defensible form, neural
  framing, abstract seed. Input for the author's paper (the paper itself
  stays in the research repo, unedited); B1's blind-eval result is the
  marked load-bearing pending section.

### WebNLG external-oracle benchmark (recorded July 17–20, 2026)

The [pilot guide](webnlg-pilot.md) and
[dated scoring ledger](decisions/decision_webnlg_scoring.md) describe an
entity-provided relation-construction benchmark over public data-to-text pairs.
Its reference triples make scoring mechanical; this is not a full text-to-graph
or runtime governed-review result.

- [x] July 17: prepared corpus, W1–W7 scoring policy, and supervised mining from
  train+validation. The [frozen mined ruleset](../crates/cognigraph-construct/fixtures/webnlg/neuron-ruleset.mined.json)
  contains 303 predicates and 1,842 templates. Validation informed tuning;
  its exact score was 12.4% recall / 76.6% precision.
- [x] July 17: one-shot test scored **7.0% recall / 68.8% precision**
  (393 correct / 571 constructed; 5,639 oracle triples). Generic scored
  0.4% / 52.6%; oracle-diagnostic scored 7.0% / 90.1%. This remains the
  frozen test result; low recall is an observed limit.
- [x] July 20: fuzzy matching tried and rejected on validation. Even gap zero
  gave 14.5% recall / 40.9% precision; wider gaps lost more precision. Exact
  matching remains the default; the fuzzy implementation is experimental.
- [x] July 20: deterministic `CorpusProposer` added 9,715 attributed examples
  and expanded 292 to 308 predicates, with 12.2% recall / 78.2% precision.
  More examples did not improve recall in this experiment.
- [x] July 20: live `gpt-5.4-mini` proposals, using train examples for the
  top-25 predicates selected by validation frequency. The
  [raw snapshot](../crates/cognigraph-construct/fixtures/webnlg/llm-proposals.json)
  scored 13.0% recall / 53.2% precision; automatic disambiguation reached
  12.6% / 71.7%. These are development-set measurements.
- [x] July 20: offline pruning removed 46 entries, leaving 202 in the
  [pruned snapshot](../crates/cognigraph-construct/fixtures/webnlg/llm-proposals-pruned.json).
  Merged and disambiguated, it scored 12.6% recall / 76.3% precision:
  609 correct versus 601 baseline. That is a small recall gain with a slight
  precision loss, not a strict improvement on both metrics. The ledger's
  “human prune/full loop” label does not establish runtime review events or
  reviewer attestations, which are absent from these candidate files.

The frozen artifact was not replaced by either proposal snapshot. Generalization
remains unestablished. Entity-discovered scoring, hard-negative restraint,
runtime governed-review evaluation, and a newly reserved evaluation set remain
open; the consumed test oracle is not available for iterative tuning.

## Hygiene (interleave anytime — small, high leverage)

- [x] **H1. CI pipeline** — landed 2026-07-04 (.github/workflows/ci.yml) — GitHub Actions running the exact local gates
  (clippy -D warnings, cargo test --all, fmt --check; live-LLM tests stay
  opt-in via COGNIGRAPH_LIVE_LLM); optionally the Docker build. The gates
  survive any session or contributor. Do this early in Arc A.
- [x] **H2. Release workflow** — v2.0.0 tagged 2026-07-05 (the second
  major iteration, first tagged release; user set the version). Full
  staged pipeline: PR triage (bot PRs deferred), lockfile refresh,
  cargo audit clean, all gates, atomic bump-commit-tag on main.
- [x] **H3. OpenAPI drift test** — landed 2026-07-05
  (src/openapi_drift.rs): source-derived route extraction (every .nest
  prefix + every .route literal) checked BOTH WAYS against the spec's
  paths, with the nest→module mapping asserted complete against main.rs
  and a vacuity floor so the extractor can't silently rot. First run
  found zero drift — the hand-maintained spec was already exact.
- [x] **H4. Persistent-mode load benchmark** — landed 2026-07-05: the
  load bench now sweeps BOTH modes plus pure-write and 100-doc-batch
  workloads. Reads unaffected by persistence; independent single-doc
  writes ceiling at ~255/s (serialized ~4 ms redb commits, flat across
  concurrency); `/api/batch` already amortizes to ~13.8k docs/s (measured
  ~54×, one transaction per batch). Server-side group commit recorded
  as a design-discussion trigger, not taken (benchmarks.md).
- [x] **H5. Env naming decision** — decided AND executed 2026-07-05
  (decision_env_naming.md): swept every CogniGraph-owned knob to
  `COGNIGRAPH_*` (query cache disambiguated to
  `COGNIGRAPH_QUERY_CACHE_*`); ecosystem-standard third-party names stay
  bare; no fallback reads — legacy names warn loudly at startup instead.
  The pre-2.0.0 breaking window was the reason to sweep now.
- [x] **H6. Token hygiene** — landed 2026-07-06
  (decision_token_hygiene.md): opt-in expiry (`COGNIGRAPH_TOKEN_TTL_SECS`
  default + per-token `expires_in_secs`, 0 = never), expired tokens fail
  like revoked ones but stay listed with `expired: true` for audit,
  in-place rotation (same record/key, fresh secret + window) via route
  and CLI. `last_used_at` and auto-purge deliberately not taken
  (recorded: write-per-request vs the H4 fsync ceiling; audit trail).
- [x] **H7. Cache backends** — landed 2026-07-06
  (decision_cache_backends.md): `PersistentCache` persists the EMBEDDING
  level on redb (deterministic, worth keeping across restarts:
  read-through 16 µs vs a provider round-trip) while results stay
  memory-only (TTL-bounded, stale after restart). redb over LMDB (stack
  already ships it); Redis deferred with trigger (multi-instance
  deployment). `COGNIGRAPH_QUERY_CACHE_BACKEND=persistent` +
  `COGNIGRAPH_QUERY_CACHE_PATH`.
- [x] **H8. Batch embedding pipeline** — landed 2026-07-06
  (decision_batch_embedding.md): `POST /api/documents/embed` + `cognigraph
  embed` — provider-batched embedding (64/request, 4 in flight) into ONE
  atomic store transaction; all-or-nothing with validation before the
  first provider call. Chunking stays upstream (cognigraph-chunker).
  Fixed in passing: search routes now resolve self-contained hits (no
  `document_id` pointer) to their own `_id` instead of returning
  `document: null` / dropping them from fusion. **Hygiene arc complete
  (H1–H8).**

## Quick wins (post-v2.1.0)

Tracked in [quick-wins-2026-07.md](quick-wins-2026-07.md): reviewer
attribution on neuron transitions, /api/construct/evaluate + /api/construct/
propose (HTTP-complete loop), CLI review ergonomics, trigger span
provenance, a hostile-scale eval, and two paper cuts. As of 2026-07-06,
only the conditional QW7 Aho-Corasick optimization remains open; the
hostile-scale eval found a restraint-quality problem rather than a
matching-cost problem.

## Strategic (later; each is a phase, not a task)

- [x] Multi-tenancy — landed 2026-07-07, live-verified over HTTP
  2026-07-13 (decision_multi_tenancy.md): backend-store-per-tenant,
  control store for auth, host-admin role, per-tenant export, routed
  facades as the single structural choke point. Deferred with triggers:
  quota enforcement, persistent query cache in MT mode, per-tenant
  metrics labels.
- [ ] HA/replication (snapshot shipping first, consensus much later)
- [ ] Client SDKs (TypeScript first) and a docs site
- [ ] Vector search tail-latency lever (per-query rayon cap) if p99 under
  concurrent vector load ever matters for the product
