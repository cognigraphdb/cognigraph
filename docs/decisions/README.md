# Decision Log

This index identifies the operative scope of every decision record. Reviewed
2026-09-09 for [CG-30](../issues/CG-30.md). The
[implementation plan](../implementation-plan.md#review-checkpoint--2026-09-09)
owns delivery status and the [issue registry](../issues/README.md) owns remaining
defects. A dated milestone's verification applies to that checkpoint; it does
not certify later changes or close subsequent issues.

## Reading status and amendments

- **Active**: the named convention or contract remains applicable within its
  stated scope. This does not assert that every implementation is defect-free.
- **Amended**: retain the record's unaffected decisions, but use the linked
  outcome, addendum, or successor for the part that changed.
- **Research**: experiment design, results, and resulting hypotheses, bounded by
  the recorded corpus, models, and scoring policy. These are not general product
  guarantees or signed promotion evidence.
- **Historical**: the replaced contract is retained for provenance. Follow its
  successor before using it operationally.

“Extends” means additive scope; “replaces” names an actual supersession. A later
milestone does not automatically replace an entire earlier record. In particular,
M18–M26 preserve older authority generations with their original meanings; M19
removes M18 v1 from new promotion authority, and M26 deployment is a separate act
from promotion selection. Historical model names, test counts, and benchmark
figures are observations from their dates, not current defaults.

The [Native-only decision](decision_native_only.md) amends every earlier
backend/storage choice: both editions use Native, and public queries use parsed
CGQL. Older adapter configuration and live-service measurements are historical.
M15–M26 authority schemas, signatures, custody and recovery contracts remain
applicable within their stated scopes; use current operator guides for setup.

## Current owner documents

The owner here is the document to consult for current behavior, rather than a
person assigned to maintain it. Decisions below supply rationale and scoped
amendments. The HTTP schema owns wire shapes; operator guides explain use and
limitations.

| Subsystem | Current owner / entry point | Boundary to retain |
|---|---|---|
| Delivery and outstanding work | [Implementation plan](../implementation-plan.md), [issues](../issues/README.md) | Historical roadmaps and experiment next steps are not the active ticket queue. |
| Backend and storage | [Native-only decision](decision_native_only.md), [Native storage model](../architecture/native-storage.md), [architecture](../architecture/components.md#core-traits) | CG-65/CG-67 complete; Native is the only runtime backend. CG-68 qualifies local readiness; publication and target-environment deployment remain separate. CG-64/CG-66 are optional deferred work. |
| CGQL and Lua | [CGQL specification](../reference/cgql.md), [Lua reference](../examples/lua-scripting.md) | The version-neutral specification covers delivered v2. Keep `graph.query()` as the entry point and honor current mutation/subquery limits. |
| Retrieval, embeddings, and cache | [Retrieval guide](../dataops/03-retrieval.md), [cache architecture](../architecture/cache.md#semantic-query-cache) | Model identity, request parameters, tenant isolation, and side-view provenance affect results. |
| Identity and legacy references | [Exact identity](decision_exact_reference_identity.md), [repair procedure](../operations/reference-repair.md) | Preserve opaque strings; construction evidence has its own explicit canonicalization boundary. |
| Authentication and tenancy | [Auth runbook](../operations/authentication.md#auth-bootstrap), [tenancy runbook](../operations/authentication.md#multi-tenancy-decision_multi_tenancymd) | Host lifecycle authority does not grant tenant data access; retirement and request incarnation are separate contracts. |
| Construction, drafting, and review | [Semantic Neurons guide](../dataops/04-semantic-neurons.md), [review policy](decision_review_policy.md#current-contract-addendum--m25-fail-closed-narrowing-2026-07-19) | Legacy authoring/review differs from signed Semantic Repair authority. |
| Durable jobs and queue operations | [Jobs runbook](../operations/jobs.md#durable-governed-jobs-m16-m17) | Four job kinds now exist; the original two-kind M16 list is historical. Single writer and in-process dispatcher remain. |
| Promotion, signed repair, and deployment | [Governance runbook](../operations/governance.md#signed-evaluation-semantic-repair-authority-and-deployment-m18-m26) | Read M18–M26 below as a versioned chain; selection, materialization, and signed deployment are distinct. |
| Artifact custody and restore | [M24 runbook](../operations/governance/artifact-custody.md#m24-artifact-custody-and-verified-restoration) | Database records and CAS bytes require separate recovery; an operation receipt does not prove continuing custody. |
| Provider selection and model defaults | [Side-view configuration](../operations/jobs.md#side-view-generation-and-configuration), [current model policy](decision_sideviews_provider_and_benchmark.md#current-model-defaults-2026-09-08) | Luna remains the economical baseline. The original ticket list is closed; the [Luna baseline](decision_luna_baseline.md) covers the synthetic comparisons, independent-label GLM Flash–Luna trial, and Terra/Luna low extension. The user selected Luna low, now adopted in the shared provider with [Rust and live HTTP verification](../issues/luna-low-runtime-2026-09-09.md). The first document trial's [CG-39](../issues/CG-39.md) and [CG-40](../issues/CG-40.md) are resolved by [policy v2](decision_directed_extraction_contracts.md), with a separately frozen development replay/live pass. Complete independent reference review before using the unrun holdout. DeepSeek V4 Flash is retired; judge qualification is unchanged. |
| HTTP, CLI, and operator console integration | [OpenAPI](../../crates/cognigraph-server/openapi.yaml), [API examples](../examples/api-usage.md), [CLI runbook](../operations/cli.md#administration-cli) | The UI decision is indexed below for history; CG-30 does not re-audit the React implementation. |
| Research and evaluation | [Positioning dossier](../../../docs/research/semantic-neurons/positioning.md), [WebNLG guide](../research/webnlg/pilot.md), [DailyMed guide](../research/dailymed/pilot.md) | Keep distinct construction facts separate from answer mentions; [CUAD scoring recovery](../issues/cuad-recovery-2026-09-09.md) corrects mixed units, while [CG-25](../issues/CG-25.md) is explicitly closed with missing execution provenance; the [Luna baseline](decision_luna_baseline.md) owns the new evaluation direction. |
| Engineering conventions | [AGENTS.md](../../AGENTS.md), standing conventions below | Repository instructions own the current workflow; the September 9 [CG-26](../issues/CG-26.md) refactor resolves the six reviewed hotspots; wider workspace modularity remains separate. |

## Standing conventions

| Decision | Status | Operative scope | Successor / amendment |
|---|---|---|---|
| [Container vulnerability gate](decision_container_vulnerability_gate.md) | Active | Both edition images and CI; refresh installed packages and block all fixable CVEs. | [CG-80](../issues/CG-80.md) owns remediation; [CG-81](../issues/CG-81.md) owns unfixed findings. |
| [Develop qualification](decision_develop_qualification.md) | Active | Incoming work, dependency freshness and available CVE fixes before integration. | [CG-82](../issues/CG-82.md) owns enforcement; [CG-83](../issues/CG-83.md) owns the initial refresh. |
| [Public evidence boundary](decision_public_evidence_boundary.md) | Active | Public engineering records and CI inputs; private captured research, audits and operational inventories. | [Evidence policy](../operations/evidence-policy.md) owns enforcement; CG-75/CG-77 own migration and qualification. |
| [Hosted QA lifecycle](decision_hosted_qa_lifecycle.md) | Active | On-demand Railway QA, separate application instances, retained volume and manual deployment. | [CG-76](../issues/CG-76.md) records shutdown and removal of the unfinished permanent ingress setup. |
| [Dependency advisories](decision_dependency_advisories.md) | Amended | Bounded Tantivy patch, visible time-bounded optional ONNX maintenance exception, and fatal unsoundness advisories. | [CG-70](../issues/CG-70.md) owns the initial inventory; the [refresh decision](decision_dependency_refresh_2026_09_14.md) updates package versions. |
| [Dependency refresh 2026-09-14](decision_dependency_refresh_2026_09_14.md) | Active | Current application dependencies with legacy password/store compatibility and optional ONNX qualification. | [CG-83](../issues/CG-83.md) owns acceptance of the v2.7.15 candidate. |
| [Continuous verification](decision_ci_verification.md) | Amended | Shared local/PR checks, required successful aggregate, pinned Actions and separate publication authority. | [CI guide](../operations/ci.md) owns tool setup and repository settings; [CG-69](../issues/CG-69.md) records qualification and the private website plan limit. |
| [Develop integration and Renovate](decision_develop_integration.md) | Active | Develop is the default integration branch; main is the separately promoted production branch. Renovate targets only develop. | [CG-73](../issues/CG-73.md) records transition evidence; [CI guide](../operations/ci.md) owns operating details. |
| [Benchmark before optimizing](decision_benchmarks_first.md) | Active | Require measured before/after rows and retain rejected approaches. | No replacement; measurements live in [benchmarks](../research/benchmarks/native-backend.md). |
| [Custom implementations and library exceptions](decision_custom_vs_library.md) | Active | Prefer understandable custom implementations, with explicit safety and measurement exceptions. | Its Outcome records Tantivy replacing custom BM25; the preference itself remains. |
| [CGQL language design](decision_cgql_design.md) | Amended | AQL/XQuery shape, case rules, null semantics, one query entry point, and spec-first changes. | [CGQL v2](decision_cgql_v2.md) extends grammar; [current spec](../reference/cgql.md) owns delivered semantics. |
| [Dual-engine query corpus](decision_dual_engine_corpus.md) | Active | Shared file-driven execution cases guard executor/backend parity. | No replacement; the original corpus count is historical. |
| [Environment naming](decision_env_naming.md) | Amended | CogniGraph-owned settings use `COGNIGRAPH_*`; ecosystem names remain bare. | Historical records retain old spellings; [configuration reference](../operations/configuration.md#configuration-reference) owns live names. |
| [File modularity](decision_file_modularity.md) | Active | Target 300–400 LOC with a coherence-based soft cap and controlled refactor passes. | [CG-26](../issues/CG-26.md) splits six reviewed server hotspots; the [verification](../issues/server-modularity-2026-09-09.md) records seven coherent exceptions and the remaining wider-workspace scope. |
| [Licensing](decision_licensing.md) | Active | FSL-1.1-Apache-2.0 for Community, including planned read replicas and manual standby; Enterprise for governance, construction, artifacts, multi-tenancy, sharding, automatic failover and multiple writers. | [CG-45](../issues/CG-45.md) adds the `enterprise` build gate; [LICENSING.md](../../LICENSING.md) is the plain-language map. |

## Platform contracts

| Decision | Status | Operative scope | Successor / amendment |
|---|---|---|---|
| [Native-first direction](decision_native_first.md) | Amended | Native/CGQL direction and no new third-party database backends remain; the original maintenance outcome is historical. | [Native-only](decision_native_only.md) replaces indefinite Arango maintenance; [M18](decision_m18_evaluation_promotion_gates.md) still closes public opaque query access. |
| [Native-only storage before first deployment](decision_native_only.md) | Active | Native-only runtime implemented and locally verified by CG-65/CG-67; CG-68 local readiness verified, importer optional and deferred. | Replaces indefinite Arango maintenance; [batch plan](../plans/native-only-2026-09-12.md) owns fresh Native readiness, without customer cutover or mandatory major-version jump. |
| [Graph-augmented default edge collection and inert rank-hint warning](decision_graph_augmented_edge_collection.md) | Active | `document_relations` stays the default in both editions; Enterprise responses warn with `inert_rank_hints` when accepted rank hints cannot affect the traversed collection; warnings are never cached. | Traversing `facts` and `document_relations` in one request is a possible later contract (CG-89). |
| [Rebuildable derivatives](decision_rebuildable_derivatives.md) | Amended | Durable documents are authoritative; vector and text indexes are derivatives. | [Storage model](../architecture/native-storage.md) distinguishes resident memory-primary from paged redb-primary bodies; [derivative identity](decision_native_derivative_identity.md) strengthens reuse. |
| [Native derivative identity](decision_native_derivative_identity.md) | Amended | Schema 2 binds reuse to database and committed-revision UUIDs; retirement quarantines derivatives. | [CG-35 collection ensures](../architecture/native-storage.md#collection-ensure-idempotency-cg-35) preserve the revision on no-op creates; [tenant deletion](decision_tenant_deletion.md) includes request pinning. |
| [Vector sidecar](decision_vector_sidecar.md) | Amended | Opt-in mmap int8 search vectors with rebuildable base/delta state. | Its CG-34 addendum makes model filtering precede truncation; [CG-35](../issues/collection-ensure-2026-09-09.md) resolves the startup invalidation called open in that addendum. |
| [Arango vector candidates](decision_arango_vector_candidates.md) | Historical | Records the removed adapter's model-identity filtering and distinct-parent result behavior. | [Native-only](decision_native_only.md) removes the runtime adapter in CG-67; retain historical evidence. The Native counterpart [CG-34](../issues/CG-34.md) remains applicable. |
| [Traversal confidence](decision_traversal_confidence.md) | Amended | Inclusive per-edge filtering and shared default-confidence/depth-zero behavior across Native modes. | [Native-only](decision_native_only.md) retires the second adapter; traversal semantics and historical measurements remain. |
| [Exact reference identity](decision_exact_reference_identity.md) | Active | Preserve accepted keys, references, model names, JSON strings, and CGQL literals exactly. | Replaces generic implicit NFC in [Unicode history](decision_unicode_semantics.md); preserves explicit CG-5 evidence normalization and locale collation. |
| [Cache backends and identity](decision_cache_backends.md) | Amended | Persist embeddings on redb; results remain in memory and scoped by exact parameters. | Same-file July 16 and CG-31/CG-32 addenda replace ambiguous parameter encodings; [tenancy](decision_multi_tenancy.md) qualifies persistence in multi-tenant mode. |
| [Batch embedding](decision_batch_embedding.md) | Active | Validate before provider spend, bounded provider batches, atomic storage, chunking upstream. | No replacement; [OpenAPI](../../crates/cognigraph-server/openapi.yaml) owns request shape. |
| [Auth through GraphBackend](decision_auth_through_trait.md) | Amended | Shared graph-backed users/tokens, hashed credentials, and role/scope enforcement. | Its JWT Outcome extends sessions; [tenancy](decision_multi_tenancy.md) relocates control data, [token hygiene](decision_token_hygiene.md) adds expiry/rotation, and [system collections](decision_system_collections.md) fences public access. |
| [API-token hygiene](decision_token_hygiene.md) | Active | Explicit expiry and atomic same-key rotation; stored tokens remain hash-only. | No replacement; use the [auth runbook](../operations/authentication.md#auth-bootstrap) for operations. |
| [Multi-tenancy](decision_multi_tenancy.md) | Amended | One store/cache per tenant, separate control store, tenant-scoped data roles. | Its Outcome replaces the D6 route/migration sketch and records deferrals; [deletion](decision_tenant_deletion.md) replaces credential preservation on retirement and adds CG-12 pinning. |
| [Tenant deletion](decision_tenant_deletion.md) | Amended | Delete credentials, retire the store, quarantine files, and isolate recreated incarnations. | Same-file CG-11/CG-12 updates define derivative quarantine and admitted-request behavior; CG-51 aligns console disclosure and returned outcomes. |
| [System collection protection](decision_system_collections.md) | Amended | Public document/query/Lua/traversal surfaces cannot expose protected control data. | Its M18 addendum replaces M15 Admin-only AQL access with denial for every role; [M25](decision_m25_governed_semantic_repair_authority.md) additionally protects semantic authority and derived collections. |
| [Mutation access](decision_mutations_access.md) | Amended | Separate read/write query authority, explicit mutation enablement, and backend capability boundaries. | Its Outcome closes UPSERT/batch deferrals and lifts Lua after RBAC; [environment naming](decision_env_naming.md) prefixes the switch; [v2 CG-7 amendment](decision_cgql_v2.md#mutation-backend-read-restriction-cg-7-2026-09-08) rejects unsupported mutation reads. |
| [CGQL v2](decision_cgql_v2.md) | Amended | Joins, subqueries, correlated execution, and positional semantics within current limits. | July 21 replaces LET-only expressions; CG-28/37/38 and CG-7/8/15/16 addenda own placement, bindings, budgets, lookup failures, and error mapping. [Workload decisions](decision_cgql_v2_workload_gaps.md) extend delivery. |
| [CGQL workload gaps](decision_cgql_v2_workload_gaps.md) | Amended | D1–D12 delivered; initial failure analysis and performance rows remain historical evidence. | Its current-outcome/CG-23 note replaces the old missing-feature backlog; [issue registry](../issues/README.md) owns current defects. |
| [Management UI integration](decision_management_ui.md) | Amended | Operator console uses existing server APIs; keep conditional capabilities explicit. | Its dated addenda cover namespace/login/navigation, raw JSON/import identity safeguards (CG-50/CG-62), production origin/authentication gating (CG-49), deletion disclosure (CG-51), tenant/user provisioning (CG-52), and verified edition/role capabilities (CG-53). [OpenAPI](../../crates/cognigraph-server/openapi.yaml) owns server contracts; UI code review is outside CG-30. |

## Construction and review contracts

| Decision | Status | Operative scope | Successor / amendment |
|---|---|---|---|
| [Directed extraction contracts](decision_directed_extraction_contracts.md) | Active | Policy v2 uses complete endpoint tokens and exact request-derived chunk/relation enums; local gates remain authoritative. | Amendments for CG-39/CG-40 preserve quote offsets, opaque IDs, malformed-output handling and the frozen v1 benchmark history. |
| [Semantic Neurons port](decision_semantic_neurons.md) | Amended | Evidence-grounded construction and inert proposals over deliberate ontology vocabulary. | [Grounding gates](decision_grounding_gates.md) and [blockers](decision_relation_blocker.md) extend grounding; [M25](decision_m25_governed_semantic_repair_authority.md) adds signed authority, [M26](decision_m26_verified_semantic_repair_materialization.md) adds separate deployment. |
| [Grounding gates](decision_grounding_gates.md) | Active | Opt-in sentence endpoint gates, templates, and distinct-fact evaluation. | Its advisor addendum adds read-only diagnostics; [CG-24](../issues/paper-counting-units-2026-09-09.md) corrects research summaries without changing these units. |
| [Cross-chunk grounding rejection](decision_cross_chunk_grounding.md) | Active | Keep chunk-local evidence; reject window/lookback semantics on the measured evidence. | No reversal recorded; D5 states the bounded pilot evidence needed to reconsider. |
| [Relation blockers](decision_relation_blocker.md) | Active | Chunk-local extraction veto without a precedence engine; blockers require human review. | Its coverage-guided iteration addendum expands proposal search, not acceptance authority; [review policy](decision_review_policy.md) owns lifecycle constraints. |
| [Ontology drafter](decision_ontology_drafter.md) | Amended | Quarantined drafts, deliberate acceptance, and a vocabulary-bootstrap scope. | [Real-label construction](decision_neurons_real_label_construction.md) records per-document drafting/identity corrections; [clinical pilot](decision_pilot_clinical_graph.md) records administrative-relation filtering; [M25](decision_m25_governed_semantic_repair_authority.md) adds authority boundaries. |
| [Review policy](decision_review_policy.md) | Amended | Current M25 addendum allows automatic acceptance only for `relation_hint`; strict parsing and legacy qualification limits apply. | CG-21 adds lifecycle serialization/stale-result checks; CG-36 adds shared dedicated-judge endpoints. Earlier alias acceptance and mutable qualification claims are historical, not signed authority. |
| [Agreement lane A+](decision_agreement_lane.md) | Amended | Adds concordant two-judge review for eligible hints; disagreement queues. | [Current review addenda](decision_review_policy.md) govern authority/concurrency; its own Outcome and CG-36 note restrict server partner configuration to the wired OpenAI provider. Historical model pair measurements do not qualify new models. |
| [Side-view providers and benchmark](decision_sideviews_provider_and_benchmark.md) | Amended | Separate generation provider axis, quarantined retrieval data, durable jobs, fusion, and cascade lifecycle. | Same-file implementation, CG-29 selection, CG-13 lifecycle, and CG-33 identity notes qualify the original design. September model defaults/comparison replace July recommendations; the [directed provider comparison](decision_luna_baseline.md#cross-provider-extension--2026-09-09) is a separate experiment. |

## Governed operations and authority generations

All records below retain their dated verification and limits. Successor links
describe the added boundary, not a retroactive upgrade of old evidence. M24–M26
do not increment promotion context versions.

| Decision | Status | Operative scope | Successor / amendment |
|---|---|---|---|
| [M15 foundation](decision_m15_foundation.md) | Amended | Atomic occurrence provenance, backend/cache parity, and truthful operations. | [M18](decision_m18_evaluation_promotion_gates.md) replaces only the Admin-only opaque-AQL permission; later [review issues](../issues/README.md) record scoped correctness fixes. |
| [M16 durable operations](decision_m16_durable_governed_operations.md) | Amended | Tenant-local immutable job inputs, idempotency, checkpoints, audit, and retry. | [M17](decision_m17_queue_scale_governance.md) extends queue behavior; [current jobs runbook](../operations/jobs.md#durable-governed-jobs-m16-m17) adds directed draft and side-view kinds beyond M16's initial two. |
| [M17 queue scale](decision_m17_queue_scale_governance.md) | Amended | Bounded cursor listing, backpressure, fair scheduling, archival, and repairable catalog. | [M18](decision_m18_evaluation_promotion_gates.md) adds promotion evidence above jobs; it does not add distributed workers or HA. |
| [M18 evaluation gates](decision_m18_evaluation_promotion_gates.md) | Amended | Frozen comparable four-job evidence and explicit selection; historical context v1. | [M19 D11](decision_m19_signed_governance_separation_of_duties.md#d11-keep-m18-v1-history-readable-but-remove-it-from-new-promotion-authority) removes v1 from new promotion authority while preserving diagnostic jobs and exact historical rollback rules. |
| [M19 signed governance](decision_m19_signed_governance_separation_of_duties.md) | Amended | Context v2 adds root-certified keys, independent policy approval, and signed promoter intent. | [M20](decision_m20_content_addressed_external_artifact_attestations.md) adds exact external byte claims on a fresh authority target; signatures alone do not prove artifact truth or consumption. |
| [M20 artifact attestations](decision_m20_content_addressed_external_artifact_attestations.md) | Amended | Context v3 binds signed content-addressed manifests; evaluation still uses the live graph. | [M21](decision_m21_verified_artifact_consumption.md) adds verified byte consumption for a fresh v4 target; M20 claims retain their original meaning. |
| [M21 verified consumption](decision_m21_verified_artifact_consumption.md) | Amended | Context v4 consumes verified graph/oracle bytes from opt-in local CAS and records receipts. | [M22](decision_m22_reproducible_corpus_graph_derivation.md) adds graph derivation; [M24](decision_m24_durable_cas_custody_verified_restoration.md) adds offline custody/restore. Staged binaries are not executed. |
| [M22 corpus-to-graph derivation](decision_m22_reproducible_corpus_graph_derivation.md) | Amended | Context v5 reproduces bounded canonical evaluation facts from prepared chunks and an exact candidate. | [M23](decision_m23_reproducible_raw_document_prepared_corpus_processing.md) adds raw-text preparation; [M26](decision_m26_verified_semantic_repair_materialization.md) can consume M22/M23 selection with M25 authority. It is not a full database reconstruction. |
| [M23 raw-text preparation](decision_m23_reproducible_raw_document_prepared_corpus_processing.md) | Amended | Context v6 adds deterministic exact UTF-8 plain-text preparation before M22 derivation. | [M24](decision_m24_durable_cas_custody_verified_restoration.md) extends recovery; PDF, Office, OCR, and arbitrary container parsing remain outside this contract. |
| [M24 custody and restore](decision_m24_durable_cas_custody_verified_restoration.md) | Amended | Bounded offline CAS bundle/restore with verification; online CAS stays read-only. | No replacement; [M25](decision_m25_governed_semantic_repair_authority.md) adds semantic authority, leaving custody and historical generation meanings intact. |
| [M25 Semantic Repair authority](decision_m25_governed_semantic_repair_authority.md) | Amended | Exact signed candidate revision, independent review, protected collections, and selection-bound governed ingest. | [M26](decision_m26_verified_semantic_repair_materialization.md) adds complete generation/impact and signed deployment; [review policy](decision_review_policy.md) records legacy auto-accept narrowing and CG-21 concurrency. |
| [M26 materialization and deployment](decision_m26_verified_semantic_repair_materialization.md) | Amended | Build a bounded verified occurrence generation, then deploy by separate signature and atomic switch. | Its CG-3 verification addendum fences directed construction against deployed generations; [current operator guide](../operations/governance/deployment.md#m26-verified-semantic-repair-generation-and-deployment) owns the workflow. |

## Research evidence and historical priorities

Experiment successors extend or revise conclusions for their own cohorts. They
do not turn fixture results into evidence of general performance, and M18
explicitly excludes historical research artifacts from promotable evidence.

| Decision | Status | Operative scope | Successor / amendment |
|---|---|---|---|
| [Captured Luna baseline](decision_luna_baseline.md) | Research | Adopt Luna low in the shared provider; retire CUAD; retain synthetic baselines, independent-label GLM Flash–Luna comparison, and Terra/Luna low extension. | Explicitly closes remaining CG-25 recovery without reconstructing history. DeepSeek V4 Flash is dropped from future runs; the SemEval comparison supplies nominal pairs and does not qualify document extraction or a judge. |
| [DailyMed mechanical ontology](decision_dailymed_ontology.md) | Research | Typed, subject-scoped rules and separate structured/narrative provenance. | Replaces earlier gate-1 draft rules; [Gate 3](decision_gate3_baseline.md) records the production admission policy and outcome. |
| [DailyMed Gate 3 baseline](decision_gate3_baseline.md) | Research | Frozen structured-oracle policy and 10,000-label baseline. | [Drift](decision_dailymed_drift.md) extends maintenance measurement; [clinical pass](decision_dailymed_clinical.md) studies relations without structured truth. Perfect admitted precision is by construction, not narrative extraction precision. |
| [DailyMed drift](decision_dailymed_drift.md) | Research | Controlled deltas on real documents plus a limited real-revision HTML follow-up. | Its Outcome qualifies the original historical-oracle plan; no claim of general automatic stored-graph repair. |
| [DailyMed clinical pass](decision_dailymed_clinical.md) | Research | Restraint-first experiment and human concept-preserving reference calibration. | Same-file calibration/tooling outcomes and [reference guide](../research/dailymed/clinical-reference.md) qualify early plans; later [real-label construction](decision_neurons_real_label_construction.md) is a separate experiment, not clinical gold. |
| [50-label construction](decision_neurons_real_label_construction.md) | Research | Governed pipeline measurement and diagnosis of entity extraction/drafting. | Same-file implementation addenda close work still named in its old next steps; [100-label pilot](decision_pilot_clinical_graph.md) extends scale and precision measurement. |
| [100-label clinical graph](decision_pilot_clinical_graph.md) | Research | Measured precision, administrative filtering, and quarantined relation-review diagnostics. | D1/D2 record implementation/re-measurement; [answer head-to-head](decision_answer_head_to_head_real_labels.md) revises D3's normalization priority. |
| [Real-label answer head-to-head](decision_answer_head_to_head_real_labels.md) | Research | Negative graph-versus-vector result under a different setup from authored fixtures. | D1 records attempted/rejected normalization; D2 raises rule density as a candidate, not a measured fix. [Current plan](../implementation-plan.md) owns scheduling. |
| [WebNLG scoring](decision_webnlg_scoring.md) | Research | Frozen exact-triple/entity-provided policy, one-shot test, and later development-set experiments. | Its CG-27 current-reading note and [pilot guide](../research/webnlg/pilot.md) qualify old future/full-loop claims; fuzzy matching was rejected and candidate pruning is not runtime signed review. |

## Superseded identity history

| Decision | Status | Operative scope | Successor / amendment |
|---|---|---|---|
| [Unicode normalization history](decision_unicode_semantics.md) | Historical | Preserves original codepoint/no-normalization decision, July implicit NFC change, and the separate CG-5 evidence boundary. | [Exact identity (CG-33)](decision_exact_reference_identity.md) replaces generic NFC-on-write/literals. Explicit construction canonicalization and locale collation remain applicable. |

## Maintaining the index

Create one `decision_{topic}.md` per architectural or strategic decision, with
Context, Decision (including its owner), and Outcome. Add exactly one inventory
row in the appropriate section above. Cross-links elsewhere may repeat freely.
For an amendment or reversal, name the replaced clause, link the successor, and
update the prior row in the same change. Preserve historical outcomes; append a
dated clarification when needed. Recheck affected owner guides and update the
implementation plan and issue registry when delivery status changes.

Run `python3 scripts/check-decision-index.py` from the repository root. The check
requires complete, unique inventory coverage, recognized statuses, and resolving
local index links/Markdown anchors. It does not establish implementation parity
or validate every outgoing link inside historical records. Reconcile semantic
status against the named outcomes, current guides, and focused code/runtime
evidence when the contract changes. The initial reconciliation and evidence
boundary are recorded in the [CG-30 report](../issues/decision-index-2026-09-09.md).
