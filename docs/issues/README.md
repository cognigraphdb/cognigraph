# CogniGraph issue registry

This is the local issue registry for CogniGraph. Each issue has a permanent,
unpadded `CG-{number}.md` identifier. Allocate the next unused integer; do not
renumber or reuse closed identifiers. Update this index when adding or changing
an issue.

The initial [2026-09-08 product-status and Rust review](review-2026-09-08.md)
examined revision `bc4bff1`, the current documentation, research drafts, and
fixtures. It recorded **30 open issues**: 9 P1, 20 P2, and 1 P3.
No finding is marked fixed by the review.

After the [first remediation batch](fixes-2026-09-08.md), **5 issues are
Resolved** (CG-2, CG-4, CG-9, CG-17, CG-18). A separate hybrid result-cache
collision found during remediation is CG-31.

The [Lua execution-control batch](lua-controls-2026-09-08.md) additionally
resolved CG-1 and CG-6, and the [directed deployment fence](directed-fence-2026-09-08.md)
resolved CG-3. The [paged-cache batch](paged-cache-2026-09-08.md) resolved CG-10
and CG-14. [Tenant derivative isolation](derivative-isolation-2026-09-08.md)
resolved CG-11. [Request isolation](request-isolation-2026-09-08.md) resolved
CG-12. [Hybrid cache identity](hybrid-cache-2026-09-08.md) resolved CG-31 and
recorded sibling-route fingerprint defects as CG-32.
[Semantic and graph cache identity](search-cache-2026-09-08.md) resolved CG-32.
[Shared completion-provider selection](completion-provider-2026-09-08.md)
resolved CG-29. [Mutation backend-read validation](mutation-backend-reads-2026-09-08.md)
resolved CG-7 using pre-execution rejection of unsupported reads.
[Shared dynamic query execution](dynamic-query-2026-09-08.md) resolved CG-8
and CG-15, including analysis parity and cumulative row accounting.
[Document lookup error propagation](document-errors-2026-09-08.md) resolved
CG-16, preserving backend failures through normal/analyzed queries and Lua.
[Canonical construction evidence](unicode-evidence-2026-09-09.md) resolved CG-5,
using NFC text before grounding, hashes, byte spans, and occurrence keys.
[Shared side-view lifecycle](side-view-lifecycle-2026-09-09.md) resolved CG-13,
covering cascades, generation races, and retryable cleanup. Its remediation also
identified [opaque reference normalization](CG-33.md).
[Traversal confidence parity](traversal-confidence-2026-09-09.md) resolved CG-19,
with per-edge pruning, default confidence, depth zero, and live Arango verification.
[Arango vector candidate filtering](vector-model-filter-2026-09-09.md) resolved CG-20,
including parent deduplication beyond a fixed candidate prefix.
[Native sidecar model selection](sidecar-model-filter-2026-09-09.md) resolved
CG-34 across base and delta candidates, writes, rebuilds, and restarts. Its
runtime verification identified unnecessary startup derivative invalidation
as [CG-35](CG-35.md).
[Neuron lifecycle serialization](neuron-lifecycle-2026-09-09.md) resolved CG-21,
including stale automated review publication. Its runtime setup identified
dedicated judge endpoint configuration as [CG-36](CG-36.md).
[Exact reference identity](exact-identity-2026-09-09.md) resolved CG-33, preserving
opaque values through writes, queries, durable job restart, and traversal. Its
bounded offline diagnosis/repair commands require explicit mappings and preserve
protected records. [API/operator contract parity](api-contract-2026-09-09.md)
resolved CG-22, including all four job kinds, directed replacement, provider
configuration, executable examples, and schema/request drift checks.
[Roadmap/workload parity](roadmap-parity-2026-09-09.md) resolved CG-23, separating
historical failures and measurements from delivered D1–D12 and the active backlog.
[Subquery specification consistency](subquery-contract-2026-09-09.md) resolved
CG-28, with executable/rejected examples and exact grouping/mutation limits.
Its probes found nested generated-name collisions ([CG-37](CG-37.md)) and
read-only query error misclassification ([CG-38](CG-38.md)).
[Whole-query generated bindings](subquery-bindings-2026-09-09.md) resolved CG-37,
including nested/sibling correlation, dynamic reads, and analysis/budget parity.
[Public query error classification](query-error-status-2026-09-09.md) resolved
CG-38, returning matching HTTP 400 plan errors while preserving backend statuses.
[Idempotent collection ensures](collection-ensure-2026-09-09.md) resolved CG-35,
preserving current vector/text files through authenticated startup and retaining
invalidation after real writes.
[Shared dedicated judge endpoints](judge-endpoint-2026-09-09.md) resolved CG-36,
preserving explicit model selection and qualification while validating both
judges through the shared OpenAI configuration before storage opens.
[Paper counting units](paper-counting-units-2026-09-09.md) resolved CG-24,
reconciling distinct construction facts and per-question answer mentions across
the draft and its publication exports, with twelve offline construction replays.
[WebNLG status and artifact reconciliation](webnlg-status-2026-09-09.md) resolved
CG-27, distinguishing completed experiment stages, development-set tradeoffs,
and candidate review from the full runtime lifecycle, with offline replays.
[Decision navigation](decision-index-2026-09-09.md) resolved CG-30, indexing all
53 records with scoped statuses, amendments, and current owner documents.
[CUAD evidence recovery](cuad-recovery-2026-09-09.md) partially remediates CG-25:
pinned inputs and score replay are available, mixed-unit recall/F1 claims are
withdrawn, and the original extraction-provenance gap remains documented.
The user subsequently retired CUAD; CG-25 is **Closed without change** for
the missing historical execution evidence. The [Luna baseline](../decisions/decision_luna_baseline.md)
is a new captured evaluation, not a reconstruction of that history.
[Server modularity](server-modularity-2026-09-09.md) resolves CG-26 across its six
named hotspots, with compatible entry points and seven documented size exceptions.
The original remediation batch closed all 38 tickets. The subsequent
[Luna document trial](../research/experiments/luna-documents-2026-09-09/results.md)
identified two new directed-construction findings: endpoint substring acceptance
(CG-39) and unconstrained completion identifiers (CG-40). The current registry
had **39 Resolved, 1 Closed without change, and 0 Open issues** after the
[policy v2 fixes and development verification](../decisions/decision_directed_extraction_contracts.md).

The [2026-09-10 Collections QA](../evidence/ui-2026-09-10-collections.md#artifact-088100ac46441b68bcc4)
resolved CG-41–CG-44. That checkpoint had **43 Resolved, 1 Closed without change,
and 0 Open issues**. Packaging review added CG-45–CG-48; the
[Community/Enterprise build split](CG-45.md) closes the last of these.
The [2026-09-11 full UI review](../evidence/ui-2026-09-11-full-review.md#artifact-2a08bd45b55e9bd0cb9c)
adds CG-49–CG-63: **15 Open issues (4 P1, 11 P2)** at that checkpoint.
The review includes real Community/Enterprise browser and API evidence, and a
separate map of backend capabilities not yet exposed by the console. No finding
was fixed by the audit; this count is not complete UI or product acceptance.

The [data-preservation fixes](../evidence/ui-2026-09-11-data-preservation.md#artifact-6afad6337a2d11fc893d)
resolve CG-50 and CG-62. That checkpoint had **49 Resolved, 1 Closed without change,
and 13 Open issues (2 P1, 11 P2)**.
[Production origin and connection-state verification](../evidence/ui-2026-09-11-production-origin.md#artifact-a495041e72ddd34399af)
resolves CG-49. That checkpoint had **50 Resolved, 1 Closed without change, and
12 Open issues (1 P1, 11 P2)**.
[Tenant deletion disclosure and lifecycle verification](../evidence/ui-2026-09-11-tenant-deletion.md#artifact-0ec008ada9942ef8213a)
resolves CG-51 and adds Enterprise host-admin probe coverage to CG-49. That
checkpoint had **51 Resolved, 1 Closed without change, and 11 Open issues (all P2)**.
[Tenant onboarding and user provisioning](../evidence/ui-2026-09-11-provisioning.md#artifact-e96a930169a1f87db8a2)
resolves CG-52. That checkpoint had **52 Resolved, 1 Closed without change, and
10 Open issues (all P2)**.
[Verified console capabilities](../evidence/ui-2026-09-11-capabilities.md#artifact-29cda8d45940a3ab08a8)
resolves CG-53. That checkpoint had **53 Resolved, 1 Closed without change, and
9 Open issues (all P2)**.
[Review paging and complete space catalogs](../evidence/ui-2026-09-11-review-paging.md#artifact-cbe96c45605c68a64fba)
resolves CG-54. That checkpoint had **54 Resolved, 1 Closed without change, and
8 Open issues (all P2)**.
[Console execution guards](../evidence/ui-2026-09-11-execution-guards.md#artifact-ba0883e40b9f39017230)
resolves CG-55. That checkpoint had **55 Resolved, 1 Closed without change, and
7 Open issues (all P2)**.
[Input-scoped console results](../evidence/ui-2026-09-11-result-ownership.md#artifact-8aa8b46fb43543771a2c)
resolves CG-56. The [table keyboard-access batch](../evidence/ui-2026-09-11-table-keyboard.md#artifact-a5b3ed52a87a809a63dc)
resolves CG-57. The [space guidance and catalog-state batch](../evidence/ui-2026-09-11-space-guidance.md#artifact-ec49fa342ffd2bbdc01d)
resolves CG-58. The [collection search/filter scope batch](../evidence/ui-2026-09-11-collection-search-scope.md#artifact-1c2e51790ed0aa822e41)
resolves CG-59. The [sidebar naming fix](../evidence/ui-2026-09-11-sidebar-names.md#artifact-2bc6b4831aa16897066a)
resolves CG-63. The [React Router update](../evidence/ui-2026-09-11-router-update.md#artifact-f2ee775cf86eb46ca1d7)
resolves CG-61. The [shared UI CI and browser gate](../evidence/ui-2026-09-11-ui-ci.md#artifact-d71f3d04fcdc5baa0fdb)
resolves CG-60. That checkpoint had **62 Resolved, 1 Closed without change, and
0 Open issues**.

The [Native-only storage batch](../plans/native-only-2026-09-12.md), approved on
2026-09-12, adds CG-64–CG-68. [Native conformance coverage](native-conformance-2026-09-12.md)
resolves CG-65. [Runtime removal](native-runtime-2026-09-12.md) resolves CG-67.
[Local readiness](native-readiness-2026-09-12.md) resolves CG-68.
Current registry: **79 Resolved, 1 Closed without
change, and 3 Open issues**. [CG-81](CG-81.md) tracks unfixed image findings.
[CG-83](CG-83.md) resolves the dependency refresh and local candidate qualification
required by the [develop gate](CG-82.md); publication remains separate.
The Native-only batch is complete. [CG-69](CG-69.md)
resolves automatic CI/protections; [CG-71](CG-71.md) resolves first Community
deployment and console packaging with [live recovery evidence](railway-community-2026-09-12.md).
[CG-70](CG-70.md) resolves Native lru
unsoundness and records a reviewed optional ONNX maintenance exception.
[CG-72](CG-72.md) resolves the subsequent login centering defect in narrow
windows with local Chromium/WebKit and both-edition browser evidence;
published to develop in v2.7.8 and deployed to Railway in v2.7.11 with
[hosted acceptance](../evidence/ui-2026-09-13-railway-v2.7.11.md#artifact-28071583ad3d71443c64).
CG-64 and CG-66 remain
Open with deferred optional scheduling; they do not block Native-only readiness
or first deployment. These are engineering tasks under the
[Native-only decision](../decisions/decision_native_only.md), not reopened UI
defects. No ticket is closed merely because it was deferred.

The [CG-73 transition](CG-73.md) integrates the six incoming dependency updates,
replaces Dependabot with Renovate, and establishes protected develop integration
with a separate main release branch. Its activation checkpoint has only the two
permanent branches and the primary worktree: **70 Resolved, 1 Closed without
change, and 2 Open issues** (CG-64/CG-66 remain deferred).

[CG-74](CG-74.md) resolves the transient-card selector race found during the
v2.7.10 promotion. Its v2.7.11 correction and promotion passed required CI and
merged into main. Historical failed runs remain linked from the issue.

## Conventions

- Status: Open, In progress, Blocked, Resolved, or Closed without change. Record the reason and evidence for the last two.
- Priority: P1 = urgent isolation, execution-control, or data-integrity defect; P2 = correctness, contract, reproducibility, or material maintainability work; P3 = lower-impact improvement. No P0 was assigned in this review.
- Each issue records the affected behavior, source evidence, reproduction or failure sequence, and acceptance criteria. Source-confirmed findings are distinct from live reproductions.
- Close a Rust issue only after the required formatting, Clippy, and test gates plus its focused real-runtime regression pass. Documentation-only closure requires source/link/example verification appropriate to the change.
- Keep research counting units and experiment provenance explicit. Historical milestone completion is not evidence that a newly discovered defect is resolved.

## Issues

| Issue | Priority | Status | Finding |
|---|---|---|---|
| [CG-1](CG-1.md) | P1 | Resolved | Lua scripts can re-enable JIT and bypass the instruction limit |
| [CG-2](CG-2.md) | P1 | Resolved | Collection names can escape the Native text-index directory |
| [CG-3](CG-3.md) | P1 | Resolved | Directed construction bypasses the deployed-generation mutation fence |
| [CG-4](CG-4.md) | P1 | Resolved | Malformed directed-completion output is treated as an empty replacement |
| [CG-5](CG-5.md) | P2 | Resolved | Native NFC stamping breaks construction hashes and byte spans |
| [CG-6](CG-6.md) | P1 | Resolved | Lua graph.query does not receive the configured CGQL budgets |
| [CG-7](CG-7.md) | P2 | Resolved | DOCUMENT expressions in mutations silently evaluate to null |
| [CG-8](CG-8.md) | P2 | Resolved | EXPLAIN ANALYZE uses different DOCUMENT semantics than execution |
| [CG-9](CG-9.md) | P2 | Resolved | UPSERT ignores additional match fields when _key is present |
| [CG-10](CG-10.md) | P1 | Resolved | Paged document reads can remain stale after a committed concurrent write |
| [CG-11](CG-11.md) | P1 | Resolved | Tenant recreation reuses the deleted tenant's text index |
| [CG-12](CG-12.md) | P1 | Resolved | In-flight requests can resume into a recreated tenant's store |
| [CG-13](CG-13.md) | P2 | Resolved | Side-view orphan prevention is bypassed by alternate deletes |
| [CG-14](CG-14.md) | P1 | Resolved | Dropping a collection does not clear paged document cache entries |
| [CG-15](CG-15.md) | P2 | Resolved | Dynamic query resolution resets source-row accounting |
| [CG-16](CG-16.md) | P2 | Resolved | DOCUMENT converts backend failures into successful null results |
| [CG-17](CG-17.md) | P2 | Resolved | Duplicate text-search fields panic instead of returning validation errors |
| [CG-18](CG-18.md) | P2 | Resolved | Text-index cache keys conflate distinct field lists |
| [CG-19](CG-19.md) | P2 | Resolved | Arango traversal confidence differs from Native semantics |
| [CG-20](CG-20.md) | P2 | Resolved | Arango indexed vector search filters model identity too late |
| [CG-21](CG-21.md) | P2 | Resolved | Concurrent neuron acceptance can commit a forbidden hint/blocker pair |
| [CG-22](CG-22.md) | P2 | Resolved | Current API and operator references omit implemented construction and job surfaces |
| [CG-23](CG-23.md) | P2 | Resolved | Current roadmap still reports CGQL workload gaps that were closed |
| [CG-24](CG-24.md) | P2 | Resolved | Paper revision 2 reintroduces superseded fact denominators |
| [CG-25](CG-25.md) | P2 | Closed without change | CUAD headline results lack a repository reproduction package |
| [CG-26](CG-26.md) | P2 | Resolved | Governance and server modules substantially exceed the modularity convention |
| [CG-27](CG-27.md) | P2 | Resolved | WebNLG status and artifact links lag the recorded experiment outcomes |
| [CG-28](CG-28.md) | P2 | Resolved | CGQL specification both supports and forbids expression-position subqueries |
| [CG-29](CG-29.md) | P2 | Resolved | Server completion selection ignores the documented provider override |
| [CG-30](CG-30.md) | P3 | Resolved | Decision log needs an index of current and superseded contracts |
| [CG-31](CG-31.md) | P2 | Resolved | Hybrid result-cache fingerprints conflate distinct request parameters |
| [CG-32](CG-32.md) | P2 | Resolved | Semantic and graph-augmented cache keys still conflate distinct parameters |
| [CG-33](CG-33.md) | P2 | Resolved | Native normalization changes opaque references in jobs and edges |
| [CG-34](CG-34.md) | P2 | Resolved | Native sidecar vector search filters model identity after truncation |
| [CG-35](CG-35.md) | P2 | Resolved | Native collection ensures invalidate derivatives on unchanged startup |
| [CG-36](CG-36.md) | P2 | Resolved | Dedicated review judges ignore the configured OpenAI base URL |
| [CG-37](CG-37.md) | P2 | Resolved | Nested lifted subqueries can reuse a synthetic binding name |
| [CG-38](CG-38.md) | P2 | Resolved | Read-only query route reports parse and validation errors as HTTP 500 |
| [CG-39](CG-39.md) | P2 | Resolved | Directed endpoint checks accept names embedded inside longer words |
| [CG-40](CG-40.md) | P2 | Resolved | Directed completion schema leaves request-bound identifiers unconstrained |
| [CG-41](CG-41.md) | P1 | Resolved | Background refresh discards unsaved document edits |
| [CG-42](CG-42.md) | P2 | Resolved | Collections deep links select the wrong document beyond page one |
| [CG-43](CG-43.md) | P2 | Resolved | Collections controls overflow at scaled desktop widths |
| [CG-44](CG-44.md) | P2 | Resolved | Closing a Collections dialog loses keyboard focus |
| [CG-45](CG-45.md) | P2 | Resolved | Community binary cannot exclude Enterprise Components |
| [CG-46](CG-46.md) | P2 | Resolved | Helm backups need isolated selectors and safe snapshot handling |
| [CG-47](CG-47.md) | P2 | Resolved | Issue identity checks must survive commits and concurrent allocation |
| [CG-48](CG-48.md) | P2 | Resolved | Quick-start examples must match network and query contracts |
| [CG-49](CG-49.md) | P1 | Resolved | Production console always targets API port 3001 |
| [CG-50](CG-50.md) | P1 | Resolved | Document inspector rewrites valid stored JSON through a lossy projection |
| [CG-51](CG-51.md) | P1 | Resolved | Tenant deletion confirmation promises recovery that recreation does not provide |
| [CG-52](CG-52.md) | P2 | Resolved | Tenant onboarding and user provisioning diverge from the authorization contract |
| [CG-53](CG-53.md) | P2 | Resolved | Console actions ignore server edition and authenticated role capabilities |
| [CG-54](CG-54.md) | P2 | Resolved | Review queue and space selectors silently truncate their datasets |
| [CG-55](CG-55.md) | P2 | Resolved | Console keyboard shortcuts bypass the running request guard |
| [CG-56](CG-56.md) | P2 | Resolved | Console results remain attached to changed inputs and failed graph requests |
| [CG-57](CG-57.md) | P2 | Resolved | Clickable table rows cannot be opened with the keyboard |
| [CG-58](CG-58.md) | P2 | Resolved | Review empty-state guidance recommends a forbidden space creation path |
| [CG-59](CG-59.md) | P2 | Resolved | Collection search and filters present partial matches as complete results |
| [CG-60](CG-60.md) | P2 | Resolved | CI omits the UI checks and browser regression coverage |
| [CG-61](CG-61.md) | P2 | Resolved | React Router lockfile retains an advisory-affected release |
| [CG-62](CG-62.md) | P1 | Resolved | Plain-text construction imports reuse chunk IDs and replace earlier evidence |
| [CG-63](CG-63.md) | P2 | Resolved | Collapsed sidebar removes the accessible names of navigation links |
| [CG-64](CG-64.md) | P2 | Resolved | Define the ArangoDB dump migration contract and qualification fixtures |
| [CG-65](CG-65.md) | P2 | Resolved | Preserve Native conformance coverage before retiring ArangoDB |
| [CG-66](CG-66.md) | P2 | Resolved | Implement the Community offline ArangoDB dump importer |
| [CG-67](CG-67.md) | P2 | Resolved | Remove the ArangoDB runtime backend and reject retired configuration |
| [CG-68](CG-68.md) | P2 | Resolved | Qualify and document the Native-only release |
| [CG-69](CG-69.md) | P1 | Resolved | Automate Native CI and enforce verified repository changes |
| [CG-70](CG-70.md) | P2 | Resolved | Track transitive dependency safety and maintenance advisories |
| [CG-71](CG-71.md) | P1 | Resolved | Package and qualify the first Community deployment with the console |
| [CG-72](CG-72.md) | P2 | Resolved | Login screen is off-center and clipped in narrow viewports |
| [CG-73](CG-73.md) | P2 | Resolved | Route dependency updates through develop and retire stale branches |
| [CG-74](CG-74.md) | P2 | Resolved | Login layout test can measure the transient connection card after logout |
| [CG-75](CG-75.md) | P2 | Resolved | Separate captured research evidence from public distribution |
| [CG-76](CG-76.md) | P2 | Resolved | Define and enforce the hosted database access boundary |
| [CG-77](CG-77.md) | P2 | Resolved | Enforce the public evidence boundary in CI and agent workflows |
| [CG-78](CG-78.md) | P2 | Resolved | Validate archived documentation links and preserve amended-source provenance |
| [CG-79](CG-79.md) | P2 | Resolved | Remove leaked SPA test fixture directories |
| [CG-80](CG-80.md) | P1 | Resolved | Published container base packages carry fixable vulnerabilities |
| [CG-81](CG-81.md) | P2 | Resolved | Triage remaining container CVEs without Debian fixes |
| [CG-82](CG-82.md) | P1 | Resolved | Enforce PR review and dependency freshness before develop integration |
| [CG-83](CG-83.md) | P1 | Resolved | Refresh the application dependency baseline for develop qualification |
| [CG-84](CG-84.md) | P2 | Resolved | TypeScript client for Bun and Node applications |
| [CG-85](CG-85.md) | P2 | Resolved | Bind variables in LIMIT for client-controlled pagination |
| [CG-86](CG-86.md) | P2 | Resolved | Unique constraints and user-declarable indexes on ordinary collections |
| [CG-87](CG-87.md) | P2 | Resolved | Native linux/arm64 and macOS Apple Silicon builds for local development |
| [CG-88](CG-88.md) | P2 | Resolved | Side views as a proposal-queue gap detector |
| [CG-89](CG-89.md) | P2 | Resolved | Graph-augmented search defaults to document_relations and omits neuron-built facts |
| [CG-90](CG-90.md) | P2 | Resolved | Construction gate refusals are not durably recorded |
| [CG-91](CG-91.md) | P3 | Resolved | Construct crate module docs describe a graduated status that does not exist |
| [CG-92](CG-92.md) | P1 | Resolved | Refresh dependencies to pass the develop freshness gate after the v2.7.15 qualification |
| [CG-93](CG-93.md) | P3 | Open | Move CI runners from pinned Ubuntu 24.04 to Ubuntu 26.04 deliberately |
| [CG-94](CG-94.md) | P3 | Resolved | Split propose.rs and directed.rs in cognigraph-construct to the module line budget |
| [CG-95](CG-95.md) | P3 | Resolved | Split draft.rs and grounding.rs in cognigraph-construct to the module line budget |
| [CG-96](CG-96.md) | P2 | Resolved | Promotion lifecycle tests overflow the default test-thread stack in x86_64 debug builds |
| [CG-97](CG-97.md) | P2 | Open | Measure side-view gap proposals on a public Semantic Neurons kit |

Next available identifier: **CG-98**.
## After the current ticket list

CG-1 through CG-38 are now resolved or explicitly closed. The user selected
a [captured Luna baseline and bounded Astra reference](../decisions/decision_luna_baseline.md)
after retiring CUAD. The subsequent
[DeepSeek/GLM comparison](../research/experiments/cross-provider-baseline-2026-09-09/results.md)
has completed. The user has dropped DeepSeek V4 Flash from all future runs;
its failed capture is retained as history. The
[larger independently labelled supplied-pair trial](../research/experiments/glm-luna-semeval-2026-09-09/results.md)
is complete: GLM Flash scored 65.75% versus Luna's 42.67% pooled accuracy,
with substantial Other-case restraint errors in both models.
The subsequent [Terra low / Luna low extension](../research/experiments/terra-luna-low-semeval-2026-09-09/results.md)
scored 72.42% and 70.08%. Terra's 2.33-point margin costs about 10.4 times as
much. That experiment recommended Luna low and GLM Flash as economical
candidates, with Terra as a stronger reference; its supplied-pair results do not
qualify document extraction. The user subsequently selected **Luna low**, now
adopted in the shared runtime with [completed Rust and live HTTP verification](luna-low-runtime-2026-09-09.md). CG-26 and
the completed evaluation evidence are checkpointed locally as `8b8cceb`. Next
comes representative document qualification and restraint work on separate
development and holdout splits. The original document trial was checkpointed
as `a0e4b24`; its CG-39/CG-40 findings are now resolved with a
[separately frozen development candidate](../research/experiments/luna-directed-v2-2026-09-09/results.md).
Next comes independent reference/evidence-policy review before precision claims
or holdout execution. Further model comparisons are optional references.
See the [implementation plan](../implementation-plan.md#review-checkpoint--2026-09-09)
and [existing measured baseline](../research/experiments/sideviews-model-comparison-2026-09-08/README.md).

## Review evidence

- [Product status, crate map, scope, validation and remediation order](review-2026-09-08.md)
- [Sanitized local HTTP observations](../evidence/engineering-historical-checks.md#artifact-b88e1642e95b63187a57)

The source citations inside issues refer to the reviewed revision. Recheck line
numbers and behavior when implementing a fix, especially after the modularity work.
